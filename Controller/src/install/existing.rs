//! Identify an installed MzMod from its native metadata and optional controller profile.
use super::{
    ordinary_metadata, path_exists, payload,
    profile::{saved_profile, InstalledProfile},
    MOD_INFO, PACKAGE, SETTINGS_FILE,
};
use crate::{
    mod_info::{self, ModIdentity, MOD_NAME},
    region::{parse_build_marker, BUILD_MARKER},
    template::CLIENT_BUILDS,
    Region,
};
use anyhow::{ensure, Context, Result};
use std::{
    fs,
    path::{Path, PathBuf},
};

pub(super) struct ExistingInstallation {
    package: PathBuf,
    build_marker: String,
    profile: Option<InstalledProfile>,
}

impl ExistingInstallation {
    fn inspect(live: &Path) -> Result<Option<Self>> {
        if !path_exists(live)? {
            return Ok(None);
        }
        ensure!(ordinary_metadata(live)?.is_dir(), "MzMod安装路径不是目录。");
        if fs::read_dir(live)?.next().is_none() {
            return Ok(None);
        }
        let package = live.join(PACKAGE);
        ensure!(
            ordinary_metadata(&package)
                .context("找不到已安装的MzMod.mpq")?
                .is_dir(),
            "MzMod.mpq应为文件夹。"
        );
        let info: ModIdentity =
            mod_info::read(&read_identity_file(&package, MOD_INFO)?).context(MOD_INFO)?;
        ensure!(info.name == MOD_NAME, "所选目录包含其他模组：{MOD_INFO}");
        let build_marker =
            String::from_utf8(read_identity_file(&package, BUILD_MARKER)?).context(BUILD_MARKER)?;
        Ok(Some(Self {
            package,
            build_marker,
            profile: saved_profile(live)?,
        }))
    }

    fn verify_region(&self, region: Region) -> Result<()> {
        if let Some(profile) = &self.profile {
            profile.verify_region(region)?;
        }
        ensure!(
            parse_build_marker(&self.build_marker) == Some(CLIENT_BUILDS.number(region))
                || self
                    .profile
                    .as_ref()
                    .is_some_and(|profile| profile.recognizes_older_build(&self.build_marker)),
            "已安装的构建为{}，所选区域需要{}。",
            self.build_marker.trim(),
            CLIENT_BUILDS.number(region)
        );
        Ok(())
    }

    /// An unlabelled package identifies a region only when its build is unique.
    pub(super) fn region_at(live: &Path) -> Option<Region> {
        let install = Self::inspect(live).ok()??;
        match &install.profile {
            Some(profile) => {
                install.verify_region(profile.region).ok()?;
                Some(profile.region)
            }
            None => CLIENT_BUILDS.unique_region(parse_build_marker(&install.build_marker)?),
        }
    }

    pub(super) fn profile_for(live: &Path, region: Region) -> Result<Option<InstalledProfile>> {
        let Some(install) = Self::inspect(live)? else {
            return Ok(None);
        };
        install.verify_region(region)?;
        Ok(install.profile)
    }

    pub(super) fn verify_committed(live: &Path) -> Result<()> {
        let missing = || format!("缺少已安装的控制器配置：{SETTINGS_FILE}");
        let install = Self::inspect(live)?.with_context(missing)?;
        let profile = install.profile.as_ref().with_context(missing)?;
        payload::verify(&install.package, &profile.files)?;
        install.verify_region(profile.region)
    }
}

fn read_identity_file(package: &Path, relative: &str) -> Result<Vec<u8>> {
    let relative = Path::new(relative);
    let path = package.join(relative);
    let mut directory = package.to_path_buf();
    for parent in relative.parent().into_iter().flat_map(Path::components) {
        directory.push(parent);
        ensure!(
            ordinary_metadata(&directory)
                .with_context(|| directory.display().to_string())?
                .is_dir(),
            "游戏资源应为目录：{}",
            directory.display()
        );
    }
    ensure!(
        ordinary_metadata(&path)
            .with_context(|| path.display().to_string())?
            .is_file(),
        "游戏资源应为文件：{}",
        path.display()
    );
    fs::read(&path).with_context(|| path.display().to_string())
}
