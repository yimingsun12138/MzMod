//! Bind each independently selectable unique charm to its regional HD model.
use crate::item_catalog::{misc_model_route, Charms, ASSET_SLOTS};
use serde_json::Value;
use std::{collections::BTreeSet, io};

pub(crate) fn check_private_charm_routes(table: &Value, charms: &Charms) -> io::Result<()> {
    let bindings = [
        ("annihilus", charms.annihilus_route.as_str()),
        ("hellfire_torch", charms.hellfire_torch_route.as_str()),
    ];
    let mut seen = BTreeSet::new();
    let rows = table
        .as_array()
        .ok_or_else(|| io::Error::other("expected a table of unique items"))?;
    for (index, row) in rows.iter().enumerate() {
        let entries = row
            .as_object()
            .ok_or_else(|| io::Error::other(format!("row {index}: expected an object")))?;
        for (key, entry) in entries {
            let own = bindings
                .iter()
                .find(|(name, _)| name == key)
                .map(|(_, route)| *route);
            if own.is_some() && !seen.insert(key.as_str()) {
                return Err(io::Error::other(format!("duplicate unique charm {key}")));
            }
            for slot in ASSET_SLOTS {
                let route = entry[slot].as_str().map(misc_model_route);
                if let Some(expected) = own {
                    if route != Some(expected) {
                        return Err(io::Error::other(format!(
                            "{key}/{slot} requires {expected}, found {}",
                            entry[slot]
                        )));
                    }
                } else if let Some((owner, _)) =
                    bindings.iter().find(|(_, model)| Some(*model) == route)
                {
                    return Err(io::Error::other(format!(
                        "{key}/{slot} uses the model reserved for {owner}"
                    )));
                }
            }
        }
    }
    for (key, _) in bindings {
        if !seen.contains(key) {
            return Err(io::Error::other(format!("missing unique charm {key}")));
        }
    }
    Ok(())
}
