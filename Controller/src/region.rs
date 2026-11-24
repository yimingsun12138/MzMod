use serde::{Deserialize, Serialize};
use std::io;

pub(crate) const COMMON_GROUP: &str = "common";
pub(crate) const BUILD_MARKER: &str = "data/global/dataversionbuild.txt";

pub(crate) fn parse_build_marker(text: &str) -> Option<&str> {
    let number = text.strip_prefix('\u{feff}').unwrap_or(text).trim();
    (!number.is_empty() && number.bytes().all(|ch| ch.is_ascii_digit())).then_some(number)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Region {
    Int = 0,
    Cn = 1,
}

impl Region {
    /// Stable order of the regional entries in the controller's saved locations.
    pub const ALL: [Self; 2] = [Self::Int, Self::Cn];

    pub(crate) fn index(self) -> usize {
        self as usize
    }

    pub fn code(self) -> &'static str {
        ["INT", "CN"][self.index()]
    }

    /// Map a slash-separated archive entry to the game path visible in this region.
    pub(crate) fn resource_path(self, entry: &str) -> io::Result<Option<&str>> {
        let (group, path) = entry
            .split_once('/')
            .ok_or_else(|| io::Error::other("missing embedded resource group"))?;
        Ok((group == COMMON_GROUP || group == self.code()).then_some(path))
    }
}
