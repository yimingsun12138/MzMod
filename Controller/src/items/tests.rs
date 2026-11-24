use super::*;
use crate::resources::read_document;
use crate::template::RegionalTemplate;
use serde_json::Value;

fn bank(files: &ModFiles, bank: &str) -> Value {
    read_document(files, &format!("data/local/lng/strings/{bank}.json")).unwrap()
}

fn row<'a>(doc: &'a Value, key: &str) -> &'a Value {
    doc.as_array()
        .unwrap()
        .iter()
        .find(|r| r["Key"] == key)
        .unwrap()
}

#[test]
fn template_beams_preserve_bytes_and_drop_selections_are_per_item() -> Result<()> {
    for region in [Region::Int, Region::Cn] {
        let original = RegionalTemplate::read(region)?.files;
        let mut files = original.clone();
        let template = Settings {
            rune_beams: std::array::from_fn(|index| index >= 24),
            small_charm_beam: false,
            large_charm_beam: false,
            grand_charm_beam: false,
            jewel_beam: false,
            unique_jewel_beam: false,
            ..crate::enabled_features()
        };
        render(&mut files, &template, region)?;
        assert_eq!(files, original);
        let mut settings = Settings {
            annihilus_beam: false,
            rune_icons: false,
            ..template
        };
        settings.rune_beams[..24].fill(true);
        settings.rune_beams[24] = false;
        // A selectable beam can be rendered with no Gul model in the input.
        files.remove("data/hd/items/misc/rune/gul_rune.json");
        render(&mut files, &settings, region)?;
        for rune in &embedded_catalog().runes[..24] {
            let path = format!(
                "data/hd/items/misc/rune/{}_rune.json",
                rune.english.to_ascii_lowercase()
            );
            let model = read_document(&files, &path)?;
            let Some(native) = &rune.native_ground_model else {
                panic!("expected native model for {}", rune.route);
            };
            let entities = model["entities"].as_array().unwrap();
            let original_entities = native["entities"].as_array().unwrap();
            assert_eq!(&entities[..entities.len() - 1], original_entities);
            assert_eq!(entities.last().unwrap()["name"], "mzmod_drop_beam_0");
            assert_eq!(model["name"], native["name"]);
            assert_eq!(
                model["dependencies"]["particles"].as_array().unwrap().len(),
                1
            );
        }
        assert!(!files.contains_key("data/hd/items/misc/rune/gul_rune.json"));
        assert!(!files
            .keys()
            .any(|k| k.starts_with("data/hd/global/ui/items/misc/rune/")));
        let charm = read_document(&files, "data/hd/items/misc/quest/mzmod_annihilus.json")?;
        assert!(!charm["entities"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["name"] == "mzmod_drop_beam_0"));
        for path in [
            "data/hd/items/misc/torch/mzmod_hellfire_torch.json",
            "data/hd/items/uniques.json",
        ] {
            assert_eq!(files[path], original[path]);
        }
    }
    Ok(())
}

#[test]
fn item_catalog_requires_both_windows_display_names_on_every_build_host() -> Result<()> {
    let source: Value = serde_json::from_slice(ITEM_CATALOG_BYTES)?;
    for field in ["english", "chinese"] {
        let mut missing = source.clone();
        let ber = missing["runes"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|rune| rune["number"] == 30)
            .unwrap();
        ber.as_object_mut().unwrap().remove(field);
        let error = serde_json::from_value::<ItemCatalog>(missing)
            .err()
            .expect("missing display name");
        assert!(error.to_string().contains(field), "{error}");
    }
    Ok(())
}

