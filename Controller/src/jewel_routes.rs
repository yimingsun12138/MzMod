//! Resolve selected jewel rows in the native HD item tables.
use std::{collections::BTreeSet, fmt, io};

use serde_json::Value;

use crate::item_catalog::{misc_model_route, Jewels, ASSET_SLOTS};

#[derive(Clone, Copy)]
pub(crate) enum JewelRouteTable {
    Base,
    Unique,
}

impl JewelRouteTable {
    pub(crate) fn path(self) -> &'static str {
        match self {
            Self::Base => "data/hd/items/items.json",
            Self::Unique => "data/hd/items/uniques.json",
        }
    }

    fn invalid(self, detail: impl fmt::Display) -> io::Error {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{}: {detail}", self.path()),
        )
    }

    pub(crate) fn source(self, jewels: &Jewels, template: Option<&[u8]>) -> io::Result<Value> {
        match (template, self) {
            (Some(bytes), _) => serde_json::from_slice(bytes).map_err(|error| self.invalid(error)),
            (None, Self::Base) => Ok(jewels.native_base_routes.clone()),
            (None, Self::Unique) => Err(self.invalid("missing template resource")),
        }
    }

    /// Produce the regional table with its selected routes and the private models they require.
    pub(crate) fn redirected(
        self,
        jewels: &Jewels,
        mut document: Value,
    ) -> io::Result<(Value, impl Iterator<Item = &str>)> {
        let (routes, fields): (_, &[&str]) = match self {
            Self::Base => (&jewels.base_routes, &["asset"]),
            Self::Unique => (&jewels.unique_routes, &ASSET_SLOTS),
        };
        let mut pending: BTreeSet<_> = routes.keys().map(String::as_str).collect();
        let rows = document
            .as_array_mut()
            .ok_or_else(|| self.invalid("expected a table of item rows"))?;
        for (index, row) in rows.iter_mut().enumerate() {
            let entries = row
                .as_object_mut()
                .ok_or_else(|| self.invalid(format!("row {index}: expected an object")))?;
            for (key, entry) in entries {
                let Some(asset) = routes.get(key) else {
                    continue;
                };
                if !pending.remove(key.as_str()) {
                    return Err(self.invalid(format!("duplicate item {key}")));
                }
                for &field in fields {
                    match entry.get_mut(field) {
                        Some(Value::String(route)) if !route.trim().is_empty() => {
                            route.clone_from(asset)
                        }
                        _ => {
                            return Err(self.invalid(format!(
                                "{key}/{field}: expected a nonempty asset route"
                            )))
                        }
                    }
                }
            }
        }
        if let Some(key) = pending.first() {
            return Err(self.invalid(format!("missing item {key}")));
        }
        Ok((
            document,
            routes.values().map(|asset| misc_model_route(asset)),
        ))
    }
}
