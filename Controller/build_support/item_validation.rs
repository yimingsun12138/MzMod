//! Check the item catalog against both regions before embedding its source bytes.
use std::{collections::BTreeSet, fs, io, path::Path};

use crate::archive::RegionalResources;
use crate::charm_routes::check_private_charm_routes;
use crate::ground_model_validation::validate_ground_models;
use crate::item_catalog::{
    inventory_icon_pair, inventory_icon_path, string_bank_path, ItemCatalog, CHINESE_OVERLAY,
};
use crate::item_colors::PROFILE_PATH;
use crate::jewel_routes::JewelRouteTable;
use crate::label_validation::validate_labels;
use crate::output_paths::{FileSource, OutputPaths};
use crate::region::Region;
use crate::string_banks::TemplateStringBanks;

pub(super) fn compile_item_catalog(
    template: &Path,
    resources: &RegionalResources,
    jewel_icons: &RegionalResources,
    banks: &mut TemplateStringBanks<'_>,
    paths: &mut OutputPaths,
    output: &Path,
) -> io::Result<ItemCatalog> {
    let source = fs::read(template.join("items.json"))?;
    crate::game_json::validate(&source)
        .map_err(|error| io::Error::other(format!("items.json: {error}")))?;
    let catalog: ItemCatalog = serde_json::from_slice(&source)
        .map_err(|error| io::Error::other(format!("items.json: {error}")))?;
    for (slot, rune) in catalog.runes.iter().enumerate() {
        if rune.english.trim().is_empty() || rune.chinese.trim().is_empty() {
            return Err(io::Error::other(format!(
                "items.json: missing display name for rune #{}",
                slot + 1
            )));
        }
    }
    for region in Region::ALL {
        for bank in catalog
            .labels
            .keys()
            .map(String::as_str)
            .chain([CHINESE_OVERLAY])
        {
            paths.claim(
                region,
                &string_bank_path(bank),
                FileSource::Template,
                "item labels",
            )?;
        }
        paths.claim(
            region,
            PROFILE_PATH,
            FileSource::Template,
            "equipment colors",
        )?;
    }
    validate_labels(&catalog, banks)?;
    validate_ground_models(&catalog, resources, paths)?;
    validate_item_routes(&catalog, resources, paths)?;
    validate_inventory_icons(&catalog, jewel_icons, paths)?;
    fs::write(output.join("item-catalog.json"), source)?;
    Ok(catalog)
}

fn validate_item_routes(
    catalog: &ItemCatalog,
    resources: &RegionalResources,
    paths: &mut OutputPaths,
) -> io::Result<()> {
    for region in Region::ALL {
        for table in [JewelRouteTable::Base, JewelRouteTable::Unique] {
            let template = resources
                .find(region, table.path())
                .map(fs::read)
                .transpose()?;
            let source = if template.is_some() {
                FileSource::Template
            } else {
                FileSource::Generated
            };
            paths.claim(region, table.path(), source, "jewel routes")?;
            let context = |error| io::Error::other(format!("{}: {error}", region.code()));
            let check_charms = |document: &serde_json::Value| {
                if matches!(table, JewelRouteTable::Unique) {
                    check_private_charm_routes(document, &catalog.charms).map_err(|error| {
                        io::Error::other(format!("{}: {}: {error}", region.code(), table.path()))
                    })?;
                }
                Ok::<(), io::Error>(())
            };
            let document = table
                .source(&catalog.jewels, template.as_deref())
                .map_err(context)?;
            check_charms(&document)?;
            let (redirected, models) = table
                .redirected(&catalog.jewels, document)
                .map_err(context)?;
            check_charms(&redirected)?;
            for model in models {
                if !catalog.jewels.models.contains_key(model) {
                    return Err(io::Error::other(format!(
                        "{}: {} refers to unknown jewel model {model}",
                        region.code(),
                        table.path()
                    )));
                }
            }
        }
    }
    Ok(())
}

fn validate_inventory_icons(
    catalog: &ItemCatalog,
    jewel_icons: &RegionalResources,
    paths: &mut OutputPaths,
) -> io::Result<()> {
    let mut sprites = BTreeSet::new();
    for (route, model) in &catalog.jewels.models {
        for sprite in model.inventory_sprites(route)? {
            if !sprite
                .rsplit_once('.')
                .is_some_and(|(_, ext)| ext.eq_ignore_ascii_case("sprite"))
            {
                return Err(io::Error::other(format!(
                    "items.json: expected a jewel inventory sprite, found {sprite:?}"
                )));
            }
            for region in Region::ALL {
                jewel_icons.resolve(region, &sprite)?;
            }
            sprites.insert(sprite);
        }
    }
    for region in Region::ALL {
        for (index, rune) in catalog.runes.iter().enumerate() {
            for path in inventory_icon_pair(&rune.route) {
                paths.claim(
                    region,
                    &path,
                    FileSource::Template,
                    format_args!("numbered rune icons (#{})", index + 1),
                )?;
            }
        }
        for route in [
            &catalog.charms.annihilus_route,
            &catalog.charms.hellfire_torch_route,
        ] {
            for path in inventory_icon_pair(route) {
                paths.claim(
                    region,
                    &path,
                    FileSource::Template,
                    "unique charm appearance",
                )?;
            }
        }
        for sprite in &sprites {
            paths.claim(
                region,
                &inventory_icon_path(sprite),
                FileSource::Generated,
                "jewel inventory icons",
            )?;
        }
        if let Some(unused) = jewel_icons
            .game_paths(region)
            .find(|&path| !sprites.contains(path))
        {
            return Err(io::Error::other(format!(
                "{}: undeclared jewel inventory sprite {unused}",
                region.code()
            )));
        }
    }
    Ok(())
}
