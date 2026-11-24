//! Validate the game metadata the installer will use to recognize each client's output.
use crate::{
    archive::RegionalResources,
    mod_info::{self, ModIdentity, MOD_INFO, MOD_NAME},
    output_paths::{FileSource, OutputPaths},
    region::{parse_build_marker, Region, BUILD_MARKER},
};
use serde::Deserialize;
use std::{fs, io};

#[derive(Deserialize)]
struct ReleaseInfo {
    #[serde(flatten)]
    identity: ModIdentity,
    author: String,
    savepath: String,
}

pub(super) fn client_builds(
    resources: &RegionalResources,
    paths: &mut OutputPaths,
) -> io::Result<Vec<String>> {
    let mut builds = Vec::new();
    for region in Region::ALL {
        for path in [MOD_INFO, BUILD_MARKER] {
            paths.claim(region, path, FileSource::Template, "installation identity")?;
        }
        let info: ReleaseInfo = mod_info::read(&fs::read(resources.resolve(region, MOD_INFO)?)?)
            .map_err(|error| io::Error::other(format!("{}/{MOD_INFO}: {error}", region.code())))?;
        for (field, value, expected) in [
            ("name", info.identity.name.as_str(), MOD_NAME),
            ("savepath", info.savepath.as_str(), "../"),
            ("author", info.author.as_str(), env!("CARGO_PKG_AUTHORS")),
        ] {
            if value != expected {
                return Err(io::Error::other(format!(
                    "{}/{MOD_INFO}: expected {field} {expected:?}, found {value:?}",
                    region.code()
                )));
            }
        }
        let marker = fs::read_to_string(resources.resolve(region, BUILD_MARKER)?)?;
        let number = parse_build_marker(&marker).ok_or_else(|| {
            io::Error::other(format!(
                "{}/{BUILD_MARKER}: invalid client build marker",
                region.code()
            ))
        })?;
        builds.push(number.to_owned());
    }
    Ok(builds)
}