#[test]
fn a_repeated_label_identity_cannot_replace_its_existing_rules() -> Result<()> {
    let mut source: Value = serde_json::from_slice(ITEM_CATALOG_BYTES)?;
    source["labels"] = Value::Null;
    let source = serde_json::to_string(&source)?;
    for (labels, expected) in [
        (
            r#"{"item-names":{},"item-names":{}}"#,
            r#"duplicate key "item-names""#,
        ),
        (
            r#"{"item-names":{"2574":{"The Gnasher":{}},"2574":{}}}"#,
            "item-names: duplicate key 2574",
        ),
        (
            r#"{"item-names":{"2574":{"The Gnasher":{"name_lines":["max_hint"]},"The Gnasher":{}}}}"#,
            r#"item-names: 2574: duplicate key "The Gnasher""#,
        ),
        (
            r#"{"item-names":{"2574":{"The Gnasher":{"name_linse":["max_hint"]}}}}"#,
            "item-names: 2574: The Gnasher: unknown field",
        ),
    ] {
        let malformed = source.replacen(r#""labels":null"#, &format!(r#""labels":{labels}"#), 1);
        let error = serde_json::from_str::<ItemCatalog>(&malformed)
            .err()
            .expect("malformed label declaration");
        assert!(error.to_string().contains(expected), "{error}");
    }
    Ok(())
}

#[test]
fn item_declarations_validate_their_own_fields_and_preserve_native_model_fields() -> Result<()> {
    let mut source: Value = serde_json::from_slice(ITEM_CATALOG_BYTES)?;
    source["runes"][0]["ground_model"]["futureClientField"] = true.into();
    let catalog: ItemCatalog = serde_json::from_value(source.clone())?;
    assert_eq!(
        catalog.runes[0].native_ground_model.as_ref().unwrap()["futureClientField"],
        true
    );
    let rune = source["runes"][0].as_object_mut().unwrap();
    let native = rune.remove("ground_model").unwrap();
    rune.insert("ground_models".into(), native);
    let error = serde_json::from_value::<ItemCatalog>(source)
        .err()
        .expect("misspelled rune model declaration");
    assert!(error.to_string().contains("ground_models"), "{error}");

    let duplicate_locale = r#"{"suffixes":{"enUS":{"full":"|ELI","tier":"|ELI","weight":""},"enUS":{"full":"","tier":"","weight":""}}}"#;
    let error = serde_json::from_str::<crate::item_catalog::BaseGroup>(duplicate_locale)
        .err()
        .expect("duplicate suffix declaration");
    assert!(
        error.to_string().contains(r#"duplicate key "enUS""#),
        "{error}"
    );
    Ok(())
}

#[test]
fn a_saved_rune_choice_uses_the_game_number_after_the_catalog_is_reordered() -> Result<()> {
    let mut source: Value = serde_json::from_slice(ITEM_CATALOG_BYTES)?;
    source["runes"].as_array_mut().unwrap().reverse();
    let reordered: ItemCatalog = serde_json::from_value(source)?;
    let mut settings = Settings::default();
    settings.rune_beams.fill(false);
    settings.rune_beams[29] = true;
    let settings = serde_json::from_value(serde_json::to_value(settings)?)?;
    for region in Region::ALL {
        let mut files = RegionalTemplate::read(region)?.files;
        beams::render(&mut files, &reordered, &settings, region)?;
        assert!(files.contains_key("data/hd/items/misc/rune/ber_rune.json"));
        assert!(!files.contains_key("data/hd/items/misc/rune/ort_rune.json"));
    }
    Ok(())
}

#[test]
fn toggling_a_beam_preserves_a_particle_used_by_the_original_appearance() -> Result<()> {
    use crate::resources::write_document;
    use serde_json::json;

    let catalog = embedded_catalog();
    let path = crate::item_catalog::ground_model_path(&catalog.charms.annihilus_route);
    for region in Region::ALL {
        let mut files = RegionalTemplate::read(region)?.files;
        let mut appearance = read_document(&files, &path)?;
        let particle = appearance["dependencies"]["particles"][0]["path"].clone();
        appearance["dependencies"]["particles"][0]["metadata"] = json!("keep");
        appearance["entities"].as_array_mut().unwrap().push(json!({
            "id": 10, "name": "appearance_glow",
            "components": [{"type": "VfxDefinitionComponent", "filename": particle}]
        }));
        write_document(&mut files, &path, &appearance)?;
        appearance["entities"]
            .as_array_mut()
            .unwrap()
            .retain(|entity| entity != &catalog.beam);
        let settings = Settings {
            annihilus_beam: false,
            ..Settings::default()
        };
        beams::render(&mut files, catalog, &settings, region)?;
        assert_eq!(read_document(&files, &path)?, appearance);
    }
    Ok(())
}

#[test]
fn disabling_item_colors_preserves_shared_profile_and_panel_colors() -> Result<()> {
    use crate::resources::write_document;
    use serde_json::json;

    let settings = Settings {
        item_colors: false,
        ..Settings::default()
    };
    for region in Region::ALL {
        let mut files = RegionalTemplate::read(region)?.files;
        let mut profile = read_document(&files, PROFILE_PATH)?;
        profile["PanelTitleColor"] = json!("$MzModEtherealNameColor");
        let panel = "data/global/ui/layouts/sharedcolorpanelhd.json";
        let panel_style = json!({"titleColor":"$PanelTitleColor","fontColor":"$FontColorSocketed"});
        write_document(&mut files, panel, &panel_style)?;
        write_document(&mut files, PROFILE_PATH, &profile)?;
        let mut expected = profile;
        expected["TooltipStyle"]["EtherealColor"] = json!("$FontColorGrey");
        expected["TooltipStyle"]["SocketedColor"] = json!("$FontColorGrey");
        render(&mut files, &settings, region)?;
        assert_eq!(read_document(&files, PROFILE_PATH)?, expected);
        assert_eq!(read_document(&files, panel)?, panel_style);
    }
    Ok(())
}

#[test]
fn restoring_native_colors_handles_regional_aliases_and_literal_values() -> Result<()> {
    use crate::item_colors::RegionalItemColors;
    use serde_json::json;

    let gray = json!({"reference":"$OriginalGray", "rgba":[99,99,99,255]});
    let literal = json!({"rgba":[7,13,23,42]});
    let colors: RegionalItemColors = serde_json::from_value(json!({
        "INT":{"ethereal":gray, "socketed":literal},
        "CN":{"ethereal":literal, "socketed":gray}
    }))?;
    let profile = json!({
        "TooltipStyle":{"EtherealColor":"$Accent", "SocketedColor":"$Accent"},
        "OriginalGray":"$Gray", "Gray":{"r":99,"g":99,"b":99,"a":255},
        "Accent":{"r":128,"g":255,"b":255,"a":255}, "PanelTitleColor":"$OriginalGray"
    });
    let native_literal = json!({"r":7,"g":13,"b":23,"a":42});
    let mut international = profile.clone();
    colors.restore_bindings(Region::Int, &mut international)?;
    assert_eq!(
        international["TooltipStyle"]["EtherealColor"],
        "$OriginalGray"
    );
    assert_eq!(
        international["TooltipStyle"]["SocketedColor"],
        native_literal
    );

    let mut china = profile;
    china["Gray"] = json!("$Accent");
    let mut expected = china.clone();
    expected["TooltipStyle"] = json!({
        "EtherealColor": native_literal, "SocketedColor":{"r":99,"g":99,"b":99,"a":255}
    });
    colors.restore_bindings(Region::Cn, &mut china)?;
    assert_eq!(china, expected);
    Ok(())
}

#[test]
fn item_and_rune_options_compose_across_the_locales() -> Result<()> {
    for region in [Region::Int, Region::Cn] {
        let original = RegionalTemplate::read(region)?.files;
        let mut settings = Settings {
            english_names: false,
            base_tiers: false,
            rune_numbers: false,
            item_colors: false,
            ..crate::enabled_features()
        };
        let mut files = original.clone();
        render(&mut files, &settings, region)?;
        let runes = bank(&files, "item-runes");
        let spirit = row(&runes, "Runeword130");
        assert_eq!(spirit["zhTW"], "ÿc4[MAX:35FCR/112Mana/8MA]\n精神");
        assert_eq!(spirit["enUS"], "ÿc4[MAX:35FCR/112Mana/8MA]\nÿc4Spirit");
        assert_eq!(row(&runes, "r24")["zhTW"], "ÿc5[x2+藍寶石x1]\nÿc8伊司特");
        assert_eq!(row(&runes, "r24L")["zhTW"], "ÿc8伊司特");
        let names = bank(&files, "item-names");
        assert_eq!(row(&names, "uit")["zhTW"], "君主盾|輕");
        assert_eq!(row(&names, "uit")["enUS"], "Monarch [L]");
        assert!(!row(&names, "crs")["zhTW"].as_str().unwrap().contains('|'));
        let overlay = bank(&files, "chinese-overlay");
        assert_eq!(row(&overlay, "bhm")["zhCN"], row(&names, "bhm")["zhTW"]);
        let profile = read_document(&files, "data/global/ui/layouts/_profilehd.json")?;
        for field in ["SocketedColor", "EtherealColor"] {
            assert_eq!(profile["TooltipStyle"][field], "$FontColorGrey");
        }
        settings.english_names = true;
        settings.max_hints = false;
        settings.base_tiers = true;
        settings.armor_weights = false;
        settings.rune_numbers = true;
        settings.rune_recipes = false;
        let mut files = original;
        render(&mut files, &settings, region)?;
        let runes = bank(&files, "item-runes");
        assert_eq!(row(&runes, "Runeword130")["zhTW"], "ÿc4Spirit\n精神");
        assert_eq!(row(&runes, "r24")["zhTW"], "ÿc8伊司特 Ist #24");
        let names = bank(&files, "item-names");
        assert_eq!(row(&names, "uit")["zhTW"], "君主盾|精");
        assert_eq!(row(&names, "uit")["enUS"], "Monarch|ELI");
    }
    Ok(())
}

#[test]
fn base_options_follow_each_items_own_language_fields() -> Result<()> {
    use crate::item_catalog::string_bank_path;
    use crate::resources::write_document;
    use serde_json::json;

    let mut catalog: ItemCatalog = serde_json::from_slice(ITEM_CATALOG_BYTES)?;
    let group = catalog
        .label("item-names", 20366, "uit")
        .unwrap()
        .base_suffix_group
        .unwrap();
    catalog.bases[group].suffixes.insert(
        "nlNL".into(),
        serde_json::from_value(json!({
            "full": " <X,L>", "tier": " <X>", "weight": " <L>"
        }))?,
    );
    let sword = json!({"id":2005, "Key":"crs", "deDE":"[ns]Kristallschwert [N]"});
    for region in Region::ALL {
        let mut shield =
            json!({"id":20366, "Key":"uit", "deDE":"[ms]Monarch [X]|[L]", "note":"extra metadata"});
        if region == Region::Cn {
            shield["nlNL"] = "Monarch <X,L>".into();
        }
        let mut original = ModFiles::new();
        write_document(
            &mut original,
            &string_bank_path("item-names"),
            &json!([shield, sword]),
        )?;
        write_document(&mut original, &string_bank_path("item-runes"), &json!([]))?;
        for (base_tiers, armor_weights, german, dutch, sword_suffix) in [
            (true, true, " [X]|[L]", " <X,L>", " [N]"),
            (true, false, " [X]", " <X>", " [N]"),
            (false, true, " [L]", " <L>", ""),
            (false, false, "", "", ""),
        ] {
            let settings = Settings {
                base_tiers,
                armor_weights,
                ..crate::enabled_features()
            };
            let mut files = original.clone();
            string_banks::render(&mut files, &settings, &catalog, &[])?;
            let names = bank(&files, "item-names");
            let shield = row(&names, "uit");
            assert_eq!(shield["deDE"], format!("[ms]Monarch{german}"));
            assert_eq!(shield["note"], "extra metadata");
            if region == Region::Cn {
                assert_eq!(shield["nlNL"], format!("Monarch{dutch}"));
            } else {
                assert!(shield.get("nlNL").is_none());
            }
            assert_eq!(
                row(&names, "crs")["deDE"],
                format!("[ns]Kristallschwert{sword_suffix}")
            );
        }
    }
    Ok(())
}

#[test]
fn invalid_native_base_translations_fail_when_rendered() -> Result<()> {
    use crate::item_catalog::string_bank_path;
    use crate::resources::write_document;
    use serde_json::json;

    let settings = Settings {
        base_tiers: false,
        ..crate::enabled_features()
    };
    for (locale, value, expected) in [
        ("nlNL", json!("Monarch"), "missing base suffix declaration"),
        ("deDE", json!(7), "expected a string"),
        ("deDE", json!("Monarch"), "unexpected base suffix"),
        (
            "deDE",
            json!(" [X]|[L]"),
            "missing name before the base suffix",
        ),
    ] {
        let mut files = ModFiles::new();
        write_document(
            &mut files,
            &string_bank_path("item-names"),
            &json!([
                {"id":20366, "Key":"uit", locale:value}
            ]),
        )?;
        let error =
            string_banks::render(&mut files, &settings, embedded_catalog(), &[]).unwrap_err();
        let message = format!("{error:#}");
        assert!(
            message.contains(locale) && message.contains(expected),
            "{message}"
        );
    }
    Ok(())
}

#[test]
fn optional_lines_follow_the_declared_order_and_leave_the_native_name_intact() -> Result<()> {
    use crate::item_catalog::LabelRecord;
    use serde_json::json;
    let record: LabelRecord = serde_json::from_value(json!({
        "name_lines": ["english_name", "max_hint"]
    }))?;
    let settings = Settings {
        max_hints: false,
        ..crate::enabled_features()
    };
    let renderer = labels::LabelRenderer::new(embedded_catalog(), (&settings).into());
    let label = renderer.bind(&record)?;
    assert_eq!(
        label.text("zhTW", "Spirit\n[35 FCR]\nLocal name\nNative detail")?,
        "Spirit\nLocal name\nNative detail"
    );
    assert_eq!(label.text("enUS", "[35 FCR]\nSpirit")?, "Spirit");

    let repeated: LabelRecord = serde_json::from_value(json!({
        "name_lines": ["max_hint", "max_hint"]
    }))?;
    let error = renderer
        .bind(&repeated)?
        .text("enUS", "[35 FCR]\nSpirit\nNative detail")
        .unwrap_err();
    assert!(error
        .to_string()
        .contains("repeated optional item name role"));
    Ok(())
}

#[test]
fn name_toggles_follow_bank_id_and_key_when_colors_change() -> Result<()> {
    use crate::resources::write_document;
    use serde_json::json;

    let mut catalog: ItemCatalog = serde_json::from_slice(ITEM_CATALOG_BYTES)?;
    let sibling_id = 90001;
    catalog.labels.get_mut("item-runes").unwrap().by_id.insert(
        sibling_id,
        serde_json::from_value(json!({"Runeword130": {"name_lines": ["max_hint"]}}))?,
    );
    let spirit = json!({
        "id": 20635, "Key": "Runeword130",
        "enUS": "ÿc1[35 FCR]\nÿc3Spirit",
        "zhTW": "ÿc1[35 FCR]\nÿc3Spirit\n精神",
        "zhCN": "ÿc1[35 FCR]\nÿc3Spirit\n精神",
    });
    let mut sibling = spirit.clone();
    sibling["id"] = sibling_id.into();
    let unlisted_id = json!({"id": 90002, "Key": "Runeword130", "frFR": "Esprit"});
    let mut unlisted_key = spirit.clone();
    unlisted_key["Key"] = "other".into();

    let mut files = ModFiles::new();
    for (bank, rows) in [
        (
            "item-runes",
            json!([spirit, sibling, unlisted_id, unlisted_key]),
        ),
        ("item-names", json!([spirit])),
        ("chinese-overlay", json!([])),
    ] {
        write_document(
            &mut files,
            &format!("data/local/lng/strings/{bank}.json"),
            &rows,
        )?;
    }
    let settings = Settings {
        english_names: false,
        max_hints: false,
        ..crate::enabled_features()
    };
    string_banks::render(&mut files, &settings, &catalog, &[])?;
    let runes = bank(&files, "item-runes");
    assert_eq!(runes[0]["enUS"], "ÿc3Spirit");
    assert_eq!(runes[0]["zhTW"], "精神");
    assert_eq!(runes[1]["zhTW"], "ÿc3Spirit\n精神");
    assert_eq!(runes[2], unlisted_id);
    assert_eq!(runes[3], unlisted_key);
    assert_eq!(bank(&files, "item-names")[0], spirit);
    Ok(())
}

#[test]
fn only_requested_overlay_rows_require_a_unique_source_across_banks() -> Result<()> {
    use crate::chinese_overlay::{primary_bank_name, OverlaySources};
    use crate::resources::write_document;
    use serde_json::json;

    let mut catalog: ItemCatalog = serde_json::from_slice(ITEM_CATALOG_BYTES)?;
    let role = json!({ "1": { "shared": { "name_lines": ["max_hint"] } } });
    catalog.labels = serde_json::from_value(json!({
        "item-names": role,
        "item-modifiers": role,
    }))?;
    let settings = Settings {
        max_hints: false,
        ..crate::enabled_features()
    };
    let label = |name: &str| {
        json!({
            "id": 1, "Key": "shared",
            "enUS": format!("MAX\n{name}"), "zhTW": format!("MAX\n{name}"), "zhCN": format!("MAX\n{name}"),
        })
    };
    let mut files = ModFiles::new();
    let path = |bank: &str| format!("data/local/lng/strings/{bank}.json");
    let independent = json!({"id": 2, "Key": "shared", "zhCN": "Story"});
    for (bank, rows) in [
        ("item-names", json!([label("Name")])),
        ("item-modifiers", json!([label("Modifier")])),
        ("chinese-overlay", json!([independent])),
    ] {
        write_document(&mut files, &path(bank), &rows)?;
    }
    let mut rendered = files.clone();
    let compile_sources = |files: &ModFiles| -> Result<OverlaySources> {
        let mut sources = OverlaySources::from_overlay(&bank(files, "chinese-overlay"))?;
        for (file_path, bytes) in files {
            if let Some(bank_name) = primary_bank_name(file_path) {
                sources.index_bank(bank_name, bytes)?;
            }
        }
        Ok(sources)
    };
    let sources = compile_sources(&files)?;
    string_banks::render(&mut rendered, &settings, &catalog, &sources.entries())?;
    assert_eq!(bank(&rendered, "item-names")[0]["zhTW"], "Name");
    assert_eq!(bank(&rendered, "item-modifiers")[0]["zhTW"], "Modifier");
    assert_eq!(
        rendered[&path("chinese-overlay")],
        files[&path("chinese-overlay")]
    );

    let requested = json!({"id": 1, "Key": "shared", "zhCN": "Original", "deDE": "Other locale"});
    let repeated = json!({"id": 1, "Key": "shared", "zhCN": "Alternate", "variant": 2});
    write_document(
        &mut files,
        &path("chinese-overlay"),
        &json!([requested, independent, repeated]),
    )?;
    catalog.labels.remove("item-names");
    let error = compile_sources(&files)
        .err()
        .expect("competing owners")
        .to_string();
    assert!(
        error.contains("1/shared")
            && error.contains("item-modifiers")
            && error.contains("item-names"),
        "{error}"
    );
    files.remove(&path("item-names"));
    let sources = compile_sources(&files)?;
    let entries = sources.entries();
    assert_eq!(entries.as_slice(), &[("item-modifiers", 1, "shared")]);
    string_banks::render(&mut files, &settings, &catalog, &entries)?;
    let overlay = bank(&files, "chinese-overlay");
    for (index, mut expected) in [(0, requested), (2, repeated)] {
        expected["zhCN"] = "Modifier".into();
        assert_eq!(overlay[index], expected);
    }
    assert_eq!(overlay[1], independent);
    Ok(())
}

#[test]
fn writable_overlay_translations_require_a_string_in_each_target_row() -> Result<()> {
    use crate::chinese_overlay::OverlaySources;
    use serde_json::json;

    let rows = json!([
        {"id": 1, "Key": "shared", "zhCN": "Primary"},
        {"id": 1, "Key": "shared", "zhCN": {"variant": 2}},
        {"id": 2, "Key": "shared", "frFR": "Story"},
    ]);
    let mut sources = OverlaySources::from_overlay(&rows)?;
    sources.index_bank("item-names", br#"[{"id":1,"Key":"shared"}]"#)?;
    sources.validate_target("item-modifiers", 1, "shared")?;
    sources.validate_target("item-names", 2, "shared")?;
    let error = sources
        .validate_target("item-names", 1, "shared")
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("1/shared") && error.contains("zhCN"),
        "{error}"
    );
    Ok(())
}

fn route<'a>(list: &'a Value, key: &str) -> &'a Value {
    list.as_array()
        .unwrap()
        .iter()
        .find_map(|r| r.get(key))
        .unwrap()
}

