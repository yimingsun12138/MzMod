//! The item template schema shared by the Cargo build and the game resource renderer.
use std::{collections::BTreeMap, io};

use serde::Deserialize;
use serde_json::Value;

pub(crate) use crate::rune_catalog::Rune;
use crate::{item_colors::RegionalItemColors, rune_catalog::RUNE_COUNT};

pub(crate) const STRING_DIRECTORY: &str = "data/local/lng/strings/";
pub(crate) const CHINESE_OVERLAY: &str = "chinese-overlay";

pub(crate) fn string_bank_path(bank: &str) -> String {
    format!("{STRING_DIRECTORY}{bank}.json")
}

pub(crate) fn localized_text<'a>(row: &'a Value, locale: &str) -> io::Result<&'a str> {
    row[locale]
        .as_str()
        .ok_or_else(|| io::Error::other(format!("{locale}: expected a string")))
}

/// D2R identifies translations with a lowercase language and an uppercase region.
pub(crate) fn native_locales(row: &Value) -> impl Iterator<Item = &str> {
    row.as_object()
        .into_iter()
        .flat_map(|row| row.keys())
        .map(String::as_str)
        .filter(|field| {
            matches!(
                field.as_bytes(),
                [b'a'..=b'z', b'a'..=b'z', b'A'..=b'Z', b'A'..=b'Z']
            )
        })
}

pub(crate) fn ground_model_path(route: &str) -> String {
    format!("data/hd/items/misc/{route}.json")
}

pub(crate) fn inventory_icon_path(sprite: &str) -> String {
    format!("data/hd/global/ui/items/misc/{sprite}")
}

pub(crate) fn inventory_sprite_name(route: &str) -> String {
    format!("{route}.sprite")
}

fn inventory_sprite_pair(route: &str) -> [String; 2] {
    [
        inventory_sprite_name(route),
        format!("{route}.lowend.sprite"),
    ]
}

pub(crate) fn inventory_icon_pair(route: &str) -> [String; 2] {
    inventory_sprite_pair(route).map(|sprite| inventory_icon_path(&sprite))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Charm {
    pub route: String,
    pub model: Value,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Charms {
    pub small: Charm,
    pub large: Charm,
    pub grand: Charm,
    pub annihilus_route: String,
    pub hellfire_torch_route: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct JewelModel {
    pub native: Value,
    numbered_inventory_variants: Option<usize>,
    icons: Option<Vec<String>>,
}

impl JewelModel {
    pub(crate) fn inventory_sprites<'a>(
        &'a self,
        route: &'a str,
    ) -> io::Result<impl Iterator<Item = String> + 'a> {
        if self.numbered_inventory_variants.is_none() && self.icons.is_none() {
            return Err(io::Error::other(format!(
                "{route}: expected inventory variants or an explicit sprite list"
            )));
        }
        let variants = self
            .numbered_inventory_variants
            .into_iter()
            .flat_map(|count| 0..=count);
        let native = variants.flat_map(move |index| {
            if index == 0 {
                inventory_sprite_pair(route)
            } else {
                inventory_sprite_pair(&format!("{route}{index}"))
            }
        });
        Ok(native.chain(self.icons.iter().flatten().cloned()))
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Jewels {
    pub native_base_routes: Value,
    #[serde(deserialize_with = "crate::unique_map::deserialize")]
    pub base_routes: BTreeMap<String, String>,
    #[serde(deserialize_with = "crate::unique_map::deserialize")]
    pub unique_routes: BTreeMap<String, String>,
    #[serde(deserialize_with = "crate::unique_map::deserialize")]
    pub models: BTreeMap<String, JewelModel>,
}

pub(crate) const ASSET_SLOTS: [&str; 3] = ["normal", "uber", "ultra"];

/// Unique HD routes can refer to a model relative to the misc item directory.
pub(crate) fn misc_model_route(asset: &str) -> &str {
    asset.strip_prefix("../misc/").unwrap_or(asset)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BaseSuffixes {
    pub full: String,
    pub tier: String,
    pub weight: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BaseGroup {
    #[serde(deserialize_with = "crate::unique_map::deserialize")]
    pub suffixes: BTreeMap<String, BaseSuffixes>,
}

#[derive(Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum NameLine {
    MaxHint,
    EnglishName,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RuneLabel {
    pub number: usize,
    #[serde(default)]
    pub recipe_line: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LabelRecord {
    #[serde(default)]
    pub name_lines: Vec<NameLine>,
    pub base_suffix_group: Option<usize>,
    pub rune: Option<RuneLabel>,
}

impl LabelRecord {
    pub(crate) fn resolve_base_group<'a>(
        &self,
        catalog: &'a ItemCatalog,
    ) -> io::Result<Option<&'a BaseGroup>> {
        let Some(index) = self.base_suffix_group else {
            return Ok(None);
        };
        let group = catalog
            .bases
            .get(index)
            .ok_or_else(|| io::Error::other(format!("unknown base suffix group {index}")))?;
        Ok(Some(group))
    }

    pub(crate) fn resolve_rune_label<'a>(
        &'a self,
        catalog: &'a ItemCatalog,
    ) -> io::Result<Option<(&'a RuneLabel, &'a Rune)>> {
        let Some(rule) = &self.rune else {
            return Ok(None);
        };
        let rune = rule
            .number
            .checked_sub(1)
            .and_then(|index| catalog.runes.get(index))
            .ok_or_else(|| io::Error::other(format!("unknown rune number {}", rule.number)))?;
        Ok(Some((rule, rune)))
    }
}

#[derive(Deserialize)]
#[serde(transparent)]
pub(crate) struct LabelBank {
    #[serde(deserialize_with = "crate::unique_map::deserialize")]
    pub by_id: BTreeMap<u64, LabelKeys>,
}

#[derive(Deserialize)]
#[serde(transparent)]
pub(crate) struct LabelKeys {
    #[serde(deserialize_with = "crate::unique_map::deserialize")]
    pub by_key: BTreeMap<String, LabelRecord>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ItemCatalog {
    #[serde(deserialize_with = "crate::rune_catalog::by_number")]
    pub runes: [Rune; RUNE_COUNT],
    pub beam: Value,
    pub charms: Charms,
    pub jewels: Jewels,
    pub bases: Vec<BaseGroup>,
    #[serde(deserialize_with = "crate::unique_map::deserialize")]
    pub labels: BTreeMap<String, LabelBank>,
    pub native_item_colors: RegionalItemColors,
}

impl ItemCatalog {
    pub(crate) fn label(&self, bank: &str, id: u64, key: &str) -> Option<&LabelRecord> {
        self.labels.get(bank)?.by_id.get(&id)?.by_key.get(key)
    }
}
