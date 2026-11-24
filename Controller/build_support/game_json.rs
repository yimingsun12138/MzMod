//! Check native JSON before its original bytes enter the game archive.
use serde::{
    de::{value::MapAccessDeserializer, MapAccess, SeqAccess, Visitor},
    Deserialize, Deserializer,
};
use std::{collections::BTreeMap, fmt};

pub(super) fn validate(bytes: &[u8]) -> serde_json::Result<()> {
    let json = bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(bytes);
    serde_json::from_slice::<CheckedValue>(json).map(|_| ())
}

struct CheckedValue;

impl<'de> Deserialize<'de> for CheckedValue {
    fn deserialize<D: Deserializer<'de>>(input: D) -> Result<Self, D::Error> {
        input.deserialize_any(Self)
    }
}

impl<'de> Visitor<'de> for CheckedValue {
    type Value = Self;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a game JSON value with unique object fields")
    }

    fn visit_unit<E>(self) -> Result<Self, E> {
        Ok(self)
    }
    fn visit_bool<E>(self, _: bool) -> Result<Self, E> {
        Ok(self)
    }
    fn visit_str<E>(self, _: &str) -> Result<Self, E> {
        Ok(self)
    }
    fn visit_u64<E>(self, _: u64) -> Result<Self, E> {
        Ok(self)
    }
    fn visit_i64<E>(self, _: i64) -> Result<Self, E> {
        Ok(self)
    }
    fn visit_f64<E>(self, _: f64) -> Result<Self, E> {
        Ok(self)
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Self, A::Error> {
        while sequence.next_element::<Self>()?.is_some() {}
        Ok(self)
    }

    fn visit_map<A: MapAccess<'de>>(self, object: A) -> Result<Self, A::Error> {
        crate::unique_map::deserialize(MapAccessDeserializer::new(object))
            .map(|_: BTreeMap<String, Self>| self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_numbers_and_fields_remain_distinct_at_each_level() {
        validate(br#"[{"x":1e900,"id":18446744073709551616},{"x":-0,"unicode":"\u00e9"}]"#)
            .unwrap();
        let error = validate(br#"{"axes":[{"x":1,"\u0078":2}]}"#).unwrap_err();
        assert!(error.to_string().contains("duplicate key \"x\""));
        for invalid in [r#"{"x":NaN}"#, r#"{"x":-Infinity}"#, "{} {}", "{"] {
            assert!(validate(invalid.as_bytes()).is_err(), "{invalid}");
        }
    }
}
