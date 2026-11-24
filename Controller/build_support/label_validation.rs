//! Exercise each declared label against the text supplied by both clients.
use std::io;

use serde_json::Value;

use crate::base_suffixes::BaseLocaleCoverage;
use crate::item_catalog::{localized_text, ItemCatalog, NameLine, CHINESE_OVERLAY};
use crate::labels::{ItemName, Label, LabelOptions, LabelRenderer};
use crate::region::Region;
use crate::string_banks::TemplateStringBanks;

fn boolean_combinations<const N: usize>() -> impl Iterator<Item = [bool; N]> {
    (0..(1usize << N)).map(|mask| std::array::from_fn(|bit| mask & (1 << bit) != 0))
}

fn label_options() -> impl Iterator<Item = LabelOptions> {
    boolean_combinations().map(
        |[english_names, max_hints, base_tiers, armor_weights, rune_numbers, rune_recipes]| {
            LabelOptions {
                english_names,
                max_hints,
                base_tiers,
                armor_weights,
                rune_numbers,
                rune_recipes,
            }
        },
    )
}

fn validate_english_name(row: &Value, roles: &[NameLine]) -> io::Result<()> {
    if roles.contains(&NameLine::EnglishName) {
        let parse = |locale| {
            ItemName::parse(localized_text(row, locale)?, roles, locale)
                .map_err(|error| io::Error::other(format!("{locale}: {error}")))
        };
        let english = parse("enUS")?;
        for locale in ["zhTW", "zhCN"] {
            if parse(locale)?.english_name != Some(english.localized) {
                return Err(io::Error::other(format!(
                    "{locale}: english_name line differs from the item's enUS name"
                )));
            }
        }
    }
    Ok(())
}

/// Report whether any supported selection changes the text supplied to the Chinese overlay.
fn validate_translations(row: &Value, labels: &[Label<'_>]) -> io::Result<bool> {
    let mut changes_traditional = false;
    for label in labels {
        for locale in label.locales(row) {
            let original = localized_text(row, locale)?;
            let rendered = label
                .text(locale, original)
                .map_err(|error| io::Error::other(format!("{locale}: {error}")))?;
            changes_traditional |= locale == "zhTW" && rendered != original;
        }
    }
    Ok(changes_traditional)
}

pub(super) fn validate_labels(
    catalog: &ItemCatalog,
    banks: &mut TemplateStringBanks<'_>,
) -> io::Result<()> {
    let renderers: Vec<_> = label_options()
        .map(|options| LabelRenderer::new(catalog, options))
        .filter(|renderer| !renderer.keeps_template())
        .collect();
    let mut base_coverage: Vec<_> = catalog.bases.iter().map(BaseLocaleCoverage::new).collect();
    for (bank, records) in &catalog.labels {
        if bank == CHINESE_OVERLAY {
            return Err(io::Error::other(
                "items.json: the Chinese overlay cannot own item labels",
            ));
        }
        for (&id, keys) in &records.by_id {
            for (key, record) in &keys.by_key {
                let labels = renderers
                    .iter()
                    .map(|renderer| renderer.bind(record))
                    .collect::<io::Result<Vec<_>>>()
                    .map_err(|error| {
                        io::Error::other(format!("items.json: {bank}/{id}/{key}: {error}"))
                    })?;
                for region in Region::ALL {
                    let row = banks.lookup(region, bank, id, key)?;
                    let row_error = |error: io::Error| {
                        io::Error::other(format!("{}: {bank}/{id}/{key}/{error}", region.code()))
                    };
                    validate_english_name(row, &record.name_lines).map_err(row_error)?;
                    if let Some(index) = record.base_suffix_group {
                        base_coverage[index]
                            .record_row((bank, id, key), row)
                            .map_err(row_error)?;
                    }
                    let changes_traditional =
                        validate_translations(row, &labels).map_err(row_error)?;
                    if changes_traditional {
                        banks
                            .overlay_sources(region)
                            .validate_target(bank, id, key)
                            .map_err(|error| {
                                io::Error::other(format!("{}: {error}", region.code()))
                            })?;
                    }
                }
            }
        }
    }
    for (index, coverage) in base_coverage.iter().enumerate() {
        coverage.check_group(index)?;
    }
    Ok(())
}
