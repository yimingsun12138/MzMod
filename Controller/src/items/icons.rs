use std::collections::BTreeSet;

use anyhow::{Context, Result};

use crate::item_catalog::{inventory_icon_pair, inventory_icon_path, Rune};
use crate::template::read_regional_files;
use crate::{ModFiles, Region};

pub(super) fn remove_rune_overrides(files: &mut ModFiles, runes: &[Rune]) {
    for rune in runes {
        for path in inventory_icon_pair(&rune.route) {
            files.remove(&path);
        }
    }
}

pub(super) fn install_jewel_icons(
    files: &mut ModFiles,
    icons: BTreeSet<String>,
    region: Region,
) -> Result<()> {
    let archive = include_bytes!(concat!(env!("OUT_DIR"), "/jewel-icons.tar.gz"));
    let mut sprites = read_regional_files(archive, region)?;
    for icon in icons {
        let bytes = sprites
            .remove(&icon)
            .with_context(|| format!("missing {} jewel icon {icon}", region.code()))?;
        files.insert(inventory_icon_path(&icon), bytes);
    }
    Ok(())
}
