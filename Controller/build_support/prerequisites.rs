//! Compile template prerequisites into the effective settings used by the UI and game renderers.
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Write,
    fs, io,
    path::Path,
};

use crate::settings::TemplateDefaults;

// Resolve the dependency graph once at build time, including shared ancestors.
fn collect_prerequisites<'a>(
    feature: &'a str,
    dependencies: &BTreeMap<&'a str, &'a [String]>,
    visiting: &mut Vec<&'a str>,
    resolved: &mut BTreeMap<&'a str, BTreeSet<&'a str>>,
) -> io::Result<()> {
    let Some(direct) = dependencies.get(feature) else {
        return Ok(());
    };
    if resolved.contains_key(feature) {
        return Ok(());
    }
    if visiting.contains(&feature) {
        return Err(io::Error::other(format!(
            "cyclic setting prerequisite: {} -> {feature}",
            visiting.join(" -> ")
        )));
    }

    visiting.push(feature);
    let mut required = BTreeSet::new();
    for prerequisite in *direct {
        required.insert(prerequisite.as_str());
        collect_prerequisites(prerequisite, dependencies, visiting, resolved)?;
        if let Some(ancestors) = resolved.get(prerequisite.as_str()) {
            required.extend(ancestors);
        }
    }
    visiting.pop();
    resolved.insert(feature, required);
    Ok(())
}

pub(super) fn write_prerequisites<'a>(
    groups: impl Iterator<Item = (&'a str, &'a [String])>,
    defaults: &TemplateDefaults,
    output: &Path,
) -> io::Result<()> {
    let mut dependencies = BTreeMap::new();
    for (feature, direct) in groups {
        defaults.require_toggle(feature)?;
        let mut unique = BTreeSet::new();
        for prerequisite in direct {
            defaults.require_toggle(prerequisite)?;
            if !unique.insert(prerequisite) {
                return Err(io::Error::other(format!(
                    "{feature}: duplicate prerequisite {prerequisite}"
                )));
            }
        }
        if !direct.is_empty() {
            dependencies.insert(feature, direct);
        }
    }

    let mut resolved = BTreeMap::new();
    for &feature in dependencies.keys() {
        collect_prerequisites(feature, &dependencies, &mut Vec::new(), &mut resolved)?;
    }
    let mut rules = String::from("&[\n");
    for (feature, required) in resolved {
        let condition = required
            .into_iter()
            .map(|field| format!("settings.{field}"))
            .collect::<Vec<_>>()
            .join(" && ");
        writeln!(
            rules,
            "({feature:?}, ConditionalSetting {{ available: |settings| {condition}, value_mut: |settings| &mut settings.{feature} }}),"
        ).expect("writing to a string");
    }
    rules.push_str("]\n");
    fs::write(output.join("setting_prerequisites.rs"), rules)
}
