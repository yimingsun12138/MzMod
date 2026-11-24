//! Compile the bytes each regional release may replace during installation.
use crate::archive::RegionalResources;
use crate::file_fingerprints::FileHashes;
use crate::game_path::windows_path_key;
use crate::region::Region;
use serde::Deserialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs, io,
    path::{Path, PathBuf},
};

#[derive(Deserialize)]
struct ReleaseHistory(
    #[serde(deserialize_with = "crate::unique_map::deserialize")] BTreeMap<String, LegacyFiles>,
);

#[derive(Deserialize)]
struct LegacyFiles(
    #[serde(deserialize_with = "crate::file_fingerprints::read_file_hashes")] FileHashes,
);

pub(super) fn write_catalog(
    resources: &RegionalResources,
    archived_hashes: &BTreeMap<PathBuf, String>,
    history_path: &Path,
    output: &Path,
) -> io::Result<()> {
    let ReleaseHistory(mut history) = serde_json::from_reader(fs::File::open(history_path)?)
        .map_err(|error| io::Error::other(format!("{}: {error}", history_path.display())))?;
    let mut regions = Vec::new();
    for region in Region::ALL {
        let LegacyFiles(legacy) = history.remove(region.code()).ok_or_else(|| {
            io::Error::other(format!("missing legacy fingerprints for {}", region.code()))
        })?;
        let mut known: BTreeSet<_> = legacy.into_iter().collect();
        for path in resources.game_paths(region) {
            let source = resources.resolve(region, path)?;
            let hash = archived_hashes.get(&source).ok_or_else(|| {
                io::Error::other(format!(
                    "{}: missing archived fingerprint for {path}",
                    region.code()
                ))
            })?;
            known.insert((windows_path_key(path), hash.clone()));
        }
        let entries: Vec<_> = known.into_iter().collect();
        regions.push(format!("&{entries:?}"));
    }
    if let Some(unknown) = history.keys().next() {
        return Err(io::Error::other(format!(
            "unknown legacy region {unknown:?}"
        )));
    }
    fs::write(
        output.join("release_fingerprints.rs"),
        format!("[{}]\n", regions.join(",\n")),
    )
}
