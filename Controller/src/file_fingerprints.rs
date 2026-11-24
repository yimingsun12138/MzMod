//! SHA-256 fingerprints keyed by package-relative game paths.
use crate::game_path::{is_valid_game_path, parent_paths, windows_path_key};
use serde::{de::Error, Deserializer};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

pub(crate) type FileHashes = BTreeMap<String, String>;

pub(crate) fn checksum(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// Read one unambiguous entry per Windows file and normalize its SHA-256 spelling.
pub(crate) fn read_file_hashes<'de, D: Deserializer<'de>>(
    input: D,
) -> Result<FileHashes, D::Error> {
    let saved: FileHashes = crate::unique_map::deserialize(input)?;
    let mut files = FileHashes::new();
    for (path, digest) in saved {
        if !is_valid_game_path(&path) {
            return Err(D::Error::custom(format!("invalid game path {path:?}")));
        }
        if digest.len() != 64 || !digest.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(D::Error::custom(format!("invalid SHA-256 for {path:?}")));
        }
        if files
            .insert(windows_path_key(&path), digest.to_ascii_lowercase())
            .is_some()
        {
            return Err(D::Error::custom(format!(
                "duplicate Windows game path {path:?}"
            )));
        }
    }
    for path in files.keys() {
        if let Some(parent) = parent_paths(path).find(|parent| files.contains_key(*parent)) {
            return Err(D::Error::custom(format!(
                "file {parent:?} is also a directory for {path:?}"
            )));
        }
    }
    Ok(files)
}