#[test]
fn private_charm_routes_match_the_selected_items_on_every_tier() -> Result<()> {
    let charms = &embedded_catalog().charms;
    for region in Region::ALL {
        let original = RegionalTemplate::read(region)?.files;
        let uniques = read_document(&original, "data/hd/items/uniques.json")?;
        let relative = format!("../misc/{}", charms.annihilus_route);
        for slot in crate::item_catalog::ASSET_SLOTS {
            for (key, value, valid) in [
                ("annihilus", relative.as_str(), true),
                ("annihilus", charms.hellfire_torch_route.as_str(), false),
                ("gheeds_fortune", relative.as_str(), false),
            ] {
                let mut changed = uniques.clone();
                let entry = changed
                    .as_array_mut()
                    .unwrap()
                    .iter_mut()
                    .find_map(|row| row.get_mut(key))
                    .unwrap();
                entry[slot] = value.into();
                let result = crate::charm_routes::check_private_charm_routes(&changed, charms);
                if valid {
                    result?;
                } else {
                    let error = result.unwrap_err();
                    assert!(
                        error.to_string().contains(&format!("{key}/{slot}")),
                        "{error}"
                    );
                }
            }
        }
        for (anni, torch) in [(true, false), (false, true)] {
            let files = crate::render_mod(
                region,
                &Settings {
                    annihilus_beam: anni,
                    torch_beam: torch,
                    ..Settings::default()
                },
            )?;
            for (key, expected) in [("annihilus", anni), ("hellfire_torch", torch)] {
                for slot in crate::item_catalog::ASSET_SLOTS {
                    let model = crate::item_catalog::misc_model_route(
                        route(&uniques, key)[slot].as_str().unwrap(),
                    );
                    let model =
                        read_document(&files, &crate::item_catalog::ground_model_path(model))?;
                    assert_eq!(
                        model["entities"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .any(|entity| entity == &embedded_catalog().beam),
                        expected,
                        "{region:?} {key}/{slot}"
                    );
                }
            }
        }
    }
    Ok(())
}

#[test]
fn charm_beams_select_exactly_one_size_and_keep_private_uniques_independent() -> Result<()> {
    let routes = ["charm_small", "charm_medium", "charm_large"];
    for region in [Region::Int, Region::Cn] {
        let original = RegionalTemplate::read(region)?.files;
        for (index, name) in routes.iter().enumerate() {
            let mut files = original.clone();
            let settings = Settings {
                small_charm_beam: index == 0,
                large_charm_beam: index == 1,
                grand_charm_beam: index == 2,
                rune_beams: [false; 33],
                annihilus_beam: false,
                torch_beam: false,
                ..Settings::default()
            };
            render(&mut files, &settings, region)?;
            let path = |name| format!("data/hd/items/misc/charm/{name}.json");
            for other in routes {
                assert_eq!(files.contains_key(&path(other)), *name == other);
            }
            let mut model = read_document(&files, &path(*name))?;
            let beam = model["entities"].as_array_mut().unwrap().pop().unwrap();
            assert_eq!(beam["name"], "mzmod_drop_beam_0");
            let particle = model["dependencies"]["particles"]
                .as_array_mut()
                .unwrap()
                .pop()
                .unwrap();
            assert_eq!(beam["components"][1]["filename"], particle["path"]);
            let charms = &embedded_catalog().charms;
            assert_eq!(
                model,
                [&charms.small, &charms.large, &charms.grand][index].model
            );
            for target in ["quest/mzmod_annihilus", "torch/mzmod_hellfire_torch"] {
                let charm = read_document(&files, &format!("data/hd/items/misc/{target}.json"))?;
                assert!(charm["entities"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .all(|e| e["name"] != "mzmod_drop_beam_0"));
            }
            let uniques = "data/hd/items/uniques.json";
            assert_eq!(files[uniques], original[uniques]);
            assert!(!files.contains_key("data/hd/items/items.json"));
        }
    }
    Ok(())
}

#[test]
fn jewel_beam_redirects_only_the_asset_fields_in_an_existing_base_table() -> Result<()> {
    use crate::resources::write_document;
    use serde_json::json;

    let mut files = ModFiles::new();
    let path = "data/hd/items/items.json";
    let mut expected = json!([
        {"gpw": {"asset": "gem/custom_diamond"}},
        {"cjw": {"asset": "gem/perfect_diamond"}},
        {"jew": {"asset": "gem/perfect_diamond", "extra": "keep"}}
    ]);
    write_document(&mut files, path, &expected)?;
    let catalog = embedded_catalog();
    jewels::install_routes_and_icons(
        &mut files,
        &catalog.jewels,
        &Settings {
            jewel_beam: true,
            unique_jewel_beam: false,
            ..Settings::default()
        },
        Region::Int,
    )?;
    expected[1]["cjw"]["asset"] = "gem/mzmod_jewel".into();
    expected[2]["jew"]["asset"] = "gem/mzmod_jewel".into();
    assert_eq!(read_document(&files, path)?, expected);
    Ok(())
}

#[test]
fn jewel_beams_require_native_model_fields_to_be_strings() -> Result<()> {
    use crate::resources::write_document;
    use serde_json::json;

    let region = Region::Cn;
    let original = RegionalTemplate::read(region)?.files;
    let base_path = "data/hd/items/items.json";
    let unique_path = "data/hd/items/uniques.json";
    for (path, key, field) in [
        (base_path, "jew", "asset"),
        (unique_path, "rainbow_facet", "uber"),
    ] {
        let mut files = original.clone();
        let mut table = if files.contains_key(path) {
            read_document(&files, path)?
        } else {
            embedded_catalog().jewels.native_base_routes.clone()
        };
        let entry = table
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find_map(|row| row.get_mut(key))
            .unwrap();
        entry[field] = json!({"variant": "regional"});
        write_document(&mut files, path, &table)?;
        let result = render(
            &mut files,
            &Settings {
                jewel_beam: path == base_path,
                unique_jewel_beam: path == unique_path,
                ..Settings::default()
            },
            region,
        );
        assert!(
            result.is_err(),
            "overwrote the structured field {key}/{field}"
        );
        assert_eq!(read_document(&files, path)?, table);
    }
    Ok(())
}

#[test]
fn jewel_inventory_variants_compose_with_explicit_and_shared_sprites() -> Result<()> {
    use crate::item_catalog::JewelModel;
    use std::collections::BTreeSet;

    let shared = "other/shared.SPRITE";
    let legacy: JewelModel = serde_json::from_value(serde_json::json!({
        "native": {}, "icons": [shared]
    }))?;
    assert_eq!(
        legacy
            .inventory_sprites("unrelated/model")?
            .collect::<Vec<_>>(),
        [shared]
    );
    let numbered: JewelModel = serde_json::from_value(serde_json::json!({
        "native": {}, "numbered_inventory_variants": 1, "icons": [shared, "gem/look.sprite"]
    }))?;
    let sprites: BTreeSet<_> = numbered.inventory_sprites("gem/look")?.collect();
    assert_eq!(
        sprites,
        [
            shared,
            "gem/look.sprite",
            "gem/look.lowend.sprite",
            "gem/look1.sprite",
            "gem/look1.lowend.sprite"
        ]
        .map(String::from)
        .into()
    );
    Ok(())
}

#[test]
fn jewel_beams_preserve_diamonds_fragments_and_regional_inventory_icons() -> Result<()> {
    let base_path = "data/hd/items/items.json";
    let unique_path = "data/hd/items/uniques.json";
    let model_dir = "data/hd/items/misc";
    let icon_dir = "data/hd/global/ui/items/misc";
    for region in [Region::Int, Region::Cn] {
        let original = RegionalTemplate::read(region)?.files;
        for (regular, unique) in [(true, false), (false, true), (true, true)] {
            let mut files = original.clone();
            render(
                &mut files,
                &Settings {
                    jewel_beam: regular,
                    unique_jewel_beam: unique,
                    ..Settings::default()
                },
                region,
            )?;
            assert_eq!(
                files.contains_key(base_path),
                regular || original.contains_key(base_path)
            );
            if regular {
                let base = read_document(&files, base_path)?;
                for code in ["jew", "cjw"] {
                    assert_eq!(route(&base, code)["asset"], "gem/mzmod_jewel");
                }
                assert_eq!(route(&base, "gpw")["asset"], "gem/perfect_diamond");
                for (index, element) in ["cold", "magic", "lightning", "fire", "physical", "poison"]
                    .iter()
                    .enumerate()
                {
                    assert_eq!(
                        route(&base, &format!("um{}", index + 1))["asset"],
                        format!("body_part/fragment_{element}")
                    );
                }
            }
            let uniques = read_document(&files, unique_path)?;
            let mut expected_uniques = read_document(&original, unique_path)?;
            if unique {
                for (key, alias) in &embedded_catalog().jewels.unique_routes {
                    let entry = expected_uniques
                        .as_array_mut()
                        .unwrap()
                        .iter_mut()
                        .find_map(|row| row.get_mut(key))
                        .unwrap();
                    for tier in ["normal", "uber", "ultra"] {
                        entry[tier] = alias.as_str().into();
                    }
                    let model = read_document(
                        &files,
                        &format!("{model_dir}/{}.json", alias.trim_start_matches("../misc/")),
                    )?;
                    assert_eq!(
                        model["entities"].as_array().unwrap().last().unwrap()["name"],
                        "mzmod_drop_beam_0"
                    );
                }
            }
            assert_eq!(uniques, expected_uniques);
            let facet = route(&uniques, "rainbow_facet");
            assert_eq!(
                facet["normal"],
                if unique {
                    "gem/mzmod_jewel"
                } else {
                    "gem/perfect_diamond"
                }
            );
            assert!(files.contains_key(&format!("{model_dir}/gem/mzmod_jewel.json")));
            assert!(!files.contains_key(&format!("{model_dir}/gem/perfect_diamond.json")));
            for variant in ["", "1", "2", "3", "4", "5", "6"] {
                for extension in ["sprite", "lowend.sprite"] {
                    assert!(files
                        .contains_key(&format!("{icon_dir}/gem/mzmod_jewel{variant}.{extension}")));
                }
            }
            for element in ["cold", "magic", "lightning", "fire", "physical", "poison"] {
                assert!(
                    !files.contains_key(&format!("{model_dir}/body_part/fragment_{element}.json"))
                );
                for extension in ["sprite", "lowend.sprite"] {
                    let name = format!("body_part/mzmod_fragment_{element}.{extension}");
                    assert_eq!(files.contains_key(&format!("{icon_dir}/{name}")), unique);
                    if unique {
                        let regional =
                            ["poison", "physical"].contains(&element) && extension == "sprite";
                        let group = if regional { region.code() } else { "common" };
                        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                            .join("../ModTemplate/jewel-icons");
                        assert_eq!(
                            files[&format!("{icon_dir}/{name}")],
                            std::fs::read(root.join(group).join(name))?
                        );
                    }
                }
            }
        }
    }
    Ok(())
}

#[test]
fn previews_render_a_single_locale_like_the_installed_bank() -> Result<()> {
    let settings = Settings {
        english_names: false,
        base_tiers: false,
        max_hints: false,
        rune_numbers: false,
        ..crate::enabled_features()
    };
    for region in Region::ALL {
        let original = RegionalTemplate::read(region)?.files;
        let names = bank(&original, "item-names");
        let runes = bank(&original, "item-runes");
        let files = crate::render_mod(region, &settings)?;
        for (bank_name, source, keys) in [
            ("item-names", names, &["uit", "crs", "jew"][..]),
            (
                "item-runes",
                runes,
                &["Runeword130", "r09", "r10", "r10L", "r33"][..],
            ),
        ] {
            let installed = bank(&files, bank_name);
            for key in keys {
                let original = row(&source, key);
                for locale in ["zhTW", "zhCN", "enUS", "deDE"] {
                    // A preview only needs the requested translation, even for a 13-locale base.
                    let sample = serde_json::json!({
                        "id": original["id"],
                        "Key": key,
                        locale: original[locale],
                    });
                    assert_eq!(
                        string_banks::preview_label(bank_name, &sample, locale, &settings),
                        row(&installed, key)[locale],
                        "{} {bank_name}/{key}/{locale}",
                        region.code()
                    );
                }
            }
        }
    }
    Ok(())
}
