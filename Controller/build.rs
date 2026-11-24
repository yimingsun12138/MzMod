#[path = "build_support/about.rs"]
mod about;
#[path = "build_support/archive.rs"]
mod archive;
#[path = "build_support/base_suffixes.rs"]
mod base_suffixes;
#[path = "build_support/charm_routes.rs"]
mod charm_routes;
#[path = "src/chinese_overlay.rs"]
mod chinese_overlay;
#[path = "build_support/cube_hud.rs"]
mod cube_hud;
#[path = "build_support/cube_layout.rs"]
mod cube_layout;
#[path = "src/file_fingerprints.rs"]
mod file_fingerprints;
#[path = "build_support/game_json.rs"]
mod game_json;
#[path = "src/game_path.rs"]
mod game_path;
#[path = "src/ground_beam.rs"]
mod ground_beam;
#[path = "src/ground_model.rs"]
mod ground_model;
#[path = "build_support/ground_model_validation.rs"]
mod ground_model_validation;
// The renderer also consumes the native game fields stored in this schema.
#[allow(dead_code)]
#[path = "src/item_catalog.rs"]
mod item_catalog;
#[path = "src/item_colors.rs"]
mod item_colors;
#[path = "build_support/item_validation.rs"]
mod item_validation;
#[path = "src/jewel_routes.rs"]
mod jewel_routes;
#[path = "build_support/label_validation.rs"]
mod label_validation;
#[path = "src/items/labels.rs"]
mod labels;
#[path = "src/mod_info.rs"]
mod mod_info;
#[path = "build_support/native_missiles.rs"]
mod native_missiles;
#[path = "build_support/output_paths.rs"]
mod output_paths;
#[path = "build_support/package_metadata.rs"]
mod package_metadata;
#[path = "build_support/prerequisites.rs"]
mod prerequisites;
#[path = "src/preview_items.rs"]
mod preview_items;
#[path = "build_support/previews.rs"]
mod previews;
#[path = "src/region.rs"]
mod region;
#[path = "build_support/release_fingerprints.rs"]
mod release_fingerprints;
#[path = "src/rune_catalog.rs"]
mod rune_catalog;
#[path = "build_support/rune_preview_icons.rs"]
mod rune_preview_icons;
#[path = "build_support/settings.rs"]
mod settings;
#[path = "build_support/string_banks.rs"]
mod string_banks;
#[path = "src/unique_map.rs"]
mod unique_map;
#[path = "build_support/visuals.rs"]
mod visuals;

use output_paths::OutputPaths;
use region::Region;
use std::{
    env, fs, io,
    path::{Path, PathBuf},
};

const RELEASE_VERSION: &str = concat!("v", env!("CARGO_PKG_VERSION"));

fn main() -> io::Result<()> {
    // A shared Cargo target directory can hold outputs from another copy of the template.
    println!("cargo:rerun-if-env-changed=MZMOD_CRATE_ROOT");
    let template = Path::new("../ModTemplate");
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo build directory"));
    println!("cargo:rerun-if-changed={}", template.display());

    let resources = archive::RegionalResources::read(template)?;
    let resources = cube_hud::compile(template, resources, &output)?;
    let mut paths = OutputPaths::from_template(
        Region::ALL
            .into_iter()
            .flat_map(|region| resources.game_paths(region).map(move |path| (region, path))),
    )?;
    let builds = package_metadata::client_builds(&resources, &mut paths)?;
    fs::write(output.join("client_builds.rs"), format!("{builds:?}"))?;
    about::write_about_copy(&output, &builds)?;
    let defaults = settings::TemplateDefaults::read(template)?;
    defaults.write_settings_code(&output)?;
    let jewel_icons = archive::RegionalResources::read(&template.join("jewel-icons"))?;
    let mut banks = string_banks::TemplateStringBanks::read(&resources)?;
    let item_catalog = item_validation::compile_item_catalog(
        template,
        &resources,
        &jewel_icons,
        &mut banks,
        &mut paths,
        &output,
    )?;
    visuals::write_visual_catalog(template, &resources, &defaults, &mut paths, &output)?;
    previews::write_previews(&item_catalog, &resources, &mut banks, &output)?;
    banks.write_overlay_sources(&output)?;
    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        configure_windows_binary(&output)?;
    }
    let mut archived_hashes = std::collections::BTreeMap::new();
    resources.write_archive(&output.join("regional-mod.tar.gz"), |source, contents| {
        archived_hashes.insert(source, file_fingerprints::checksum(contents));
    })?;
    release_fingerprints::write_catalog(
        &resources,
        &archived_hashes,
        &template.join("legacy.json"),
        &output,
    )?;
    jewel_icons.write_archive(&output.join("jewel-icons.tar.gz"), |_, _| {})
}

fn configure_windows_binary(output: &Path) -> io::Result<()> {
    let icon = "assets/ui/leah.ico";
    println!("cargo:rerun-if-changed={icon}");
    let binary = "MzModController";
    if env::var("TARGET").as_deref() == Ok("x86_64-pc-windows-gnullvm") {
        println!("cargo:rustc-link-arg-bin={binary}=-Wl,--no-insert-timestamp");
    }
    let author = env!("CARGO_PKG_AUTHORS");
    let version = RELEASE_VERSION;
    let major = env!("CARGO_PKG_VERSION_MAJOR");
    let minor = env!("CARGO_PKG_VERSION_MINOR");
    let patch = env!("CARGO_PKG_VERSION_PATCH");
    let script = output.join("controller.rc");
    fs::write(
        &script,
        format!(
            r#"#include <winver.h>
1 ICON "{icon}"
VS_VERSION_INFO VERSIONINFO
FILEVERSION {major},{minor},{patch},0
PRODUCTVERSION {major},{minor},{patch},0
FILEFLAGSMASK VS_FFI_FILEFLAGSMASK
FILEOS VOS_NT_WINDOWS32
FILETYPE VFT_APP
BEGIN
    BLOCK "StringFileInfo"
    BEGIN
        BLOCK "040904B0"
        BEGIN
            VALUE "CompanyName", "{author}"
            VALUE "FileDescription", "MzMod Controller"
            VALUE "FileVersion", "{version}"
            VALUE "InternalName", "{binary}"
            VALUE "OriginalFilename", "{binary}_{version}.exe"
            VALUE "ProductName", "MzMod"
            VALUE "ProductVersion", "{version}"
        END
    END
    BLOCK "VarFileInfo"
    BEGIN
        VALUE "Translation", 0x0409, 1200
    END
END
"#
        ),
    )?;
    embed_resource::compile_for(script, [binary], embed_resource::NONE)
        .manifest_required()
        .map_err(io::Error::other)
}
