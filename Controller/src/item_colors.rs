//! Apply the regional equipment-name bindings within the shared HD profile.
use std::{collections::BTreeSet, io};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::region::Region;

pub(crate) const PROFILE_PATH: &str = "data/global/ui/layouts/_profilehd.json";
pub(crate) const ETHEREAL_FIELD: &str = "EtherealColor";
pub(crate) const SOCKETED_FIELD: &str = "SocketedColor";

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct EquipmentColors<T> {
    pub ethereal: T,
    pub socketed: T,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct NativeColor {
    reference: Option<String>,
    rgba: [u8; 4],
}

impl NativeColor {
    fn binding_for(&self, profile: &Value) -> io::Result<Value> {
        if let Some(reference) = &self.reference {
            let name = reference
                .strip_prefix('$')
                .filter(|name| !name.is_empty())
                .ok_or_else(|| {
                    io::Error::other(format!("invalid native color reference {reference:?}"))
                })?;
            if resolve_rgba(profile, &profile[name]).is_ok_and(|color| color == self.rgba) {
                return Ok(Value::String(reference.clone()));
            }
        }
        // Another feature can change a shared alias independently of equipment names.
        let [r, g, b, a] = self.rgba;
        Ok(json!({ "r": r, "g": g, "b": b, "a": a }))
    }
}

#[derive(Deserialize)]
#[serde(untagged, deny_unknown_fields)]
pub(crate) enum RegionalItemColors {
    Shared {
        common: EquipmentColors<NativeColor>,
    },
    Regional {
        #[serde(rename = "INT")]
        international: EquipmentColors<NativeColor>,
        #[serde(rename = "CN")]
        china: EquipmentColors<NativeColor>,
    },
}

impl RegionalItemColors {
    pub(crate) fn restore_bindings(&self, region: Region, profile: &mut Value) -> io::Result<()> {
        let colors = match (self, region) {
            (Self::Shared { common }, _) => common,
            (Self::Regional { international, .. }, Region::Int) => international,
            (Self::Regional { china, .. }, Region::Cn) => china,
        };
        for (field, native) in [
            (ETHEREAL_FIELD, &colors.ethereal),
            (SOCKETED_FIELD, &colors.socketed),
        ] {
            let binding = native
                .binding_for(profile)
                .map_err(|error| io::Error::other(format!("TooltipStyle.{field}: {error}")))?;
            let field_value = profile
                .get_mut("TooltipStyle")
                .and_then(|style| style.get_mut(field))
                .ok_or_else(|| io::Error::other(format!("missing TooltipStyle.{field}")))?;
            *field_value = binding;
        }
        Ok(())
    }
}

/// Resolve a literal or a chain of profile references to the four game color channels.
pub(crate) fn resolve_rgba<'a>(profile: &'a Value, mut value: &'a Value) -> io::Result<[u8; 4]> {
    let mut visited = BTreeSet::new();
    while let Some(name) = value.as_str().and_then(|s| s.strip_prefix('$')) {
        if !visited.insert(name) {
            return Err(io::Error::other(format!("cyclic color reference ${name}")));
        }
        value = profile
            .get(name)
            .ok_or_else(|| io::Error::other(format!("missing color reference ${name}")))?;
    }
    let channel = |key: &str| {
        u8::deserialize(&value[key])
            .map_err(|error| io::Error::other(format!("invalid color channel {key}: {error}")))
    };
    Ok([channel("r")?, channel("g")?, channel("b")?, channel("a")?])
}
