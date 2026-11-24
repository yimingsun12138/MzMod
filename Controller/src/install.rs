//! Build and install the selected regional profile at a validated destination.
mod destination;
mod existing;
mod ownership;
mod payload;
mod profile;
mod transaction;

#[cfg(test)]
mod tests;

use crate::{
    mod_info::{MOD_INFO, MOD_NAME as MOD_DIRECTORY},
    template::RegionalTemplate,
    Region, Settings,
};
use anyhow::{ensure, Context, Result};
use existing::ExistingInstallation;
use ownership::ManagedFiles;
use profile::InstalledProfile;
use std::{fs, io, path::Path};
use transaction::Workspace;

pub use destination::Destination;

const SETTINGS_FILE: &str = "controller-settings.json";
const PACKAGE: &str = "MzMod.mpq";

fn ordinary_metadata(path: &Path) -> Result<fs::Metadata> {
    let metadata = fs::symlink_metadata(path)?;
    #[cfg(windows)]
    let linked = {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    };
    #[cfg(not(windows))]
    let linked = metadata.is_symlink();
    ensure!(!linked, "此路径是链接，请选择实际目录：{}", path.display());
    Ok(metadata)
}

fn path_exists(path: &Path) -> io::Result<bool> {
    match path.symlink_metadata() {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

fn load_settings(parent: &Path, region: Region) -> Result<Settings> {
    let _workspace = Workspace::open_existing(parent)?;
    let profile = ExistingInstallation::profile_for(&parent.join(MOD_DIRECTORY), region)?;
    Ok(profile
        .map(|saved| saved.settings.choices)
        .unwrap_or_default())
}

fn install_profile(
    destination: &Destination,
    region: Region,
    update: impl FnOnce(&mut Settings),
) -> Result<Settings> {
    let (workspace, previous) = destination
        .open_for_update(region)
        .with_context(|| destination.read_context())?;
    let mut settings = previous
        .as_ref()
        .map(|profile| profile.settings.choices.clone())
        .unwrap_or_default();
    update(&mut settings);
    let template = RegionalTemplate::read(region)?;
    let output = template.render(&settings)?;
    let (profile, previous_files) =
        InstalledProfile::for_update(previous, region, settings, &output);

    workspace.replace(|live, stage| {
        if path_exists(live)? {
            let managed = ManagedFiles::for_update(region, previous_files.as_ref(), &profile.files);
            managed.preserve_unmanaged(live, stage)?;
        }
        let package = stage.join(PACKAGE);
        for (path, data) in output {
            let destination = package.join(path);
            fs::create_dir_all(destination.parent().context("invalid resource path")?)?;
            fs::write(destination, data)?;
        }
        profile.write_to(stage)
    })?;
    Ok(profile.settings.choices)
}

pub fn load(game: &Path, region: Region) -> Result<Settings> {
    Destination::Game(game.to_owned()).load(region)
}

pub fn apply(game: &Path, region: Region, settings: &Settings) -> Result<()> {
    Destination::Game(game.to_owned()).apply(region, settings)
}
