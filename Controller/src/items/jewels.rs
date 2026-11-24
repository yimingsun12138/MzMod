use std::collections::BTreeSet;

use anyhow::{Context, Result};

use super::icons::install_jewel_icons;
use crate::item_catalog::Jewels;
use crate::jewel_routes::JewelRouteTable;
use crate::resources::write_document;
use crate::{ModFiles, Region, Settings};

pub(super) fn install_routes_and_icons<'a>(
    files: &mut ModFiles,
    jewels: &'a Jewels,
    settings: &Settings,
    region: Region,
) -> Result<BTreeSet<&'a str>> {
    let mut models = BTreeSet::new();
    for (table, enabled) in [
        (JewelRouteTable::Base, settings.jewel_beam),
        (JewelRouteTable::Unique, settings.unique_jewel_beam),
    ] {
        if enabled {
            let path = table.path();
            let source = table.source(jewels, files.get(path).map(Vec::as_slice))?;
            let (document, required_models) = table.redirected(jewels, source)?;
            models.extend(required_models);
            write_document(files, path, &document)?;
        }
    }
    if models.is_empty() {
        return Ok(models);
    }

    let mut icons = BTreeSet::new();
    for &route in &models {
        let model = jewels
            .models
            .get(route)
            .with_context(|| format!("missing private jewel model {route}"))?;
        icons.extend(model.inventory_sprites(route)?);
    }
    install_jewel_icons(files, icons, region)?;
    Ok(models)
}
