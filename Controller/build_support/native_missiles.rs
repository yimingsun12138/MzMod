//! Expand shared or regional native missile routes into controller region order.
use crate::region::Region;
use serde::{
    de::{self, MapAccess, Visitor},
    Deserialize, Deserializer,
};
use std::fmt;

#[derive(Debug)]
pub(super) struct NativeMissile(pub(super) [String; Region::ALL.len()]);

impl<'de> Deserialize<'de> for NativeMissile {
    fn deserialize<D: Deserializer<'de>>(input: D) -> Result<Self, D::Error> {
        struct Routes;

        impl<'de> Visitor<'de> for Routes {
            type Value = NativeMissile;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a missile route string or an object with a route for each region")
            }

            fn visit_str<E: de::Error>(self, value: &str) -> Result<Self::Value, E> {
                if value.trim().is_empty() {
                    return Err(E::custom("empty native missile route"));
                }
                Ok(NativeMissile(Region::ALL.map(|_| value.to_owned())))
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut routes = Region::ALL.map(|_| String::new());
                while let Some(region) = map.next_key::<Region>()? {
                    let slot = &mut routes[region.index()];
                    if !slot.is_empty() {
                        return Err(de::Error::duplicate_field(region.code()));
                    }
                    *slot = map.next_value().map_err(|error| {
                        de::Error::custom(format!("{}: {error}", region.code()))
                    })?;
                    if slot.trim().is_empty() {
                        return Err(de::Error::custom(format!(
                            "{}: empty native missile route",
                            region.code()
                        )));
                    }
                }
                for region in Region::ALL {
                    if routes[region.index()].is_empty() {
                        return Err(de::Error::missing_field(region.code()));
                    }
                }
                Ok(NativeMissile(routes))
            }
        }
        input.deserialize_any(Routes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn route_values_follow_region_codes_in_any_order() {
        for (input, expected) in [
            (r#""shared""#, ["shared", "shared"]),
            (
                r#"{"CN":"china","INT":"international"}"#,
                ["international", "china"],
            ),
        ] {
            let routes: NativeMissile = serde_json::from_str(input).unwrap();
            assert_eq!(routes.0, expected);
        }
    }

    #[test]
    fn every_native_route_is_a_string_and_every_region_is_known_and_unique() {
        for (input, detail) in [
            (r#"" ""#, "empty"),
            (r#"["a","b"]"#, "sequence"),
            (r#"{"INT":"a"}"#, "CN"),
            (r#"{"INT":"a","CN":"b","EU":"c"}"#, "EU"),
            (r#"{"INT":"a","CN":"b","INT":"c"}"#, "duplicate field `INT`"),
            (r#"{"INT":"a","CN":null}"#, "CN"),
            (r#"{"INT":"a","CN":""}"#, "CN"),
        ] {
            let error = serde_json::from_str::<NativeMissile>(input).unwrap_err();
            assert!(error.to_string().contains(detail), "{input}: {error}");
        }
    }
}
