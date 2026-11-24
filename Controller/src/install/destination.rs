//! Resolve a game or portable output to the directory containing its regional installation.
use super::{
    existing::ExistingInstallation,
    ordinary_metadata,
    profile::InstalledProfile,
    transaction::{previous_install, Workspace},
    MOD_DIRECTORY,
};
use crate::{Region, Settings};
use anyhow::{ensure, Result};
use std::{
    fs,
    path::{Path, PathBuf},
};

fn empty_directory(path: &Path) -> bool {
    ordinary_metadata(path).is_ok_and(|metadata| metadata.is_dir())
        && fs::read_dir(path).is_ok_and(|mut entries| entries.next().is_none())
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Destination {
    Game(PathBuf),
    Export(PathBuf),
}

impl Destination {
    pub fn beside(directory: &Path, region: Region) -> Self {
        let in_mods = directory
            .file_name()
            .is_some_and(|name| name.eq_ignore_ascii_case("mods"));
        let shared = match directory.parent() {
            Some(game) if in_mods && game.join("D2R.exe").is_file() => Self::Game(game.to_owned()),
            _ => Self::Export(directory.to_owned()),
        };
        let existing = shared.existing();
        if ExistingInstallation::region_at(&existing) == Some(region) {
            return shared;
        }
        let regional = Self::Export(directory.join(region.code()));
        if existing.symlink_metadata().is_ok() && !empty_directory(&existing) {
            return regional;
        }
        let regional_install = regional.existing();
        // Keep an unreadable regional profile at its displayed path so the player can repair it.
        if regional_install.symlink_metadata().is_ok()
            && ExistingInstallation::region_at(&regional_install)
                .is_none_or(|installed| installed == region)
        {
            return regional;
        }
        shared
    }

    fn parent(&self) -> PathBuf {
        match self {
            Self::Game(game) => game.join("mods"),
            Self::Export(directory) => directory.clone(),
        }
    }

    pub fn live(&self) -> PathBuf {
        self.parent().join(MOD_DIRECTORY)
    }

    fn existing(&self) -> PathBuf {
        let live = self.live();
        if live.symlink_metadata().is_ok() {
            return live;
        }
        previous_install(&self.parent())
    }

    pub fn installed_region(&self) -> Option<Region> {
        ExistingInstallation::region_at(&self.existing())
    }

    pub(crate) fn read_context(&self) -> String {
        let directory = match self {
            Self::Game(directory) | Self::Export(directory) => directory,
        };
        format!("无法读取输出目录（{}）", directory.display())
    }

    fn validated_parent(&self) -> Result<PathBuf> {
        let parent = self.parent();
        match self {
            Self::Game(game) => {
                ensure!(game.is_dir(), "请选择实际的游戏安装目录。");
                ensure!(game.join("D2R.exe").is_file(), "此目录中找不到D2R.exe。");
            }
            Self::Export(directory) => {
                if directory.symlink_metadata().is_ok() {
                    ensure!(
                        ordinary_metadata(directory)?.is_dir(),
                        "输出位置不是目录：{}",
                        directory.display()
                    );
                }
            }
        }
        Ok(parent)
    }

    pub fn load(&self, region: Region) -> Result<Settings> {
        super::load_settings(&self.validated_parent()?, region)
    }

    pub(super) fn open_for_update(
        &self,
        region: Region,
    ) -> Result<(Workspace, Option<InstalledProfile>)> {
        let workspace = Workspace::open(&self.validated_parent()?)?;
        let previous = ExistingInstallation::profile_for(&workspace.live, region)?;
        Ok((workspace, previous))
    }

    pub fn apply(&self, region: Region, settings: &Settings) -> Result<()> {
        super::install_profile(self, region, |stored| {
            stored.clone_from(settings);
        })?;
        Ok(())
    }

    /// Apply this editor's changes to the latest profile and return the committed choices.
    pub fn apply_edits(
        &self,
        region: Region,
        baseline: &Settings,
        edited: &Settings,
    ) -> Result<Settings> {
        super::install_profile(self, region, |stored| {
            stored.merge_edits(baseline, edited);
        })
    }
}
