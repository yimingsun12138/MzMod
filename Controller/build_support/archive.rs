use crate::game_path::{is_valid_game_path, windows_path_key};
use crate::region::{Region, COMMON_GROUP};
use flate2::{Compression, GzBuilder};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{self, BufWriter, Write},
    path::{Path, PathBuf},
};
use tar::{Builder, Header};

fn collect_files(
    root: &Path,
    directory: &Path,
    files: &mut BTreeMap<PathBuf, PathBuf>,
) -> io::Result<()> {
    for entry in fs::read_dir(root.join(directory))? {
        let entry = entry?;
        let relative = directory.join(entry.file_name());
        let kind = entry.file_type()?;
        if kind.is_dir() {
            collect_files(root, &relative, files)?;
        } else if kind.is_file() {
            files.insert(relative, entry.path());
        } else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("unsupported template entry: {}", relative.display()),
            ));
        }
    }
    Ok(())
}

/// Maps archive names and regional game paths to their physical source files.
pub(super) struct RegionalResources {
    entries: BTreeMap<PathBuf, PathBuf>,
    regional_paths: [BTreeMap<String, PathBuf>; Region::ALL.len()],
}

impl RegionalResources {
    pub(super) fn read(root: &Path) -> io::Result<Self> {
        let mut entries = BTreeMap::new();
        for group in std::iter::once(COMMON_GROUP).chain(Region::ALL.map(Region::code)) {
            if root.join(group).try_exists()? {
                collect_files(root, Path::new(group), &mut entries)?;
            }
        }
        Self::from_entries(entries)
    }

    fn from_entries(entries: BTreeMap<PathBuf, PathBuf>) -> io::Result<Self> {
        let mut regional_paths: [_; Region::ALL.len()] = Default::default();
        for region in Region::ALL {
            regional_paths[region.index()] = index_region(region, &entries)?;
        }
        Ok(Self {
            entries,
            regional_paths,
        })
    }

    /// Add only the files compiled by this build, even when Cargo reuses OUT_DIR.
    pub(super) fn with_generated(
        mut self,
        generated: BTreeMap<PathBuf, PathBuf>,
    ) -> io::Result<Self> {
        for (name, source) in generated {
            if self.entries.insert(name.clone(), source).is_some() {
                return Err(io::Error::other(format!(
                    "generated resource conflicts with template: {}",
                    name.display()
                )));
            }
        }
        Self::from_entries(self.entries)
    }

    pub(super) fn find(&self, region: Region, game_path: &str) -> Option<PathBuf> {
        self.regional_paths[region.index()].get(game_path).cloned()
    }

    pub(super) fn game_paths(&self, region: Region) -> impl Iterator<Item = &str> {
        self.regional_paths[region.index()]
            .keys()
            .map(String::as_str)
    }

    pub(super) fn resolve(&self, region: Region, game_path: &str) -> io::Result<PathBuf> {
        self.find(region, game_path).ok_or_else(|| {
            io::Error::other(format!(
                "{}: missing template resource {game_path}",
                region.code()
            ))
        })
    }

    pub(super) fn write_archive(
        &self,
        output: &Path,
        mut on_file: impl FnMut(PathBuf, &[u8]),
    ) -> io::Result<()> {
        let writer = BufWriter::new(fs::File::create(output)?);
        let gzip = GzBuilder::new().mtime(0).write(writer, Compression::best());
        let mut archive = Builder::new(gzip);
        for (path, source) in &self.entries {
            let contents = fs::read(source)?;
            if path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("json"))
            {
                crate::game_json::validate(&contents)
                    .map_err(|error| io::Error::other(format!("{}: {error}", path.display())))?;
            }
            let mut header = Header::new_gnu();
            header.set_size(contents.len() as u64);
            header.set_mode(0o644);
            header.set_mtime(0);
            header.set_uid(0);
            header.set_gid(0);
            archive.append_data(&mut header, path, contents.as_slice())?;
            on_file(source.clone(), &contents);
        }
        archive.into_inner()?.finish()?.flush()
    }
}

fn index_region(
    region: Region,
    entries: &BTreeMap<PathBuf, PathBuf>,
) -> io::Result<BTreeMap<String, PathBuf>> {
    let mut paths = BTreeMap::new();
    let mut windows_names = BTreeSet::new();
    for (entry, source) in entries {
        let archive_path = entry
            .to_str()
            .ok_or_else(|| io::Error::other("non-UTF-8 game path"))?
            .replace(std::path::MAIN_SEPARATOR, "/");
        let Some(name) = region.resource_path(&archive_path)? else {
            continue;
        };
        if name.contains('\\') || !is_valid_game_path(name) {
            return Err(io::Error::other(format!(
                "{}: invalid game path {name:?}",
                region.code()
            )));
        }
        if !windows_names.insert(windows_path_key(name)) {
            return Err(io::Error::other(format!(
                "{}: duplicate resource path {name} in {}",
                region.code(),
                source.display()
            )));
        }
        paths.insert(name.to_owned(), source.clone());
    }
    Ok(paths)
}
