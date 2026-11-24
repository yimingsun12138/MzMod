use super::*;
use serde_json::Value;
use std::collections::BTreeSet;

use crate::template::RegionalTemplate;

const CATALOG: &[u8] = include_bytes!("../../../ModTemplate/visuals.json");
const LAYOUT: &str = "data/global/ui/layouts/";
const PANEL: &str = "data/hd/global/ui/panel/";
const FONT: &str = "data/hd/ui/fonts/arfangxinshuh7c95b5_eb_t.ttf";
const STORM_DISCHARGE: &str = "data/hd/overlays/sorceress/thunderstormcast.json";

fn has_layout(files: &ModFiles, name: &str) -> bool {
    files.contains_key(&format!("{LAYOUT}{name}"))
}

fn check_gameplay_huds(files: &ModFiles) -> Result<()> {
    for directory in ["", "controller/"] {
        let mut entry = read_document(files, &format!("{LAYOUT}{directory}hudpanelhd.json"))?;
        let path = format!("{LAYOUT}{directory}mzmodgameplayhudhd.json");
        let gameplay = read_document(files, &path)?;
        assert_eq!(entry["name"], "HUDPanel");
        entry["name"] = "MzModGameplayHUD".into();
        if directory.is_empty() {
            let children = entry["children"].as_array_mut().unwrap();
            let count = children.len();
            children.retain(|widget| {
                widget["type"] != "TimerWidget"
                    || widget["fields"]["message"] != "PanelManager:OpenPanel:MzModCubeStartup"
            });
            assert_eq!(children.len() + 1, count);
        } else {
            assert_eq!(entry["basedOn"], "HUDPanelHD.json");
            entry["basedOn"] = "MzModGameplayHUDHD.json".into();
        }
        assert_eq!(entry, gameplay, "{path}");
    }
    Ok(())
}

#[test]
fn each_whole_file_override_has_one_owner_in_its_region() -> Result<()> {
    let settings = crate::enabled_features();
    for region in Region::ALL {
        let original = RegionalTemplate::read(region)?.files;
        let mut owned = BTreeSet::from([MISSILES]);
        for group in visual_overrides(&settings) {
            assert!(group.enabled);
            for &path in group.paths_by_region[region.index()] {
                assert!(
                    original.contains_key(path),
                    "{} lacks {path}",
                    region.code()
                );
                assert!(owned.insert(path), "shared override {path}");
            }
        }
    }
    Ok(())
}

#[test]
fn cube_switches_control_both_inputs_and_the_bootstrap_independently() -> Result<()> {
    for region in [Region::Int, Region::Cn] {
        let original = crate::render_mod(region, &crate::enabled_features())?;
        for (mini_cube, auto_open_cube) in
            [(true, true), (true, false), (false, true), (false, false)]
        {
            let settings = Settings {
                mini_cube,
                auto_open_cube,
                ..crate::enabled_features()
            };
            assert_eq!(settings.prerequisites_met("auto_open_cube"), mini_cube);
            let files = crate::render_mod(region, &settings)?;
            let auto = mini_cube && auto_open_cube;
            assert_eq!(settings.auto_open_cube, auto_open_cube);
            assert_eq!(has_layout(&files, "hudpanelhd.json"), auto);
            assert_eq!(has_layout(&files, "mzmodcubestartuphd.json"), auto);
            assert_eq!(
                has_layout(&files, "controller/mzmodgameplayhudhd.json"),
                auto
            );
            assert_eq!(has_layout(&files, "mzmodgameplayhudhd.json"), auto);
            assert_eq!(has_layout(&files, "controller/hudpanelhd.json"), mini_cube);
            assert_eq!(
                has_layout(&files, "playerinventoryexpansionlayouthd.json"),
                mini_cube
            );
            assert_eq!(
                has_layout(&files, "horadriccubeminilayouthd.json"),
                mini_cube
            );
            for suffix in [".sprite", ".lowend.sprite"] {
                assert_eq!(
                    files.contains_key(&format!("{PANEL}hud_02/minicubeicon{suffix}")),
                    mini_cube
                );
                assert_eq!(
                    files.contains_key(&format!("{PANEL}horadric_cube/cube_grid{suffix}")),
                    mini_cube
                );
            }
            assert!(files.contains_key(&format!("{PANEL}hud_02/experience_bar.sprite")));
            if !mini_cube {
                for (path, raw) in files.iter().filter(|(p, _)| p.starts_with(LAYOUT)) {
                    let text = std::str::from_utf8(raw)?;
                    for name in [
                        "MzModMiniCube",
                        "HoradricCubeMiniLayout",
                        "MzModCubeStartup",
                        "MzModGameplayHUD",
                    ] {
                        assert!(!text.contains(name), "{name} still referenced by {path}");
                    }
                }
            } else if auto_open_cube {
                check_gameplay_huds(&files)?;
                assert_eq!(files, original);
            }
        }
    }
    Ok(())
}

