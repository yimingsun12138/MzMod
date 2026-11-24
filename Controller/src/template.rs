//! Load the shared resources and exactly one region's overrides.
use crate::{items, mod_info::MOD_INFO, visuals, ModFiles, Region, Settings};
use anyhow::{ensure, Context, Result};
use flate2::read::GzDecoder;
use std::io::Read;

pub(crate) struct ClientBuilds([&'static str; Region::ALL.len()]);

pub(crate) const CLIENT_BUILDS: ClientBuilds =
    ClientBuilds(include!(concat!(env!("OUT_DIR"), "/client_builds.rs")));

impl ClientBuilds {
    pub(crate) fn number(&self, region: Region) -> &'static str {
        self.0[region.index()]
    }

    pub(crate) fn contains(&self, number: &str) -> bool {
        self.0.contains(&number)
    }

    pub(crate) fn unique_region(&self, number: &str) -> Option<Region> {
        let mut matches = Region::ALL
            .into_iter()
            .filter(|&region| self.number(region) == number);
        let region = matches.next()?;
        matches.next().is_none().then_some(region)
    }
}

pub(crate) fn read_regional_files(archive: &[u8], region: Region) -> Result<ModFiles> {
    let mut files = ModFiles::new();
    for entry in tar::Archive::new(GzDecoder::new(archive)).entries()? {
        let mut entry = entry?;
        let path = entry.path()?;
        let path = path.to_str().context("invalid embedded resource path")?;
        let Some(name) = region.resource_path(path)? else {
            continue;
        };
        let name = name.to_owned();
        let mut contents = Vec::new();
        entry.read_to_end(&mut contents)?;
        ensure!(
            files.insert(name.clone(), contents).is_none(),
            "duplicate embedded resource: {name}"
        );
    }
    Ok(files)
}

pub(crate) struct RegionalTemplate {
    region: Region,
    pub(crate) files: ModFiles,
}

impl RegionalTemplate {
    pub(crate) fn read(region: Region) -> Result<Self> {
        let archive = include_bytes!(concat!(env!("OUT_DIR"), "/regional-mod.tar.gz"));
        let files = read_regional_files(archive, region)?;
        ensure!(files.contains_key(MOD_INFO), "incomplete embedded MzMod");
        Ok(Self { region, files })
    }

    pub(crate) fn render(mut self, settings: &Settings) -> Result<ModFiles> {
        let effective = settings.effective();
        items::render(&mut self.files, &effective, self.region)?;
        visuals::render(&mut self.files, &effective, self.region)?;
        Ok(self.files)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_unique_build_number_identifies_an_unlabelled_installation() {
        let builds = ClientBuilds(["101", "102"]);
        assert_eq!(builds.unique_region("102"), Some(Region::Cn));
        assert_eq!(builds.unique_region("100"), None);

        let shared = ClientBuilds(["103", "103"]);
        assert!(shared.contains("103"));
        assert_eq!(shared.unique_region("103"), None);
    }
}
