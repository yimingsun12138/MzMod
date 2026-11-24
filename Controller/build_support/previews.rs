use std::{collections::BTreeMap, fs, io, path::Path};

use serde::Deserialize;
use serde_json::Value;

use crate::archive::RegionalResources;
use crate::item_catalog::{localized_text, ItemCatalog};
use crate::item_colors::{
    resolve_rgba, EquipmentColors, ETHEREAL_FIELD, PROFILE_PATH, SOCKETED_FIELD,
};
use crate::preview_items::{
    PreviewItems, PreviewLabel, PreviewLanguage, RegionalPreview, RunePreview, TooltipColors,
};
use crate::region::Region;
use crate::rune_preview_icons::RuneIconCompiler;
use crate::string_banks::TemplateStringBanks;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ItemReference {
    bank: String,
    id: u64,
    key: String,
}

impl ItemReference {
    fn resolve_label(
        &self,
        region: Region,
        banks: &mut TemplateStringBanks<'_>,
    ) -> io::Result<PreviewLabel> {
        let row = banks.lookup(region, &self.bank, self.id, &self.key)?;
        for language in PreviewLanguage::ALL {
            localized_text(row, language.locale()).map_err(|error| {
                io::Error::other(format!(
                    "{}: preview {}/{}/{}/{error}",
                    region.code(),
                    self.bank,
                    self.id,
                    self.key
                ))
            })?;
        }
        Ok(PreviewLabel {
            bank: self.bank.clone(),
            row: row.clone(),
        })
    }

    fn rune_route<'a>(&self, catalog: &'a ItemCatalog) -> io::Result<&'a str> {
        let label = catalog
            .label(&self.bank, self.id, &self.key)
            .ok_or_else(|| io::Error::other("missing rune label in item catalog"))?;
        let (_, rune) = label
            .resolve_rune_label(catalog)?
            .ok_or_else(|| io::Error::other("expected a rune label"))?;
        Ok(&rune.route)
    }
}

fn tooltip_field(profile: &Value, field: &str) -> io::Result<[u8; 4]> {
    resolve_rgba(profile, &profile["TooltipStyle"][field])
        .map_err(|error| io::Error::other(format!("TooltipStyle.{field}: {error}")))
}

fn tooltip_colors(profile: &Value) -> io::Result<TooltipColors> {
    let font_color = |name: &str| {
        resolve_rgba(profile, &profile[name])
            .map_err(|error| io::Error::other(format!("{name}: {error}")))
    };
    let inline = [
        ('0', "FontColorWhite"),
        ('1', "FontColorRed"),
        ('2', "FontColorGreen"),
        ('3', "FontColorBlue"),
        ('4', "FontColorGoldYellow"),
        ('5', "FontColorGray"),
        ('6', "FontColorBlack"),
        ('8', "FontColorOrange"),
        ('9', "FontColorYellow"),
    ]
    .into_iter()
    .map(|(code, name)| Ok((code, font_color(name)?)))
    .collect::<io::Result<_>>()?;
    Ok(TooltipColors {
        default_item: tooltip_field(profile, "DefaultColor")?,
        rune: tooltip_field(profile, "RuneColor")?,
        runeword: font_color("FontColorGoldYellow")?,
        inline,
    })
}

fn equipment_colors(profile: &Value) -> io::Result<EquipmentColors<[u8; 4]>> {
    Ok(EquipmentColors {
        ethereal: tooltip_field(profile, ETHEREAL_FIELD)?,
        socketed: tooltip_field(profile, SOCKETED_FIELD)?,
    })
}

pub(super) fn write_previews(
    catalog: &ItemCatalog,
    resources: &RegionalResources,
    banks: &mut TemplateStringBanks<'_>,
    output: &Path,
) -> io::Result<()> {
    let manifest = "assets/ui/preview-items.json";
    println!("cargo:rerun-if-changed={manifest}");
    let selected: PreviewItems<ItemReference> = serde_json::from_reader(fs::File::open(manifest)?)?;
    let mut icons = RuneIconCompiler::new(resources)?;
    let mut regional_examples = BTreeMap::new();
    for region in Region::ALL {
        let [left_rune, right_rune] = &selected.runes;
        let mut compile_rune = |item: &ItemReference| -> io::Result<RunePreview> {
            let variants = item
                .rune_route(catalog)
                .and_then(|route| icons.compile(region, route))
                .map_err(|error| {
                    io::Error::other(format!(
                        "{manifest}: {}: {}/{}/{}: {error}",
                        region.code(),
                        item.bank,
                        item.id,
                        item.key
                    ))
                })?;
            Ok(RunePreview {
                label: item.resolve_label(region, banks)?,
                icons: variants,
            })
        };
        let runes = [compile_rune(left_rune)?, compile_rune(right_rune)?];
        let profile_path = resources.resolve(region, PROFILE_PATH)?;
        let mut profile: Value = serde_json::from_reader(fs::File::open(profile_path)?)?;
        if profile
            .pointer("/TooltipStyle/fontStyle/options/newlineHandling")
            .and_then(Value::as_str)
            != Some("reversed")
        {
            return Err(io::Error::other(format!(
                "{}/{PROFILE_PATH}: item labels and previews require reversed TooltipStyle.fontStyle.options.newlineHandling",
                region.code()
            )));
        }
        let tooltip_colors = tooltip_colors(&profile).map_err(|error| {
            io::Error::other(format!("{}/{PROFILE_PATH}: {error}", region.code()))
        })?;
        let colors = |profile: &Value| {
            equipment_colors(profile)
                .map_err(|error| io::Error::other(format!("{}: {error}", region.code())))
        };
        let enhanced_colors = colors(&profile)?;
        catalog
            .native_item_colors
            .restore_bindings(region, &mut profile)
            .map_err(|error| io::Error::other(format!("{}: {error}", region.code())))?;
        let native_colors = colors(&profile)?;
        regional_examples.insert(
            region.code(),
            RegionalPreview {
                items: PreviewItems {
                    runeword: selected.runeword.resolve_label(region, banks)?,
                    shield: selected.shield.resolve_label(region, banks)?,
                    sword: selected.sword.resolve_label(region, banks)?,
                    runes,
                },
                tooltip_colors,
                enhanced_colors,
                native_colors,
            },
        );
    }
    serde_json::to_writer(
        fs::File::create(output.join("item-previews.json"))?,
        &regional_examples,
    )?;
    icons.write(output)
}
