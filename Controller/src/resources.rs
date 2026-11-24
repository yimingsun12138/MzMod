//! Game-relative resource paths and their file contents.
use anyhow::{Context, Result};
use serde_json::Value;
use std::collections::BTreeMap;

pub type ModFiles = BTreeMap<String, Vec<u8>>;

pub(crate) fn read_document(files: &ModFiles, path: &str) -> Result<Value> {
    let bytes = files.get(path).with_context(|| format!("missing {path}"))?;
    serde_json::from_slice(bytes).with_context(|| format!("invalid JSON in {path}"))
}

pub(crate) fn write_document(files: &mut ModFiles, path: &str, document: &Value) -> Result<()> {
    let mut bytes = serde_json::to_vec_pretty(document)?;
    bytes.push(b'\n');
    files.insert(path.to_owned(), bytes);
    Ok(())
}
