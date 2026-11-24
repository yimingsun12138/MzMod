//! Record the generated files and verify the installed payload before releasing a backup.
use super::{ordinary_metadata, MOD_INFO};
use crate::{
    file_fingerprints::{checksum, read_file_hashes, FileHashes},
    game_path::{parent_paths, windows_path_key},
    region::BUILD_MARKER,
    ModFiles,
};
use anyhow::{ensure, Context, Result};
use serde::{de::Error, Deserializer};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

pub(super) fn fingerprints(files: &ModFiles) -> FileHashes {
    files
        .iter()
        .map(|(path, bytes)| (path.clone(), checksum(bytes)))
        .collect()
}

/// A controller-created installation always records its package identity and client build.
pub(super) fn read_manifest<'de, D: Deserializer<'de>>(input: D) -> Result<FileHashes, D::Error> {
    let files = read_file_hashes(input)?;
    for required in [MOD_INFO, BUILD_MARKER] {
        if !files.contains_key(required) {
            return Err(D::Error::custom(format!(
                "missing game resource record: {required}"
            )));
        }
    }
    Ok(files)
}

/// Before discarding a backup, check the saved fingerprints using actual directory entries.
pub(super) fn verify(package: &Path, files: &FileHashes) -> Result<()> {
    let mut expected = BTreeMap::new();
    for (path, digest) in files {
        for parent in parent_paths(path) {
            expected.entry(parent).or_insert(None);
        }
        expected.insert(path.as_str(), Some(digest.as_str()));
    }
    let mut found = BTreeSet::new();
    let mut directories = vec![(package.to_owned(), String::new())];
    while let Some((directory, prefix)) = directories.pop() {
        for entry in fs::read_dir(&directory).with_context(|| directory.display().to_string())? {
            let entry = entry?;
            let name = entry.file_name();
            let Some(name) = name.to_str() else { continue };
            if name.contains('\\') {
                continue;
            }
            let key = format!("{prefix}{}", windows_path_key(name));
            let Some((key, digest)) = expected.get_key_value(key.as_str()) else {
                continue;
            };
            ensure!(found.insert(*key), "游戏目录中有多个同名资源：{key}");
            let path = entry.path();
            let metadata = ordinary_metadata(&path).with_context(|| key.to_string())?;
            if let Some(digest) = digest {
                ensure!(metadata.is_file(), "游戏资源应为文件：{key}");
                let contents = fs::read(&path).with_context(|| key.to_string())?;
                ensure!(
                    checksum(&contents) == *digest,
                    "游戏资源与配置记录不符：{key}"
                );
            } else {
                ensure!(metadata.is_dir(), "游戏资源应为目录：{key}");
                directories.push((path, format!("{key}/")));
            }
        }
    }
    if let Some((missing, _)) = expected
        .iter()
        .find(|(path, digest)| digest.is_some() && !found.contains(**path))
    {
        anyhow::bail!("缺少已安装的游戏资源：{missing}");
    }
    Ok(())
}
