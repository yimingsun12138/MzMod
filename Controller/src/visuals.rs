//! Compose combat cues, scene guides and HUD choices over the regional template.
use anyhow::{Context, Result};
use serde_json::Value;

use crate::resources::read_document;
use crate::{ModFiles, Region, Settings};

#[cfg(test)]
mod tests;

struct VisualOverride {
    enabled: bool,
    paths_by_region: [&'static [&'static str]; Region::ALL.len()],
    native_missiles: &'static [(&'static str, [&'static str; Region::ALL.len()])],
}

include!(concat!(env!("OUT_DIR"), "/visual_catalog.rs"));

pub(crate) fn render(files: &mut ModFiles, settings: &Settings, region: Region) -> Result<()> {
    let overrides = visual_overrides(settings);
    render_missiles(files, &overrides, region)
        .with_context(|| format!("{} combat visuals", region.code()))?;
    for group in overrides {
        if !group.enabled {
            for &path in group.paths_by_region[region.index()] {
                files.remove(path);
            }
        }
    }
    Ok(())
}

fn render_missiles(
    files: &mut ModFiles,
    overrides: &[VisualOverride],
    region: Region,
) -> Result<()> {
    let owners = overrides
        .iter()
        .filter(|group| !group.native_missiles.is_empty());
    if owners.clone().all(|group| group.enabled) {
        return Ok(());
    }
    if owners.clone().all(|group| !group.enabled) {
        files.remove(MISSILES);
        return Ok(());
    }
    let mut missiles = read_document(files, MISSILES)?;
    for group in owners.filter(|group| !group.enabled) {
        for (name, native_visuals) in group.native_missiles {
            let entry = missiles
                .get_mut(*name)
                .with_context(|| format!("missing missile: {name}"))?;
            *entry = Value::String(native_visuals[region.index()].to_owned());
        }
    }
    files.insert(MISSILES.into(), serde_json::to_vec(&missiles)?);
    Ok(())
}
