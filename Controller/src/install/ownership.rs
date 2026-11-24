//! Identify controller-managed resources and carry the player's other files forward.
use super::{ordinary_metadata, PACKAGE, SETTINGS_FILE};
use crate::file_fingerprints::{checksum, FileHashes};
use crate::game_path::{parent_paths, windows_path_key};
use crate::Region;
use anyhow::{bail, ensure, Context, Result};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

const RELEASE_FINGERPRINTS: [&[(&str, &str)]; Region::ALL.len()] =
    include!(concat!(env!("OUT_DIR"), "/release_fingerprints.rs"));

#[derive(Default)]
pub(super) struct ManagedFiles<'a> {
    hashes: BTreeMap<String, BTreeSet<&'a str>>,
    required_directories: BTreeSet<String>,
}

impl<'a> ManagedFiles<'a> {
    pub(super) fn for_update(
        region: Region,
        previous_profile: Option<&'a FileHashes>,
        next_profile: &'a FileHashes,
    ) -> Self {
        let mut managed = Self::default();
        for &(path, hash) in RELEASE_FINGERPRINTS[region.index()] {
            managed.allow(path, hash);
        }
        for (path, hash) in previous_profile.into_iter().flatten().chain(next_profile) {
            managed.allow(path, hash);
        }
        for path in next_profile.keys() {
            let key = windows_path_key(&format!("{PACKAGE}/{path}"));
            managed
                .required_directories
                .extend(parent_paths(&key).map(str::to_owned));
        }
        managed
    }

    fn allow(&mut self, path: &str, hash: &'a str) {
        let key = windows_path_key(&format!("{PACKAGE}/{path}"));
        self.hashes.entry(key).or_default().insert(hash);
    }

    pub(super) fn preserve_unmanaged(&self, live: &Path, stage: &Path) -> Result<()> {
        let mut conflicts = Vec::new();
        self.copy_directory(live, stage, Path::new(""), &mut conflicts)?;
        conflicts.sort();
        ensure!(
            conflicts.is_empty(),
            "发现外部修改，本次设置未应用：{}",
            conflicts.join("\n")
        );
        Ok(())
    }

    fn copy_directory(
        &self,
        live: &Path,
        stage: &Path,
        relative_dir: &Path,
        conflicts: &mut Vec<String>,
    ) -> Result<()> {
        let directory = live.join(relative_dir);
        let directory_name = || directory.display().to_string();
        for entry in fs::read_dir(&directory).with_context(directory_name)? {
            let entry = entry.with_context(directory_name)?;
            let relative = relative_dir.join(entry.file_name());
            let name = || relative.display().to_string();
            let source = entry.path();
            let destination = stage.join(&relative);
            let metadata = ordinary_metadata(&source).with_context(name)?;
            let key = windows_path_key(&relative.to_string_lossy());
            if key == SETTINGS_FILE {
                continue;
            }
            if let Some(hashes) = self.hashes.get(&key) {
                if metadata.is_file() {
                    let contents = fs::read(&source).with_context(name)?;
                    if hashes.contains(checksum(&contents).as_str()) {
                        continue;
                    }
                }
                conflicts.push(name());
            } else if metadata.is_dir() {
                fs::create_dir_all(&destination).with_context(name)?;
                self.copy_directory(live, stage, &relative, conflicts)?;
            } else if self.required_directories.contains(&key) {
                conflicts.push(name());
            } else if metadata.is_file() {
                fs::copy(source, destination).with_context(name)?;
            } else {
                bail!("无法保留特殊文件：{}", source.display());
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{install::payload, template::RegionalTemplate};

    #[test]
    fn release_fingerprints_match_the_archived_files_and_regional_history() -> Result<()> {
        let history: BTreeMap<String, FileHashes> =
            serde_json::from_slice(include_bytes!("../../../ModTemplate/legacy.json"))?;
        for region in Region::ALL {
            let template = RegionalTemplate::read(region)?;
            let current = payload::fingerprints(&template.files);
            let expected: BTreeSet<_> = current
                .iter()
                .chain(&history[region.code()])
                .map(|(path, hash)| (windows_path_key(path), hash.to_ascii_lowercase()))
                .collect();
            let compiled: Vec<_> = RELEASE_FINGERPRINTS[region.index()]
                .iter()
                .map(|&(path, hash)| (path.to_owned(), hash.to_owned()))
                .collect();
            assert!(
                compiled.iter().eq(&expected),
                "{} release fingerprints",
                region.code()
            );
        }
        Ok(())
    }
}
