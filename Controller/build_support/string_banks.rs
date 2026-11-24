//! Resolve template labels by region, bank, native id and case-sensitive key.
use std::{
    collections::{btree_map::Entry, BTreeMap},
    fs, io,
    path::{Path, PathBuf},
};

use serde_json::Value;

use crate::archive::RegionalResources;
use crate::chinese_overlay::{primary_bank_name, OverlaySources};
use crate::item_catalog::{string_bank_path, CHINESE_OVERLAY};
use crate::region::Region;

type RowsById = BTreeMap<u64, Vec<Value>>;

fn read_bank(path: &Path) -> io::Result<RowsById> {
    let rows: Vec<Value> = serde_json::from_slice(&fs::read(path)?)
        .map_err(|error| io::Error::other(format!("{}: {error}", path.display())))?;
    let mut by_id = RowsById::new();
    for row in rows {
        let id = row["id"]
            .as_u64()
            .ok_or_else(|| io::Error::other(format!("{}: missing string id", path.display())))?;
        by_id.entry(id).or_default().push(row);
    }
    Ok(by_id)
}

pub(super) struct TemplateStringBanks<'a> {
    resources: &'a RegionalResources,
    banks: BTreeMap<PathBuf, RowsById>,
    overlay_sources: Vec<OverlaySources>,
}

impl<'a> TemplateStringBanks<'a> {
    pub(super) fn read(resources: &'a RegionalResources) -> io::Result<Self> {
        let mut overlay_sources = Vec::new();
        for region in Region::ALL {
            let sources = read_overlay_sources(resources, region)
                .map_err(|error| io::Error::other(format!("{}: {error}", region.code())))?;
            overlay_sources.push(sources);
        }
        Ok(Self {
            resources,
            banks: BTreeMap::new(),
            overlay_sources,
        })
    }

    pub(super) fn overlay_sources(&self, region: Region) -> &OverlaySources {
        &self.overlay_sources[region.index()]
    }

    pub(super) fn write_overlay_sources(&self, output: &Path) -> io::Result<()> {
        let regions =
            Region::ALL.map(|region| format!("&{:?}", self.overlay_sources(region).entries()));
        fs::write(
            output.join("overlay_sources.rs"),
            format!("[{}]\n", regions.join(",\n")),
        )
    }

    pub(super) fn lookup(
        &mut self,
        region: Region,
        bank: &str,
        id: u64,
        key: &str,
    ) -> io::Result<&Value> {
        let path = self.resources.resolve(region, &string_bank_path(bank))?;
        let rows = match self.banks.entry(path) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => {
                let rows = read_bank(entry.key())?;
                entry.insert(rows)
            }
        };
        let mut matches = rows
            .get(&id)
            .into_iter()
            .flatten()
            .filter(|row| row["Key"].as_str() == Some(key));
        match (matches.next(), matches.next()) {
            (Some(row), None) => Ok(row),
            (None, _) => Err(io::Error::other(format!(
                "{}: missing template label {bank}/{id}/{key}",
                region.code()
            ))),
            _ => Err(io::Error::other(format!(
                "{}: duplicate template label {bank}/{id}/{key}",
                region.code()
            ))),
        }
    }
}

fn read_overlay_sources(
    resources: &RegionalResources,
    region: Region,
) -> io::Result<OverlaySources> {
    let path = resources.resolve(region, &string_bank_path(CHINESE_OVERLAY))?;
    let overlay = serde_json::from_reader(fs::File::open(path)?)
        .map_err(|error| io::Error::other(format!("{CHINESE_OVERLAY}: {error}")))?;
    let mut sources = OverlaySources::from_overlay(&overlay)?;
    for path in resources.game_paths(region) {
        if let Some(bank) = primary_bank_name(path) {
            let bytes = fs::read(resources.resolve(region, path)?)?;
            sources.index_bank(bank, &bytes)?;
        }
    }
    Ok(sources)
}
