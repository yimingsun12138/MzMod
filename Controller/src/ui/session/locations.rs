//! Persist the selected region and each region's game directory across controller windows.
use crate::{install::Destination, Region};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, OpenOptions},
    io,
    path::{Path, PathBuf},
};

#[derive(Default, Serialize, Deserialize)]
struct OutputLocations {
    selected: Option<Region>,
    paths: [Option<PathBuf>; Region::ALL.len()],
    #[serde(flatten)]
    additional: serde_json::Map<String, serde_json::Value>,
}

impl OutputLocations {
    fn inferred(exe_directory: &Path) -> Self {
        Self {
            selected: Region::ALL.into_iter().find(|&region| {
                Destination::beside(exe_directory, region).installed_region() == Some(region)
            }),
            ..Self::default()
        }
    }

    fn read(path: &Path) -> Result<Option<Self>> {
        match fs::read(path) {
            Ok(bytes) => Ok(Some(serde_json::from_slice(&bytes)?)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        }
    }
}

pub(super) enum LocationChange {
    SelectRegion(Region),
    SetGameDirectory(Region, Option<PathBuf>),
}

impl LocationChange {
    fn apply(&self, locations: &mut OutputLocations) {
        match self {
            Self::SelectRegion(region) => locations.selected = Some(*region),
            Self::SetGameDirectory(region, directory) => {
                locations.paths[region.index()].clone_from(directory);
            }
        }
    }
}

#[derive(Default)]
pub(super) struct LocationPreferences {
    file: Option<PathBuf>,
    current: OutputLocations,
}

impl LocationPreferences {
    pub(super) fn open(exe_directory: &Path) -> Self {
        let file = std::env::var_os("LOCALAPPDATA")
            .map(|root| PathBuf::from(root).join("MzMod/Controller/locations.json"));
        Self::load(file, exe_directory)
    }

    fn load(file: Option<PathBuf>, exe_directory: &Path) -> Self {
        let current = file
            .as_deref()
            .and_then(|path| OutputLocations::read(path).ok().flatten())
            .unwrap_or_else(|| OutputLocations::inferred(exe_directory));
        Self { file, current }
    }

    pub(super) fn region(&self) -> Region {
        self.current.selected.unwrap_or(Region::Int)
    }

    pub(super) fn game_directory(&self, region: Region) -> Option<&Path> {
        self.current.paths[region.index()].as_deref()
    }

    pub(super) fn destination(&self, exe_directory: &Path, region: Region) -> Destination {
        match self.game_directory(region) {
            Some(path) => Destination::Game(path.to_owned()),
            None => Destination::beside(exe_directory, region),
        }
    }

    pub(super) fn remember(&mut self, change: LocationChange) -> Result<()> {
        change.apply(&mut self.current);
        let Some(path) = &self.file else {
            return Ok(());
        };
        fs::create_dir_all(path.parent().context("missing settings directory")?)?;
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path.with_extension("lock"))?;
        lock.lock()?;
        let mut saved = OutputLocations::read(path)?.unwrap_or_else(|| OutputLocations {
            selected: self.current.selected,
            ..OutputLocations::default()
        });
        change.apply(&mut saved);
        let staged = path.with_extension("tmp");
        let bytes = serde_json::to_vec(&saved)?;
        if let Err(error) = fs::write(&staged, bytes).and_then(|()| fs::rename(&staged, path)) {
            let _ = fs::remove_file(&staged);
            return Err(error.into());
        }
        if let LocationChange::SelectRegion(region) = change {
            self.current.paths[region.index()] = saved.paths[region.index()].take();
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::TestDirectory;
    use super::*;
    use LocationChange::{SelectRegion, SetGameDirectory};
    use Region::{Cn, Int};

    #[test]
    fn windows_remember_each_location_change_independently() -> Result<()> {
        let root = TestDirectory::new("location-windows");
        let file = root.0.join("locations.json");
        fs::write(
            &file,
            r#"{"selected":"INT","paths":[null,null],"extra":42}"#,
        )?;
        let open = || LocationPreferences::load(Some(file.clone()), &root.0);
        let mut first = open();
        let mut second = open();
        second.remember(SelectRegion(Cn))?;

        let start = std::sync::Barrier::new(2);
        std::thread::scope(|scope| -> Result<()> {
            let other = scope.spawn(|| {
                start.wait();
                second.remember(SetGameDirectory(Cn, Some("china".into())))
            });
            start.wait();
            first.remember(SetGameDirectory(Int, Some("global".into())))?;
            other.join().unwrap()
        })?;
        let stored = open();
        assert_eq!(stored.region(), Cn);
        assert_eq!(stored.game_directory(Int), Some(Path::new("global")));
        assert_eq!(stored.game_directory(Cn), Some(Path::new("china")));
        assert_eq!(first.region(), Int);

        first.remember(SetGameDirectory(Int, None))?;
        assert_eq!(open().region(), Cn);
        first.remember(SelectRegion(Cn))?;
        assert_eq!(first.game_directory(Cn), Some(Path::new("china")));
        second.remember(SelectRegion(Int))?;
        assert_eq!(second.game_directory(Int), None);
        assert_eq!(open().current.additional["extra"], 42);
        Ok(())
    }

    #[test]
    fn first_save_uses_the_portable_region_and_preserves_an_unreadable_file() -> Result<()> {
        let root = TestDirectory::new("location-inference");
        let file = root.0.join("locations.json");
        let portable = Destination::Export(root.0.clone());
        portable.apply(Cn, &crate::Settings::default())?;
        let mut preferences = LocationPreferences::load(Some(file.clone()), &root.0);
        assert_eq!(preferences.region(), Cn);
        assert!(!file.exists());
        fs::rename(portable.live(), root.0.join("moved"))?;
        preferences.remember(SetGameDirectory(Cn, Some("first".into())))?;
        assert_eq!(OutputLocations::read(&file)?.unwrap().selected, Some(Cn));

        let unreadable = b"incomplete settings";
        fs::write(&file, unreadable)?;
        assert!(preferences.remember(SetGameDirectory(Cn, None)).is_err());
        assert_eq!(preferences.game_directory(Cn), None);
        assert_eq!(fs::read(file)?, unreadable);
        Ok(())
    }
}
