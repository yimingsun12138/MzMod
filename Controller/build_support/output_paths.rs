//! Check the source and owner of every controlled file in each region's Windows output.
use crate::game_path::{is_valid_game_path, parent_paths, windows_path_key};
use crate::region::Region;
use std::{collections::BTreeMap, fmt, io};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum FileSource {
    Template,
    Generated,
}

struct OutputFile {
    path: String,
    owner: Option<String>,
}

#[derive(Default)]
pub(crate) struct OutputPaths {
    regions: [BTreeMap<String, OutputFile>; Region::ALL.len()],
}

impl OutputPaths {
    pub(crate) fn from_template<'a>(
        paths: impl IntoIterator<Item = (Region, &'a str)>,
    ) -> io::Result<Self> {
        let mut outputs = Self::default();
        for (region, path) in paths {
            outputs.insert(region, path, None)?;
        }
        Ok(outputs)
    }

    pub(crate) fn claim(
        &mut self,
        region: Region,
        path: &str,
        source: FileSource,
        owner: impl fmt::Display,
    ) -> io::Result<()> {
        let code = region.code();
        if path.contains('\\') || !is_valid_game_path(path) {
            return Err(io::Error::other(format!(
                "{code}: invalid game path {path:?} for {owner}"
            )));
        }
        let files = &mut self.regions[region.index()];
        if let Some(file) = files.get_mut(&windows_path_key(path)) {
            if let Some(previous) = &file.owner {
                return Err(io::Error::other(format!(
                    "{code}: {path} claimed by both {previous} ({}) and {owner}",
                    file.path
                )));
            }
            if source == FileSource::Generated {
                return Err(io::Error::other(format!(
                    "{code}: generated {path} for {owner} conflicts with archived {}",
                    file.path
                )));
            }
            if file.path != path {
                return Err(io::Error::other(format!(
                    "{code}: {owner} requests {path}, archived as {}",
                    file.path
                )));
            }
            file.owner = Some(owner.to_string());
            return Ok(());
        }
        if source == FileSource::Template {
            return Err(io::Error::other(format!(
                "{code}: missing template resource {path} for {owner}"
            )));
        }
        self.insert(region, path, Some(owner.to_string()))
    }

    fn insert(&mut self, region: Region, path: &str, owner: Option<String>) -> io::Result<()> {
        let files = &mut self.regions[region.index()];
        let key = windows_path_key(path);
        let descendant_prefix = format!("{key}/");
        let ancestor = parent_paths(&key).find_map(|parent| files.get(parent));
        let descendant = files
            .range(descendant_prefix.clone()..)
            .next()
            .filter(|(other, _)| other.starts_with(&descendant_prefix))
            .map(|(_, file)| file);
        if let Some(other) = ancestor.or(descendant) {
            return Err(io::Error::other(format!(
                "{}: file and directory paths conflict: {path} and {}",
                region.code(),
                other.path
            )));
        }
        if files.contains_key(&key) {
            return Err(io::Error::other(format!(
                "{}: duplicate Windows output path {path}",
                region.code()
            )));
        }
        files.insert(
            key,
            OutputFile {
                path: path.to_owned(),
                owner,
            },
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use FileSource::{Generated, Template};
    use Region::{Cn, Int};

    #[test]
    fn ownership_follows_the_region_and_the_windows_destination() -> io::Result<()> {
        let mut paths =
            OutputPaths::from_template([(Int, "data/Name.json"), (Cn, "data/Name.json")])?;
        paths.claim(Int, "data/Name.json", Template, "labels")?;
        paths.claim(Cn, "data/Name.json", Template, "scene")?;
        let error = paths
            .claim(Int, "data/Name.json", Template, "scene")
            .unwrap_err()
            .to_string();
        assert!(error.contains("INT") && error.contains("labels") && error.contains("scene"));
        assert!(paths
            .claim(Cn, "data/absent.sprite", Template, "icons")
            .is_err());
        paths.claim(Cn, "data/Él.sprite", Generated, "jewels")?;
        assert!(paths
            .claim(Cn, "DATA/él.sprite", Generated, "runes")
            .is_err());
        assert!(paths
            .claim(Cn, "data/Él.sprite/child", Generated, "model")
            .is_err());
        assert!(paths.claim(Int, "DATA", Generated, "model").is_err());
        Ok(())
    }

    #[test]
    fn an_optional_file_requires_a_free_destination_in_its_region() -> io::Result<()> {
        let mut paths = OutputPaths::from_template([(Int, "items/ber.sprite")])?;
        assert!(paths
            .claim(Int, "items/BER.sprite", Generated, "jewels")
            .is_err());
        assert!(paths
            .claim(Int, "items/BER.sprite", Template, "runes")
            .is_err());
        paths.claim(Cn, "items/ber.sprite", Generated, "jewels")?;
        assert!(
            OutputPaths::from_template([(Int, "items/file"), (Int, "ITEMS/file/child")]).is_err()
        );
        Ok(())
    }
}
