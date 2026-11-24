//! Compile visual file ownership and native missile routes into the controller.
use std::{collections::BTreeMap, fs, io, path::Path};

use serde::Deserialize;
use serde_json::Value;

use crate::archive::RegionalResources;
use crate::native_missiles::NativeMissile;
use crate::output_paths::{FileSource, OutputPaths};
use crate::region::Region;
use crate::settings::TemplateDefaults;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct VisualCatalog {
    missile_table: String,
    #[serde(deserialize_with = "crate::unique_map::deserialize")]
    overrides: BTreeMap<String, VisualOverride>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct VisualOverride {
    #[serde(default)]
    paths: Vec<String>,
    #[serde(default, deserialize_with = "crate::unique_map::deserialize")]
    regional_paths: BTreeMap<String, Vec<String>>,
    #[serde(default, deserialize_with = "crate::unique_map::deserialize")]
    native_missiles: BTreeMap<String, NativeMissile>,
    #[serde(default)]
    requires: Vec<String>,
}

#[derive(Deserialize)]
#[serde(transparent)]
struct MissileTable(
    #[serde(deserialize_with = "crate::unique_map::deserialize")] BTreeMap<String, Value>,
);

fn compile_file_paths(
    feature: &str,
    group: &VisualOverride,
    outputs: &mut OutputPaths,
) -> io::Result<String> {
    for code in group.regional_paths.keys() {
        if !Region::ALL.iter().any(|region| region.code() == code) {
            return Err(io::Error::other(format!(
                "{feature}: unknown region {code}"
            )));
        }
    }
    let mut slices = Vec::new();
    for region in Region::ALL {
        let mut paths: Vec<&str> = group.paths.iter().map(String::as_str).collect();
        if let Some(regional) = group.regional_paths.get(region.code()) {
            paths.extend(regional.iter().map(String::as_str));
        }
        for &path in &paths {
            outputs.claim(region, path, FileSource::Template, feature)?;
        }
        slices.push(format!("&{paths:?}"));
    }
    Ok(format!("[{}]", slices.join(", ")))
}

fn validate_missile_routes(
    resources: &RegionalResources,
    catalog: &VisualCatalog,
) -> io::Result<()> {
    let mut owners = BTreeMap::new();
    for (feature, group) in &catalog.overrides {
        for key in group.native_missiles.keys() {
            if let Some(other) = owners.insert(key, feature) {
                return Err(io::Error::other(format!(
                    "visuals.json: {feature} and {other} both own missile {key:?}"
                )));
            }
        }
    }
    if owners.is_empty() {
        return Err(io::Error::other("missile table has no declared routes"));
    }
    for region in Region::ALL {
        let source = resources.resolve(region, &catalog.missile_table)?;
        let MissileTable(table) =
            serde_json::from_reader(fs::File::open(&source)?).map_err(|error| {
                io::Error::other(format!(
                    "{}/{}: {error}",
                    region.code(),
                    catalog.missile_table
                ))
            })?;
        for (name, feature) in &owners {
            if !table
                .get(name.as_str())
                .and_then(Value::as_str)
                .is_some_and(|route| !route.trim().is_empty())
            {
                return Err(io::Error::other(format!(
                    "{}/{}, {feature}: expected a nonempty missile route at {name:?}",
                    region.code(),
                    catalog.missile_table
                )));
            }
        }
    }

    Ok(())
}

pub(super) fn write_visual_catalog(
    template: &Path,
    resources: &RegionalResources,
    defaults: &TemplateDefaults,
    paths: &mut OutputPaths,
    output: &Path,
) -> io::Result<()> {
    let catalog: VisualCatalog =
        serde_json::from_reader(fs::File::open(template.join("visuals.json"))?)
            .map_err(|error| io::Error::other(format!("visuals.json: {error}")))?;
    for region in Region::ALL {
        paths.claim(
            region,
            &catalog.missile_table,
            FileSource::Template,
            "shared missile table",
        )?;
    }
    validate_missile_routes(resources, &catalog)?;
    crate::prerequisites::write_prerequisites(
        catalog
            .overrides
            .iter()
            .map(|(feature, group)| (feature.as_str(), group.requires.as_slice())),
        defaults,
        output,
    )?;
    let mut code = format!("const MISSILES: &str = {:?};\n", catalog.missile_table);
    let mut selections = Vec::new();
    for (feature, group) in &catalog.overrides {
        let paths_by_region = compile_file_paths(feature, group, paths)?;
        if group.paths.is_empty()
            && group.regional_paths.values().all(Vec::is_empty)
            && group.native_missiles.is_empty()
        {
            if group.requires.is_empty() {
                return Err(io::Error::other(format!(
                    "{feature}: empty visual override"
                )));
            }
            continue;
        }
        let routes: Vec<_> = group
            .native_missiles
            .iter()
            .map(|(key, route)| (key, &route.0))
            .collect();
        selections.push(format!(
            "VisualOverride {{ enabled: settings.{feature}, paths_by_region: {paths_by_region}, native_missiles: &{routes:?} }},"
        ));
    }
    code.push_str(&format!(
        "fn visual_overrides(settings: &Settings) -> [VisualOverride; {}] {{ [\n{}\n] }}\n",
        selections.len(),
        selections.join("\n")
    ));
    fs::write(output.join("visual_catalog.rs"), code)
}
