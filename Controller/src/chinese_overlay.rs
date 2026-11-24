//! Resolve the native string bank supplying each requested Chinese overlay translation.
use std::{collections::BTreeMap, io};

use serde::Deserialize;
use serde_json::Value;

use crate::item_catalog::{CHINESE_OVERLAY, STRING_DIRECTORY};

pub(crate) fn primary_bank_name(path: &str) -> Option<&str> {
    let bank = path.strip_prefix(STRING_DIRECTORY)?.strip_suffix(".json")?;
    (!bank.is_empty() && bank != CHINESE_OVERLAY && !bank.contains('/')).then_some(bank)
}

#[derive(Deserialize)]
struct StringIdentity {
    id: u64,
    #[serde(rename = "Key")]
    key: String,
}

#[derive(Default)]
struct OverlayRequest {
    source_bank: Option<String>,
    missing_target_text: bool,
}

pub(crate) struct OverlaySources {
    requests: BTreeMap<(u64, String), OverlayRequest>,
}

impl OverlaySources {
    pub(crate) fn from_overlay(overlay: &Value) -> io::Result<Self> {
        let rows = overlay.as_array().ok_or_else(|| {
            io::Error::other(format!("{CHINESE_OVERLAY}: expected a string bank"))
        })?;
        let mut requests = BTreeMap::<_, OverlayRequest>::new();
        for (index, row) in rows.iter().enumerate() {
            let identity = StringIdentity::deserialize(row).map_err(|error| {
                io::Error::other(format!("{CHINESE_OVERLAY} row {index}: {error}"))
            })?;
            let request = requests.entry((identity.id, identity.key)).or_default();
            request.missing_target_text |= !row["zhCN"].is_string();
        }
        Ok(Self { requests })
    }

    pub(crate) fn index_bank(&mut self, bank: &str, bytes: &[u8]) -> io::Result<()> {
        let rows: Vec<StringIdentity> = serde_json::from_slice(bytes)
            .map_err(|error| io::Error::other(format!("invalid source bank {bank}: {error}")))?;
        for row in rows {
            let identity = (row.id, row.key);
            if let Some(request) = self.requests.get_mut(&identity) {
                if let Some(previous) = &request.source_bank {
                    return Err(io::Error::other(format!(
                        "ambiguous Chinese overlay source {}/{}: {previous}, {bank}",
                        identity.0, identity.1
                    )));
                }
                request.source_bank = Some(bank.to_owned());
            }
        }
        Ok(())
    }

    /// A source that can change must be able to write every matching overlay row.
    pub(crate) fn validate_target(&self, bank: &str, id: u64, key: &str) -> io::Result<()> {
        if let Some(request) = self.requests.get(&(id, key.to_owned())) {
            if request.source_bank.as_deref() == Some(bank) && request.missing_target_text {
                return Err(io::Error::other(format!(
                    "{CHINESE_OVERLAY}/{id}/{key}/zhCN: expected a string for source {bank}"
                )));
            }
        }
        Ok(())
    }

    pub(crate) fn entries(&self) -> Vec<(&str, u64, &str)> {
        let mut entries: Vec<_> = self
            .requests
            .iter()
            .filter_map(|((id, key), request)| {
                Some((request.source_bank.as_deref()?, *id, key.as_str()))
            })
            .collect();
        entries.sort_unstable();
        entries
    }
}
