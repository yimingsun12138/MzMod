//! Deserialize a map without silently replacing a repeated key.

use serde::{de, Deserialize, Deserializer};
use std::{collections::BTreeMap, fmt, marker::PhantomData};

pub(super) fn deserialize<'de, K, V, D>(input: D) -> Result<BTreeMap<K, V>, D::Error>
where
    K: Deserialize<'de> + Ord + fmt::Debug + fmt::Display,
    V: Deserialize<'de>,
    D: Deserializer<'de>,
{
    struct UniqueKeys<K, V>(PhantomData<(K, V)>);
    impl<'de, K, V> de::Visitor<'de> for UniqueKeys<K, V>
    where
        K: Deserialize<'de> + Ord + fmt::Debug + fmt::Display,
        V: Deserialize<'de>,
    {
        type Value = BTreeMap<K, V>;

        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("an object with unique keys")
        }

        fn visit_map<A: de::MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
            let mut values = BTreeMap::new();
            while let Some(key) = map.next_key::<K>()? {
                if values.contains_key(&key) {
                    return Err(de::Error::custom(format!("duplicate key {key:?}")));
                }
                let value = map
                    .next_value()
                    .map_err(|error| de::Error::custom(format!("{key}: {error}")))?;
                values.insert(key, value);
            }
            Ok(values)
        }
    }
    input.deserialize_map(UniqueKeys(PhantomData))
}
