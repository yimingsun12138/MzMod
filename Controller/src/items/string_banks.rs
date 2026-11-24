//! Apply label changes to the regional language banks and their Chinese overlay.
use std::collections::BTreeMap;

use anyhow::{Context, Result};
use serde_json::Value;

use super::labels::{LabelOptions, LabelRenderer};
use crate::item_catalog::{localized_text, string_bank_path, ItemCatalog, CHINESE_OVERLAY};
use crate::resources::{read_document, write_document};
use crate::{ModFiles, Settings};

impl From<&Settings> for LabelOptions {
    fn from(settings: &Settings) -> Self {
        Self {
            english_names: settings.english_names,
            max_hints: settings.max_hints,
            base_tiers: settings.base_tiers,
            armor_weights: settings.armor_weights,
            rune_numbers: settings.rune_numbers,
            rune_recipes: settings.rune_recipes,
        }
    }
}

fn identity(row: &Value) -> Result<(u64, &str)> {
    let id = row["id"].as_u64().context("missing string id")?;
    let key = row["Key"].as_str().context("missing string key")?;
    Ok((id, key))
}

fn edit_rows(
    document: &mut Value,
    bank: &str,
    mut edit: impl FnMut(&mut Value) -> Result<bool>,
) -> Result<bool> {
    let mut changed = false;
    for row in document
        .as_array_mut()
        .with_context(|| format!("invalid {bank} string bank"))?
    {
        changed |= edit(row).with_context(|| format!("{bank} / {} / {}", row["id"], row["Key"]))?;
    }
    Ok(changed)
}

pub(super) fn render(
    files: &mut ModFiles,
    settings: &Settings,
    catalog: &ItemCatalog,
    overlay_sources: &[(&str, u64, &str)],
) -> Result<()> {
    let renderer = LabelRenderer::new(catalog, settings.into());
    if renderer.keeps_template() {
        return Ok(());
    }
    let mut chinese_changes = BTreeMap::new();
    for bank in catalog.labels.keys() {
        let path = string_bank_path(bank);
        let mut document = read_document(files, &path)?;
        let changed = edit_rows(&mut document, bank, |row| {
            let (id, key) = identity(row)?;
            let Some(record) = catalog.label(bank, id, key) else {
                return Ok(false);
            };
            let label = renderer.bind(record)?;
            let overlay_key = overlay_sources
                .binary_search(&(bank.as_str(), id, key))
                .ok()
                .map(|index| overlay_sources[index].2);
            let mut updates = Vec::new();
            for locale in label.locales(row) {
                let original = localized_text(row, locale)?;
                let new = label
                    .text(locale, original)
                    .with_context(|| format!("invalid {locale} label"))?;
                if original != new {
                    updates.push((locale.to_owned(), new.into_owned()));
                }
            }
            let changed = !updates.is_empty();
            for (locale, new) in updates {
                if let ("zhTW", Some(key)) = (locale.as_str(), overlay_key) {
                    chinese_changes.insert((id, key), new.clone());
                }
                row[locale] = Value::String(new);
            }
            Ok(changed)
        })?;
        if changed {
            write_document(files, &path, &document)?;
        }
    }
    if !chinese_changes.is_empty() {
        let overlay_path = string_bank_path(CHINESE_OVERLAY);
        let mut overlay = read_document(files, &overlay_path)?;
        let changed = edit_rows(&mut overlay, CHINESE_OVERLAY, |row| {
            if let Some(new) = chinese_changes.get(&identity(row)?) {
                if localized_text(row, "zhCN")? != new {
                    row["zhCN"] = Value::String(new.clone());
                    return Ok(true);
                }
            }
            Ok(false)
        })?;
        if changed {
            write_document(files, &overlay_path, &overlay)?;
        }
    }
    Ok(())
}

#[cfg(any(windows, test))]
pub(crate) fn preview_label(bank: &str, row: &Value, locale: &str, settings: &Settings) -> String {
    let (id, key) = identity(row).expect("preview item identity");
    let original = localized_text(row, locale).expect("validated preview locale");
    let catalog = super::embedded_catalog();
    let Some(record) = catalog.label(bank, id, key) else {
        return original.to_owned();
    };
    LabelRenderer::new(catalog, settings.into())
        .bind(record)
        .expect("validated preview label rule")
        .text(locale, original)
        .expect("validated preview label")
        .into_owned()
}