#[test]
fn combat_switches_restore_only_their_own_missiles() -> Result<()> {
    let catalog: Value = serde_json::from_slice(CATALOG)?;
    for region in [Region::Int, Region::Cn] {
        let original = RegionalTemplate::read(region)?.files;
        let original_table: Value = serde_json::from_slice(&original[MISSILES])?;
        for (sorceress, assassin) in [(true, true), (false, true), (true, false), (false, false)] {
            let mut files = original.clone();
            let settings = Settings {
                sorceress_eye_care: sorceress,
                assassin_eye_care: assassin,
                ..crate::enabled_features()
            };
            render(&mut files, &settings.effective(), region)?;
            assert_eq!(files.contains_key(STORM_DISCHARGE), sorceress);
            if sorceress && assassin {
                assert_eq!(files, original);
            } else if sorceress || assassin {
                let mut expected = original_table.clone();
                let disabled = if sorceress {
                    "assassin_eye_care"
                } else {
                    "sorceress_eye_care"
                };
                for (key, value) in catalog["overrides"][disabled]["native_missiles"]
                    .as_object()
                    .unwrap()
                {
                    expected[key] = match value {
                        Value::String(_) => value.clone(),
                        regional => regional[region.code()].clone(),
                    };
                }
                assert_eq!(files[MISSILES], serde_json::to_vec(&expected)?);
            } else {
                assert!(!files.contains_key(MISSILES));
            }
        }
    }
    Ok(())
}

#[test]
fn mixed_combat_settings_preserve_regional_routes_and_unmanaged_numbers() -> Result<()> {
    let native_a = &[("a", ["int_native_a", "cn_native_a"])];
    let native_b = &[("b", ["int_native_b", "cn_native_b"])];
    for region in Region::ALL {
        let code = region.code().to_ascii_lowercase();
        let table = format!(
            r#"{{"b":"{code}_reduced_b","extra":[{{"n":12345678901234567890.0123456789}}],"a":"{code}_reduced_a"}}"#
        );
        for a_enabled in [true, false] {
            let mut files = ModFiles::from([(MISSILES.into(), table.as_bytes().to_vec())]);
            let groups = [
                VisualOverride {
                    enabled: a_enabled,
                    paths_by_region: [&[], &[]],
                    native_missiles: native_a,
                },
                VisualOverride {
                    enabled: !a_enabled,
                    paths_by_region: [&[], &[]],
                    native_missiles: native_b,
                },
            ];
            render_missiles(&mut files, &groups, region)?;
            let disabled = if a_enabled { "b" } else { "a" };
            let expected = table.replace(
                &format!("reduced_{disabled}"),
                &format!("native_{disabled}"),
            );
            assert_eq!(files[MISSILES], expected.as_bytes());
        }
    }
    Ok(())
}

fn assert_scene_guide_choices(files: &ModFiles, settings: &Settings) {
    let check = |enabled: bool, path: &str| {
        assert_eq!(files.contains_key(path), enabled, "{path}");
    };
    check(
        settings.entrances,
        "data/hd/roomtiles/act_2_desert_to_tomb_viper.json",
    );
    for symbol in 1..=7 {
        check(
            settings.tomb_symbols,
            &format!("data/hd/roomtiles/act_2_desert_to_tomb_tal_{symbol}.json"),
        );
    }
    for orientation in ["e", "n", "s", "w"] {
        check(
            settings.hall_directions,
            &format!("data/hd/env/preset/expansion/wildtemple/nihl{orientation}.json"),
        );
    }
    check(
        settings.horazon_journal,
        "data/hd/objects/env_pillars/arcane_tome.json",
    );
    for book in ["tower_tome", "lam_esens_tome"] {
        check(
            settings.quest_books,
            &format!("data/hd/objects/env_pillars/{book}.json"),
        );
    }
    check(
        settings.waypoints,
        "data/hd/objects/waypoint_portals/sewer_waypoint.json",
    );
}

#[test]
fn guide_and_appearance_switches_withdraw_only_their_regional_files() -> Result<()> {
    let catalog: Value = serde_json::from_slice(CATALOG)?;
    let cases: &[&[&str]] = &[
        &["waypoints"],
        &["entrances"],
        &["tomb_symbols"],
        &["hall_directions"],
        &["horazon_journal"],
        &["quest_books"],
        &["custom_font"],
        &["hud_style"],
        &["health_bars"],
        &["custom_font", "hud_style", "health_bars"],
    ];
    for region in Region::ALL {
        let original = RegionalTemplate::read(region)?.files;
        for &disabled in cases {
            let mut profile = serde_json::to_value(crate::enabled_features())?;
            let mut expected_removed = BTreeSet::new();
            for &feature in disabled {
                profile[feature] = false.into();
                let group = &catalog["overrides"][feature];
                for paths in [&group["paths"], &group["regional_paths"][region.code()]] {
                    if let Some(paths) = paths.as_array() {
                        expected_removed.extend(paths.iter().map(|path| path.as_str().unwrap()));
                    }
                }
            }
            let settings: Settings = serde_json::from_value(profile)?;
            let mut files = original.clone();
            render(&mut files, &settings.effective(), region)?;
            let removed = original
                .keys()
                .map(String::as_str)
                .filter(|path| !files.contains_key(*path))
                .collect::<BTreeSet<_>>();
            assert_eq!(removed, expected_removed, "{region:?} {disabled:?}");
            for (path, bytes) in &files {
                assert!(
                    original.get(path) == Some(bytes),
                    "{region:?} {disabled:?}: {path}"
                );
            }
            assert_scene_guide_choices(&files, &settings);
            assert_eq!(files.contains_key(FONT), settings.custom_font);
            assert_eq!(
                files.contains_key(&format!("{PANEL}hud_02/experience_bar.sprite")),
                settings.hud_style
            );
            assert_eq!(
                has_layout(&files, "controller/hudmonsterhealthhd.json"),
                settings.health_bars
            );
            assert!(has_layout(&files, "controller/hudpanelhd.json"));
        }
    }
    Ok(())
}
