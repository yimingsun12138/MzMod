//! Read and write player choices and the fingerprints recorded with an installation.
mod fields;

use super::{ordinary_metadata, path_exists, payload, SETTINGS_FILE};
use crate::{
    file_fingerprints::{checksum, FileHashes},
    region::{parse_build_marker, BUILD_MARKER},
    template::CLIENT_BUILDS,
    ModFiles, Region, Settings,
};
use anyhow::{ensure, Context, Result};
use fields::{RawMembers, RecordFields};
use serde::{Deserialize, Serialize};
use std::{fs, io, path::Path};

const PROFILE_FORMAT: u32 = 1;

#[derive(Serialize, Deserialize)]
pub(super) struct InstalledProfile {
    format: u32,
    pub(super) region: Region,
    #[serde(deserialize_with = "read_saved_settings")]
    pub(super) settings: SavedSettings,
    #[serde(deserialize_with = "payload::read_manifest")]
    pub(super) files: FileHashes,
    #[serde(flatten, skip_deserializing)]
    additional: RawMembers,
}

#[derive(Serialize)]
pub(super) struct SavedSettings {
    #[serde(flatten)]
    pub(super) choices: Settings,
    #[serde(flatten)]
    additional: RawMembers,
}

fn read_saved_settings<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<SavedSettings, D::Error> {
    let mut fields = RecordFields::deserialize(deserializer)?;
    // Earlier profiles grouped the seven tomb symbols with other map entrances.
    if let Some(entrances) = fields.members.get("entrances").cloned() {
        fields
            .members
            .entry("tomb_symbols".into())
            .or_insert(entrances);
    }
    let (choices, additional) = fields.decode().map_err(serde::de::Error::custom)?;
    Ok(SavedSettings {
        choices,
        additional,
    })
}

impl InstalledProfile {
    fn read(reader: impl io::Read) -> serde_json::Result<Self> {
        let fields: RecordFields = serde_json::from_reader(reader)?;
        let (mut profile, additional) = fields.decode::<Self>()?;
        profile.additional = additional;
        Ok(profile)
    }

    pub(super) fn for_update(
        previous: Option<Self>,
        region: Region,
        choices: Settings,
        payload: &ModFiles,
    ) -> (Self, Option<FileHashes>) {
        let files = payload::fingerprints(payload);
        if let Some(mut profile) = previous {
            let previous_files = std::mem::replace(&mut profile.files, files);
            profile.settings.choices = choices;
            return (profile, Some(previous_files));
        }
        let profile = Self {
            format: PROFILE_FORMAT,
            region,
            settings: SavedSettings {
                choices,
                additional: RawMembers::new(),
            },
            files,
            additional: RawMembers::new(),
        };
        (profile, None)
    }

    pub(super) fn verify_region(&self, region: Region) -> Result<()> {
        ensure!(
            self.format == PROFILE_FORMAT && self.region == region,
            "已有配置的版本或区域不匹配。"
        );
        Ok(())
    }

    pub(super) fn recognizes_older_build(&self, build: &str) -> bool {
        let Some(number) = parse_build_marker(build) else {
            return false;
        };
        let versions = number
            .parse::<u64>()
            .ok()
            .zip(CLIENT_BUILDS.number(self.region).parse::<u64>().ok());
        if !versions.is_some_and(|(installed, current)| installed < current)
            || CLIENT_BUILDS.contains(number)
        {
            return false;
        }
        self.files
            .get(BUILD_MARKER)
            .is_some_and(|digest| *digest == checksum(build.as_bytes()))
    }

    pub(super) fn write_to(&self, directory: &Path) -> Result<()> {
        fs::write(
            directory.join(SETTINGS_FILE),
            serde_json::to_vec_pretty(self)?,
        )?;
        Ok(())
    }
}

pub(super) fn saved_profile(live: &Path) -> Result<Option<InstalledProfile>> {
    let file = live.join(SETTINGS_FILE);
    if !path_exists(&file)? {
        return Ok(None);
    }
    ensure!(ordinary_metadata(&file)?.is_file(), "控制器配置应为文件。");
    let profile =
        InstalledProfile::read(fs::File::open(file)?).context("无法读取已有的控制器配置")?;
    Ok(Some(profile))
}
