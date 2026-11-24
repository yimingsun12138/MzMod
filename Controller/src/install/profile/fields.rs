//! Decode the fields recognized by a profile record and retain every other value verbatim.
use serde::{
    de::{self, DeserializeOwned, Visitor},
    Deserialize, Deserializer,
};
use serde_json::value::RawValue;
use std::collections::BTreeMap;

pub(super) type RawMembers = BTreeMap<String, Box<RawValue>>;

#[derive(Deserialize)]
#[serde(transparent)]
pub(super) struct RecordFields {
    #[serde(deserialize_with = "crate::unique_map::deserialize")]
    pub(super) members: RawMembers,
}

impl RecordFields {
    pub(super) fn decode<T: DeserializeOwned>(mut self) -> serde_json::Result<(T, RawMembers)> {
        let mut names = &[][..];
        let record = T::deserialize(KnownFields {
            members: &self.members,
            names: &mut names,
        })?;
        for name in names {
            self.members.remove(*name);
        }
        Ok((record, self.members))
    }
}

// Serde supplies a named struct's wire field names, including any aliases.
struct KnownFields<'de, 'names> {
    members: &'de RawMembers,
    names: &'names mut &'static [&'static str],
}

impl<'de> Deserializer<'de> for KnownFields<'de, '_> {
    type Error = serde_json::Error;

    fn deserialize_struct<V: Visitor<'de>>(
        self,
        _: &'static str,
        names: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        *self.names = names;
        let fields = names
            .iter()
            .filter_map(|&name| self.members.get(name).map(|raw| (name, raw.as_ref())));
        visitor.visit_map(de::value::MapDeserializer::new(fields))
    }

    fn deserialize_any<V: Visitor<'de>>(self, _: V) -> Result<V::Value, Self::Error> {
        Err(de::Error::custom("expected a named profile record"))
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string bytes byte_buf
        option unit unit_struct newtype_struct seq tuple tuple_struct map enum identifier ignored_any
    }
}
