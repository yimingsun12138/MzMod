//! Compile default values and per-choice updates from the template's field shapes.
use serde::Deserialize;
use std::{collections::BTreeMap, fs, io, io::Write, path::Path};

#[derive(Deserialize)]
#[serde(untagged)]
enum Choice {
    Toggle(bool),
    Selection(Vec<bool>),
}

#[derive(Deserialize)]
#[serde(transparent)]
pub(super) struct TemplateDefaults(
    #[serde(deserialize_with = "crate::unique_map::deserialize")] BTreeMap<String, Choice>,
);

impl TemplateDefaults {
    pub(super) fn read(template: &Path) -> io::Result<Self> {
        let file = fs::File::open(template.join("defaults.json"))?;
        serde_json::from_reader(file).map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("defaults.json: {error}"),
            )
        })
    }

    pub(super) fn require_toggle(&self, name: &str) -> io::Result<()> {
        match self.0.get(name) {
            Some(Choice::Toggle(_)) => Ok(()),
            Some(Choice::Selection(_)) => Err(io::Error::other(format!(
                "visuals.json: {name:?} names a list of choices, expected a single toggle"
            ))),
            None => Err(io::Error::other(format!(
                "visuals.json: {name:?} is missing from defaults.json"
            ))),
        }
    }

    // Rust checks the complete field set and fixed array lengths against Settings.
    pub(super) fn write_settings_code(&self, output: &Path) -> io::Result<()> {
        let mut defaults = Vec::from(b"Self {\n");
        let mut merged = Vec::from(b"Self {\n");
        for (name, choice) in &self.0 {
            if !name.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
                || !name
                    .bytes()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_')
            {
                return Err(io::Error::other(format!(
                    "defaults.json: invalid field {name:?}"
                )));
            }
            let merge = match choice {
                Choice::Toggle(value) => {
                    writeln!(defaults, "{name}: {value},")?;
                    "merge_toggle"
                }
                Choice::Selection(values) => {
                    writeln!(defaults, "{name}: {values:?},")?;
                    "merge_selection"
                }
            };
            writeln!(
                merged,
                "{name}: {merge}(self.{name}, baseline.{name}, edited.{name}),"
            )?;
        }
        defaults.extend_from_slice(b"}\n");
        merged.extend_from_slice(b"}\n");
        fs::write(output.join("default_settings.rs"), defaults)?;
        fs::write(output.join("merged_settings.rs"), merged)
    }
}
