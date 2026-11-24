//! Apply player choices to item labels, inventory icons and ground models.
use std::sync::OnceLock;

use anyhow::Result;

use crate::item_catalog::ItemCatalog;
use crate::item_colors::PROFILE_PATH;
use crate::resources::{read_document, write_document};
use crate::{ModFiles, Region, Settings};

mod beams;
mod icons;
mod jewels;
mod labels;
mod string_banks;

#[cfg(windows)]
pub(crate) use string_banks::preview_label;
#[cfg(test)]
mod tests;

const ITEM_CATALOG_BYTES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/item-catalog.json"));
const OVERLAY_SOURCES: [&[(&str, u64, &str)]; Region::ALL.len()] =
    include!(concat!(env!("OUT_DIR"), "/overlay_sources.rs"));

fn embedded_catalog() -> &'static ItemCatalog {
    static CATALOG: OnceLock<ItemCatalog> = OnceLock::new();
    CATALOG.get_or_init(|| {
        serde_json::from_slice(ITEM_CATALOG_BYTES)
            .expect("Cargo validates the embedded item catalog")
    })
}

#[cfg(windows)]
pub(crate) fn rune_names() -> impl Iterator<Item = (&'static str, &'static str)> {
    embedded_catalog()
        .runes
        .iter()
        .map(|rune| (rune.english.as_str(), rune.chinese.as_str()))
}

pub(crate) fn render(files: &mut ModFiles, settings: &Settings, region: Region) -> Result<()> {
    let catalog = embedded_catalog();
    string_banks::render(files, settings, catalog, OVERLAY_SOURCES[region.index()])?;
    if !settings.item_colors {
        let mut profile = read_document(files, PROFILE_PATH)?;
        catalog
            .native_item_colors
            .restore_bindings(region, &mut profile)?;
        write_document(files, PROFILE_PATH, &profile)?;
    }
    if !settings.rune_icons {
        icons::remove_rune_overrides(files, &catalog.runes);
    }
    beams::render(files, catalog, settings, region)
}
