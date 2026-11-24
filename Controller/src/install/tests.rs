use super::*;
use crate::{
    file_fingerprints::checksum, region::BUILD_MARKER, render_mod, template::CLIENT_BUILDS,
};
use std::{
    path::PathBuf,
    sync::atomic::{AtomicU32, Ordering},
};

struct TestGame(PathBuf);

impl TestGame {
    fn new() -> Self {
        static NEXT: AtomicU32 = AtomicU32::new(0);
        let directory = std::env::temp_dir().join(format!(
            "mzmod-install-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&directory).unwrap();
        fs::write(directory.join("D2R.exe"), []).unwrap();
        Self(directory)
    }
    fn live(&self) -> PathBuf {
        self.0.join("mods/MzMod")
    }
}

impl Drop for TestGame {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn restores_each_region_and_preserves_unowned_files() -> Result<()> {
    for region in [Region::Int, Region::Cn] {
        let game = TestGame::new();
        let default = Settings::default();
        let mut custom = Settings {
            english_names: false,
            mini_cube: false,
            sorceress_eye_care: false,
            custom_font: false,
            ..default.clone()
        };
        custom.rune_beams[0] = true;
        let package = game.live().join(PACKAGE);
        assert_eq!(load(&game.0, region)?, default);
        apply(&game.0, region, &default)?;
        fs::write(game.live().join("player-note.txt"), b"keep")?;
        apply(&game.0, region, &custom)?;
        assert_eq!(load(&game.0, region)?, custom);
        let low_rune = package.join("data/hd/items/misc/rune/el_rune.json");
        assert!(low_rune.is_file());
        assert!(!package
            .join("data/global/ui/layouts/playerinventoryexpansionlayouthd.json")
            .exists());
        apply(&game.0, region, &default)?;
        assert_eq!(low_rune.exists(), default.rune_beams[0]);
        for (path, original) in render_mod(region, &default)? {
            assert_eq!(
                checksum(&fs::read(package.join(&path))?),
                checksum(&original),
                "{path}"
            );
        }
        assert_eq!(fs::read(game.live().join("player-note.txt"))?, b"keep");
    }
    Ok(())
}

#[test]
fn independent_editor_changes_survive_in_the_profile_and_rendered_mod() -> Result<()> {
    for region in Region::ALL {
        let game = TestGame::new();
        let destination = Destination::Game(game.0.clone());
        let baseline = destination.load(region)?;
        let mut first = baseline.clone();
        first.english_names = !baseline.english_names;
        first.rune_beams[29] = !baseline.rune_beams[29];
        let mut second = baseline.clone();
        second.mini_cube = !baseline.mini_cube;
        second.rune_beams[8] = !baseline.rune_beams[8];
        let mut expected = first.clone();
        expected.mini_cube = second.mini_cube;
        expected.rune_beams[8] = second.rune_beams[8];

        assert_eq!(destination.apply_edits(region, &baseline, &first)?, first);
        assert_eq!(
            destination.apply_edits(region, &baseline, &second)?,
            expected
        );
        assert_eq!(destination.load(region)?, expected);
        let runes = game.live().join(PACKAGE).join("data/hd/items/misc/rune");
        assert_eq!(
            runes.join("ber_rune.json").exists(),
            expected.rune_beams[29]
        );
        assert_eq!(runes.join("ort_rune.json").exists(), expected.rune_beams[8]);
        assert_eq!(
            destination.apply_edits(region, &baseline, &baseline)?,
            expected
        );

        destination.apply(region, &baseline)?;
        assert_eq!(destination.load(region)?, baseline);
    }
    Ok(())
}

#[test]
fn conflict_and_wrong_region_leave_the_live_install_intact() -> Result<()> {
    let game = TestGame::new();
    apply(&game.0, Region::Int, &Settings::default())?;
    let state = fs::read(game.live().join(SETTINGS_FILE))?;
    assert!(apply(&game.0, Region::Cn, &Settings::default()).is_err());
    let strings = game.live().join(PACKAGE).join("data/local/lng/strings");
    let names = ["item-names.json", "item-runes.json"];
    for name in names {
        fs::write(strings.join(name), name)?;
    }
    let error = format!(
        "{:#}",
        apply(&game.0, Region::Int, &Settings::default()).unwrap_err()
    );
    for name in names {
        assert!(error.contains(name), "{error}");
        assert_eq!(fs::read(strings.join(name))?, name.as_bytes());
    }
    assert_eq!(fs::read(game.live().join(SETTINGS_FILE))?, state);
    Ok(())
}

#[test]
fn a_player_file_can_occupy_a_directory_until_a_selected_resource_needs_it() -> Result<()> {
    let game = TestGame::new();
    let region = Region::Cn;
    let mut settings = Settings {
        custom_font: false,
        ..Settings::default()
    };
    apply(&game.0, region, &settings)?;
    let fonts = game.live().join(PACKAGE).join("data/hd/ui/FONTS");
    fs::create_dir_all(fonts.parent().unwrap())?;
    fs::write(&fonts, b"player file")?;
    apply(&game.0, region, &settings)?;

    settings.custom_font = true;
    let error = format!("{:#}", apply(&game.0, region, &settings).unwrap_err());
    assert!(error.contains("FONTS"), "{error}");
    assert_eq!(fs::read(&fonts)?, b"player file");
    assert!(!load(&game.0, region)?.custom_font);
    fs::rename(&fonts, fonts.with_file_name("player-file"))?;
    apply(&game.0, region, &settings)?;
    assert_eq!(load(&game.0, region)?, settings);
    assert_eq!(
        fs::read(fonts.with_file_name("player-file"))?,
        b"player file"
    );
    Ok(())
}

#[cfg(windows)]
#[test]
fn locked_files_are_identified_and_the_same_update_succeeds_after_they_close() -> Result<()> {
    use std::os::windows::fs::OpenOptionsExt;

    let game = TestGame::new();
    let region = Region::Int;
    let initial = Settings::default();
    let edited = Settings {
        mini_cube: !initial.mini_cube,
        ..initial.clone()
    };
    apply(&game.0, region, &initial)?;
    let note = game.live().join("player-note.txt");
    fs::write(&note, b"keep")?;
    let strings = game
        .live()
        .join(PACKAGE)
        .join("data/local/lng/strings/item-runes.json");
    let original_profile = fs::read(game.live().join(SETTINGS_FILE))?;
    for file in [&note, &strings] {
        let lock = fs::OpenOptions::new().read(true).share_mode(0).open(file)?;
        let error = format!("{:#}", apply(&game.0, region, &edited).unwrap_err());
        drop(lock);
        assert!(
            error.contains(file.file_name().unwrap().to_str().unwrap()),
            "{error}"
        );
        assert_eq!(fs::read(game.live().join(SETTINGS_FILE))?, original_profile);
        assert_eq!(fs::read(&note)?, b"keep");
    }
    apply(&game.0, region, &edited)?;
    assert_eq!(load(&game.0, region)?, edited);
    assert_eq!(fs::read(note)?, b"keep");
    Ok(())
}

#[test]
fn enabling_a_new_resource_checks_the_players_existing_file() -> Result<()> {
    let game = TestGame::new();
    let region = Region::Int;
    let mut settings = Settings::default();
    settings.rune_beams[0] = false;
    apply(&game.0, region, &settings)?;
    let package = game.live().join(PACKAGE);
    let name = "data/hd/items/misc/rune/el_rune.json";
    let file = package.join(name).with_file_name("EL_RUNE.JSON");
    assert!(!package.join(name).exists());
    fs::create_dir_all(file.parent().unwrap())?;
    fs::write(&file, b"player model")?;
    settings.rune_beams[0] = true;
    assert!(apply(&game.0, region, &settings).is_err());
    assert_eq!(fs::read(&file)?, b"player model");
    assert!(!load(&game.0, region)?.rune_beams[0]);

    let rendered = render_mod(region, &settings)?;
    fs::write(&file, &rendered[name])?;
    apply(&game.0, region, &settings)?;
    assert_eq!(fs::read(package.join(name))?, rendered[name]);
    assert_eq!(load(&game.0, region)?, settings);
    Ok(())
}

#[test]
fn older_profiles_keep_their_choices_and_inherit_new_defaults() -> Result<()> {
    let game = TestGame::new();
    let region = Region::Int;
    let mut settings = Settings::default();
    settings.waypoints = !settings.waypoints;
    apply(&game.0, region, &settings)?;
    let marker = game.live().join(PACKAGE).join(BUILD_MARKER);
    let profile = game.live().join(SETTINGS_FILE);
    let mut saved: serde_json::Value = serde_json::from_slice(&fs::read(&profile)?)?;
    saved["settings"]
        .as_object_mut()
        .unwrap()
        .remove("mini_cube");
    let previous_build = "100";
    saved["files"][BUILD_MARKER] = checksum(previous_build.as_bytes())
        .to_ascii_uppercase()
        .into();
    fs::write(&profile, serde_json::to_vec(&saved)?)?;
    fs::write(&marker, b"1")?;
    assert!(load(&game.0, region).is_err());
    fs::write(&marker, previous_build)?;
    assert!(load(&game.0, Region::Cn).is_err());
    assert_eq!(load(&game.0, region)?, settings);
    apply(&game.0, region, &settings)?;
    assert_eq!(
        fs::read_to_string(marker)?.trim(),
        CLIENT_BUILDS.number(region)
    );
    Ok(())
}

#[test]
fn ambiguous_profile_fields_stop_updates_before_any_choice_is_applied() -> Result<()> {
    let game = TestGame::new();
    let region = Region::Int;
    let defaults = Settings::default();
    apply(&game.0, region, &defaults)?;
    let path = game.live().join(SETTINGS_FILE);
    let profile: serde_json::Value = serde_json::from_slice(&fs::read(&path)?)?;
    let encoded = serde_json::to_string(&profile)?;
    for (prefix, member, repeated) in [
        (r#""settings":{"#, "mini_cube", r#""mini_cube":true,"#),
        (
            r#""settings":{"#,
            "rune_icons",
            r#""rune_icons":"invalid","#,
        ),
        (
            r#""settings":{"#,
            "future_choice",
            r#""future_choice":true,"future_choice":false,"#,
        ),
        ("{", "extra", r#""extra":1,"extra":2,"#),
    ] {
        let ambiguous = encoded.replacen(prefix, &format!("{prefix}{repeated}"), 1);
        fs::write(&path, &ambiguous)?;
        for error in [
            load(&game.0, region).unwrap_err(),
            apply(&game.0, region, &defaults).unwrap_err(),
        ] {
            assert!(format!("{error:#}").contains(member), "{error:#}");
        }
        assert_eq!(fs::read(&path)?, ambiguous.as_bytes());
    }
    Ok(())
}

#[test]
fn known_choices_can_change_while_other_profile_fields_survive_the_update() -> Result<()> {
    let game = TestGame::new();
    let region = Region::Cn;
    let destination = Destination::Game(game.0.clone());
    let mut edited = Settings::default();
    destination.apply(region, &edited)?;
    let path = game.live().join(SETTINGS_FILE);
    let mut profile: serde_json::Value = serde_json::from_slice(&fs::read(&path)?)?;
    let opaque = r#"{"serial":184467440737095516161234567890,"decimal":0.12345678901234567890123456789,"nested":[ 1e900, -0.00, "\u0078" ]}"#;
    profile["annotation"] = "future-value".into();
    profile["settings"]["future_choice"] = "future-value".into();
    let document = serde_json::to_string(&profile)?.replace(r#""future-value""#, opaque);
    fs::write(&path, document)?;

    let baseline = destination.load(region)?;
    edited.rune_icons = !baseline.rune_icons;
    assert_eq!(destination.apply_edits(region, &baseline, &edited)?, edited);
    let text = fs::read_to_string(path)?;
    type Members<'a> = std::collections::BTreeMap<&'a str, &'a serde_json::value::RawValue>;
    let saved: Members = serde_json::from_str(&text)?;
    let choices: Members = serde_json::from_str(saved["settings"].get())?;
    assert_eq!(
        serde_json::from_str::<bool>(choices["rune_icons"].get())?,
        edited.rune_icons
    );
    assert_eq!(saved["annotation"].get(), opaque);
    assert_eq!(choices["future_choice"].get(), opaque);
    Ok(())
}

#[test]
fn tomb_symbols_inherit_old_entrance_choices_and_can_then_be_set_independently() -> Result<()> {
    for region in [Region::Int, Region::Cn] {
        let game = TestGame::new();
        let profile = game.live().join(SETTINGS_FILE);
        let roomtiles = game.live().join(PACKAGE).join("data/hd/roomtiles");
        for entrances in [false, true] {
            let mut settings = Settings {
                entrances,
                tomb_symbols: entrances,
                ..Settings::default()
            };
            apply(&game.0, region, &settings)?;
            let mut saved: serde_json::Value = serde_json::from_slice(&fs::read(&profile)?)?;
            saved["settings"]
                .as_object_mut()
                .unwrap()
                .remove("tomb_symbols");
            fs::write(&profile, serde_json::to_vec(&saved)?)?;
            assert_eq!(load(&game.0, region)?, settings);

            settings.tomb_symbols = !entrances;
            apply(&game.0, region, &settings)?;
            assert_eq!(load(&game.0, region)?, settings);
            for index in 1..=7 {
                let tile = roomtiles.join(format!("act_2_desert_to_tomb_tal_{index}.json"));
                assert_eq!(tile.is_file(), settings.tomb_symbols);
            }
            assert_eq!(
                roomtiles.join("act_2_desert_to_tomb_viper.json").is_file(),
                entrances
            );
        }
    }
    Ok(())
}

#[test]
fn an_ambiguous_or_incomplete_manifest_cannot_update_an_installation() -> Result<()> {
    let game = TestGame::new();
    let region = Region::Cn;
    let settings = Settings::default();
    apply(&game.0, region, &settings)?;
    let profile_path = game.live().join(SETTINGS_FILE);
    let previous: serde_json::Value = serde_json::from_slice(&fs::read(&profile_path)?)?;
    let note = game.live().join("player-note.txt");
    fs::write(&note, b"keep")?;

    let valid = checksum(b"old");
    for (name, digest) in [
        (MOD_INFO, None),
        (BUILD_MARKER, None),
        (MOD_INFO, Some("not a digest")),
        ("MODINFO.JSON", Some(valid.as_str())),
        ("data/global", Some(valid.as_str())),
    ] {
        let mut saved = previous.clone();
        if let Some(digest) = digest {
            saved["files"][name] = digest.into();
        } else {
            saved["files"].as_object_mut().unwrap().remove(name);
        }
        let bytes = serde_json::to_vec(&saved)?;
        fs::write(&profile_path, &bytes)?;
        let error = format!("{:#}", load(&game.0, region).unwrap_err());
        assert!(
            error
                .to_ascii_lowercase()
                .contains(&name.to_ascii_lowercase()),
            "{error}"
        );
        assert!(apply(&game.0, region, &settings).is_err());
        assert_eq!(fs::read(&profile_path)?, bytes);
        assert_eq!(fs::read(&note)?, b"keep");
    }
    Ok(())
}

#[test]
fn interrupted_swap_recovers_without_erasing_the_previous_profile() -> Result<()> {
    let game = TestGame::new();
    let custom = Settings {
        mini_cube: false,
        ..Settings::default()
    };
    apply(&game.0, Region::Cn, &custom)?;
    let work = game.0.join("mods/.mzmod-controller");
    fs::rename(game.live(), work.join("previous"))?;
    fs::create_dir(work.join("staging"))?;
    let executable = game.0.join("D2R.exe");
    fs::remove_file(&executable)?;
    assert!(load(&game.0, Region::Cn).is_err());
    assert!(apply(&game.0, Region::Cn, &custom).is_err());
    assert!(!game.live().exists());
    assert!(work.join("previous").join(SETTINGS_FILE).is_file());
    assert!(work.join("staging").is_dir());
    fs::write(executable, [])?;
    assert_eq!(load(&game.0, Region::Cn)?, custom);
    assert!(game.live().join(SETTINGS_FILE).is_file());
    assert!(!work.join("staging").exists());
    // The backup is disposable only after the replacement has a valid saved profile.
    let previous = work.join("previous");
    fs::create_dir(&previous)?;
    fs::write(previous.join("player-note.txt"), b"keep")?;
    let profile = game.live().join(SETTINGS_FILE);
    let valid = fs::read(&profile)?;
    fs::remove_file(&profile)?;
    assert!(load(&game.0, Region::Cn).is_err());
    assert_eq!(fs::read(previous.join("player-note.txt"))?, b"keep");
    fs::write(&profile, b"incomplete")?;
    assert!(load(&game.0, Region::Cn).is_err());
    assert_eq!(fs::read(previous.join("player-note.txt"))?, b"keep");
    fs::write(profile, valid)?;
    let marker = game.live().join(PACKAGE).join(BUILD_MARKER);
    let original_marker = fs::read(&marker)?;
    fs::write(&marker, "invalid")?;
    assert!(load(&game.0, Region::Cn).is_err());
    assert_eq!(fs::read(previous.join("player-note.txt"))?, b"keep");
    fs::write(marker, original_marker)?;

    let _lock = Workspace::open(&game.0.join("mods"))?;
    assert!(!previous.exists());
    assert!(apply(&game.0, Region::Cn, &custom).is_err());
    Ok(())
}

#[test]
fn backup_cleanup_verifies_the_committed_profile_and_ignores_unowned_files() -> Result<()> {
    let game = TestGame::new();
    let region = Region::Int;
    let settings = Settings::default();
    apply(&game.0, region, &settings)?;
    let previous = transaction::previous_install(&game.0.join("mods"));
    fs::create_dir(&previous)?;
    let note = previous.join("previous-note.txt");
    fs::write(&note, b"keep")?;
    let assert_retained = |expected: &str| -> Result<()> {
        let error = format!("{:#}", load(&game.0, region).unwrap_err());
        assert!(error.contains(expected), "{error}");
        assert!(error.contains(&previous.display().to_string()), "{error}");
        assert_eq!(fs::read(&note)?, b"keep");
        Ok(())
    };

    let package = game.live().join(PACKAGE);
    let relative = "data/local/lng/strings/item-names.json";
    let file = package.join(relative);
    let original = fs::read(&file)?;
    fs::remove_file(&file)?;
    assert_retained(relative)?;
    fs::write(&file, b"altered")?;
    assert_retained(relative)?;
    assert_eq!(fs::read(&file)?, b"altered");
    fs::write(&file, original)?;

    let config = game.live().join(SETTINGS_FILE);
    let saved: serde_json::Value = serde_json::from_slice(&fs::read(&config)?)?;
    let member = format!(
        "{}:{}",
        serde_json::to_string(relative)?,
        saved["files"][relative]
    );
    let repeated =
        serde_json::to_string(&saved)?.replacen(&member, &format!("{member},{member}"), 1);
    fs::write(&config, repeated)?;
    assert_retained(relative)?;
    for required in ["modinfo.json", BUILD_MARKER] {
        let mut incomplete = saved.clone();
        incomplete["files"]
            .as_object_mut()
            .unwrap()
            .remove(required);
        fs::write(&config, serde_json::to_vec(&incomplete)?)?;
        assert_retained(required)?;
    }
    for invalid in ["../player-note.txt", "C:/other", "data/local"] {
        let mut malformed = saved.clone();
        malformed["files"][invalid] = checksum(b"other").into();
        fs::write(&config, serde_json::to_vec(&malformed)?)?;
        assert_retained(invalid)?;
    }

    let alias = relative.replace('/', "\\").to_ascii_uppercase();
    let mut historical = saved;
    historical["files"][&alias] = historical["files"][relative].clone();
    fs::write(&config, serde_json::to_vec(&historical)?)?;
    // Both spellings name the same Windows resource.
    assert!(load(&game.0, region).is_err());
    assert!(note.is_file());
    historical["files"]
        .as_object_mut()
        .unwrap()
        .remove(relative);
    let older_build = b"100";
    historical["files"]
        .as_object_mut()
        .unwrap()
        .remove(BUILD_MARKER);
    historical["files"][BUILD_MARKER.to_ascii_uppercase()] = checksum(older_build).into();
    fs::write(package.join(BUILD_MARKER), older_build)?;
    // A previous release's own resources are authoritative during recovery.
    fs::write(package.join("data/retiré.txt"), b"old release")?;
    historical["files"]["DATA/RETIRÉ.TXT"] = checksum(b"old release").to_ascii_uppercase().into();
    fs::write(&config, serde_json::to_vec(&historical)?)?;
    fs::write(package.join("player-note.txt"), b"custom")?;
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink("missing-player-file", package.join("player-link"))?;
        fs::write(package.join("data\\retiré.txt"), b"unrelated flat file")?;
    }
    assert_eq!(load(&game.0, region)?, settings);
    assert!(!previous.exists());
    assert_eq!(fs::read(package.join("player-note.txt"))?, b"custom");
    Ok(())
}

#[cfg(unix)]
#[test]
fn recovery_refuses_links_to_managed_files_and_parent_directories() -> Result<()> {
    let game = TestGame::new();
    let region = Region::Cn;
    apply(&game.0, region, &Settings::default())?;
    let previous = game.0.join("mods/.mzmod-controller/previous");
    fs::create_dir(&previous)?;
    let package = game.live().join(PACKAGE);
    for relative in [
        "modinfo.json",
        "data/global",
        "data/local/lng/strings/item-names.json",
    ] {
        let path = package.join(relative);
        let external = game.0.join("external");
        fs::rename(&path, &external)?;
        std::os::unix::fs::symlink(&external, &path)?;
        let error = format!("{:#}", load(&game.0, region).unwrap_err());
        assert!(error.contains(relative), "{error}");
        assert!(previous.is_dir());
        fs::remove_file(&path)?;
        fs::rename(&external, path)?;
    }
    load(&game.0, region)?;
    assert!(!previous.exists());
    Ok(())
}

#[test]
fn portable_outputs_stay_copyable_and_keep_each_regions_settings() -> Result<()> {
    let folder = TestGame::new();
    fs::remove_file(folder.0.join("D2R.exe"))?;
    let cn = Destination::beside(&folder.0, Region::Cn);
    assert_eq!(cn.live(), folder.0.join("MzMod"));
    assert_eq!(cn.load(Region::Cn)?, Settings::default());
    assert_eq!(fs::read_dir(&folder.0)?.count(), 0);
    let custom = Settings {
        rune_icons: !Settings::default().rune_icons,
        ..Settings::default()
    };
    cn.apply(Region::Cn, &custom)?;
    assert!(cn.apply(Region::Int, &Settings::default()).is_err());
    fs::write(cn.live().join("custom.txt"), b"keep")?;
    let int = Destination::beside(&folder.0, Region::Int);
    assert_eq!(int.live(), folder.0.join("INT/MzMod"));
    int.apply(Region::Int, &Settings::default())?;
    for (region, destination) in [(Region::Int, &int), (Region::Cn, &cn)] {
        assert_eq!(Destination::beside(&folder.0, region), *destination);
        assert_eq!(destination.installed_region(), Some(region));
        assert_eq!(
            fs::read_to_string(destination.live().join(PACKAGE).join(BUILD_MARKER))?.trim(),
            CLIENT_BUILDS.number(region)
        );
    }
    assert_eq!(cn.load(Region::Cn)?, custom);
    assert_eq!(fs::read(cn.live().join("custom.txt"))?, b"keep");
    assert!(!int.live().join("custom.txt").exists());
    fs::rename(cn.live(), folder.0.join("copied-mod"))?;
    assert_eq!(Destination::beside(&folder.0, Region::Int), int);
    assert_eq!(Destination::beside(&folder.0, Region::Cn), cn);
    Ok(())
}

#[test]
fn a_foreign_region_in_the_regional_folder_leaves_the_free_shared_output_available() -> Result<()> {
    let folder = TestGame::new();
    let shared = Destination::Export(folder.0.clone());
    let regional = Destination::Export(folder.0.join("CN"));
    let defaults = Settings::default();
    regional.apply(Region::Int, &defaults)?;
    let note = regional.live().join("player-note.txt");
    fs::write(&note, b"keep")?;
    assert_eq!(Destination::beside(&folder.0, Region::Cn), shared);

    // An unreadable profile at the regional path still needs the player's attention.
    let profile = regional.live().join(SETTINGS_FILE);
    let original = fs::read(&profile)?;
    fs::write(&profile, b"incomplete")?;
    assert_eq!(Destination::beside(&folder.0, Region::Cn), regional);
    assert!(regional.load(Region::Cn).is_err());
    fs::write(profile, original)?;

    fs::create_dir(shared.live())?;
    let occupied = shared.live().join("occupied.txt");
    fs::write(&occupied, b"other output")?;
    assert_eq!(Destination::beside(&folder.0, Region::Cn), regional);
    assert!(regional.apply(Region::Cn, &defaults).is_err());
    assert_eq!(fs::read(&occupied)?, b"other output");
    fs::remove_file(occupied)?;

    let selected = Destination::beside(&folder.0, Region::Cn);
    assert_eq!(selected, shared);
    selected.apply(Region::Cn, &defaults)?;
    assert_eq!(selected.installed_region(), Some(Region::Cn));
    assert_eq!(regional.load(Region::Int)?, defaults);
    assert_eq!(fs::read(note)?, b"keep");
    Ok(())
}

#[test]
fn a_mods_backup_is_not_a_game_directory() -> Result<()> {
    let folder = TestGame::new();
    let executable = folder.0.join("D2R.exe");
    fs::remove_file(&executable)?;
    fs::create_dir_all(folder.live())?;
    let mods = folder.0.join("mods");
    assert!(load(&folder.0, Region::Int).is_err());
    assert!(apply(&folder.0, Region::Int, &Settings::default()).is_err());
    assert!(fs::read_dir(folder.live())?.next().is_none());
    assert!(!mods.join(".mzmod-controller").exists());

    fs::create_dir(executable)?;
    assert!(load(&folder.0, Region::Int).is_err());
    assert_eq!(
        Destination::beside(&mods, Region::Int),
        Destination::Export(mods)
    );
    Ok(())
}

#[test]
fn a_standalone_mods_directory_adopts_a_legacy_export() -> Result<()> {
    let folder = TestGame::new();
    fs::remove_file(folder.0.join("D2R.exe"))?;
    let mods = folder.0.join("mods");
    let existing = mods.join("MzMod");
    let package = existing.join(PACKAGE);
    fs::create_dir_all(package.join("data/global"))?;
    let template = RegionalTemplate::read(Region::Cn)?;
    fs::write(
        package.join("modinfo.json"),
        &template.files["modinfo.json"],
    )?;
    fs::write(package.join(BUILD_MARKER), &template.files[BUILD_MARKER])?;
    let destination = Destination::Export(mods.clone());
    let suggested = Destination::beside(&mods, Region::Cn);
    if CLIENT_BUILDS.number(Region::Cn) == CLIENT_BUILDS.number(Region::Int) {
        assert_eq!(suggested.live(), mods.join("CN/MzMod"));
        assert_eq!(destination.installed_region(), None);
    } else {
        assert_eq!(suggested, destination);
        assert_eq!(destination.installed_region(), Some(Region::Cn));
    }
    assert_eq!(destination.load(Region::Cn)?, Settings::default());
    assert_eq!(
        Destination::beside(&mods, Region::Int).live(),
        mods.join("INT/MzMod")
    );
    let waypoint = "data/hd/objects/waypoint_portals/sewer_waypoint.json";
    let path = package.join(waypoint);
    let note = path.with_file_name("player-note.txt");
    let gameplay_huds = [
        "data/global/ui/layouts/mzmodgameplayhudhd.json",
        "data/global/ui/layouts/controller/mzmodgameplayhudhd.json",
    ];
    for name in gameplay_huds.into_iter().chain([waypoint]) {
        let file = package.join(name);
        fs::create_dir_all(file.parent().unwrap())?;
        fs::write(file, &template.files[name])?;
    }
    fs::write(&note, b"keep")?;
    let settings = Settings {
        waypoints: false,
        mini_cube: true,
        auto_open_cube: false,
        ..Settings::default()
    };
    destination.apply(Region::Cn, &settings)?;
    assert!(!path.exists());
    for name in gameplay_huds {
        assert!(!package.join(name).exists());
    }
    assert!(package
        .join("data/global/ui/layouts/controller/hudpanelhd.json")
        .is_file());
    assert_eq!(fs::read(note)?, b"keep");
    assert_eq!(destination.load(Region::Cn)?, settings);
    Ok(())
}

#[test]
fn a_preexisting_work_folder_cannot_erase_player_files() -> Result<()> {
    let folder = TestGame::new();
    fs::remove_file(folder.0.join("D2R.exe"))?;
    let staging = folder.0.join(".mzmod-controller/staging");
    fs::create_dir_all(&staging)?;
    fs::write(staging.join("notes"), "keep")?;
    let target = Destination::beside(&folder.0, Region::Int);
    assert!(target.load(Region::Int).is_err());
    assert!(target.apply(Region::Int, &Settings::default()).is_err());
    assert_eq!(fs::read(staging.join("notes"))?, b"keep");
    assert!(!target.live().exists());
    Ok(())
}

#[test]
fn default_stays_beside_the_executable_in_the_game_root_or_mods() -> Result<()> {
    let game = TestGame::new();
    let mods = game.0.join("mods");
    let export = Destination::beside(&game.0, Region::Int);
    assert_eq!(export, Destination::Export(game.0.clone()));
    export.apply(Region::Int, &Settings::default())?;
    assert!(game.0.join("MzMod/MzMod.mpq").is_dir());
    assert!(!mods.exists());

    fs::create_dir(&mods)?;
    let install = Destination::beside(&mods, Region::Int);
    assert_eq!(install, Destination::Game(game.0.clone()));
    install.apply(Region::Int, &Settings::default())?;
    assert!(game.live().join(PACKAGE).is_dir());
    let other = Destination::beside(&mods, Region::Cn);
    assert_eq!(other, Destination::Export(mods.join("CN")));
    other.apply(Region::Cn, &Settings::default())?;
    assert_eq!(other.installed_region(), Some(Region::Cn));
    assert_eq!(install.installed_region(), Some(Region::Int));
    assert!(install.load(Region::Cn).is_err());
    Ok(())
}

#[test]
fn portable_export_preserves_unrecognized_and_conflicting_outputs() -> Result<()> {
    let folder = TestGame::new();
    fs::remove_file(folder.0.join("D2R.exe"))?;
    let foreign = folder.0.join("MzMod");
    fs::create_dir(&foreign)?;
    assert_eq!(Destination::beside(&folder.0, Region::Int).live(), foreign);
    fs::write(foreign.join("custom.txt"), b"keep")?;
    let int = Destination::beside(&folder.0, Region::Int);
    assert_eq!(int.live(), folder.0.join("INT/MzMod"));
    int.apply(Region::Int, &Settings::default())?;
    let modified = int.live().join(PACKAGE).join("modinfo.json");
    fs::write(&modified, br#"{"name":"MzMod","custom":true}"#)?;
    assert!(int.apply(Region::Int, &Settings::default()).is_err());
    assert_eq!(fs::read(&modified)?, br#"{"name":"MzMod","custom":true}"#);
    assert_eq!(fs::read(foreign.join("custom.txt"))?, b"keep");
    Ok(())
}

#[test]
fn portable_routing_and_explicit_updates_require_the_same_package_identity() -> Result<()> {
    let folder = TestGame::new();
    let shared = Destination::Export(folder.0.clone());
    let regional = Destination::Export(folder.0.join("CN"));
    let custom = Settings {
        waypoints: false,
        ..Settings::default()
    };
    regional.apply(Region::Cn, &custom)?;
    shared.apply(Region::Cn, &Settings::default())?;
    let live = shared.live();
    let info = live.join(PACKAGE).join(MOD_INFO);
    let profile = live.join(SETTINGS_FILE);
    let original_info = fs::read(&info)?;
    let saved = fs::read(&profile)?;
    let check_rejected = || -> Result<()> {
        assert_eq!(shared.installed_region(), None);
        assert!(shared.load(Region::Cn).is_err());
        assert!(shared.apply(Region::Cn, &custom).is_err());
        let selected = Destination::beside(&folder.0, Region::Cn);
        assert_eq!(selected, regional);
        assert_eq!(selected.load(Region::Cn)?, custom);
        Ok(())
    };
    fs::remove_file(&profile)?;
    for other in [
        br#"{"name":"OtherMod"}"#.as_slice(),
        br#"{"name":"OtherMod","name":"MzMod"}"#,
    ] {
        fs::write(&info, other)?;
        check_rejected()?;
        assert_eq!(fs::read(&info)?, other);
    }
    fs::write(&info, original_info)?;
    fs::write(&profile, b"incomplete")?;
    check_rejected()?;
    for (key, bad_value) in [("format", 99.into()), ("region", "INT".into())] {
        let mut invalid: serde_json::Value = serde_json::from_slice(&saved)?;
        invalid[key] = bad_value;
        fs::write(&profile, serde_json::to_vec(&invalid)?)?;
        if key == "region" && CLIENT_BUILDS.number(Region::Cn) == CLIENT_BUILDS.number(Region::Int)
        {
            assert_eq!(shared.installed_region(), Some(Region::Int));
        } else {
            check_rejected()?;
        }
    }
    fs::write(&profile, saved)?;
    assert_eq!(shared.installed_region(), Some(Region::Cn));
    assert_eq!(Destination::beside(&folder.0, Region::Cn), shared);
    Ok(())
}
