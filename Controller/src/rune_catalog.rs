//! Place template rune records in the numbered slots used by saved player choices.
use serde::{de::Error, Deserialize, Deserializer};
use serde_json::Value;

pub(crate) const RUNE_COUNT: usize = 33;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Rune {
    number: usize,
    pub route: String,
    pub english: String,
    // The Windows selection list displays both names.
    #[cfg_attr(not(windows), allow(dead_code))]
    pub chinese: String,
    #[serde(rename = "ground_model")]
    pub native_ground_model: Option<Value>,
}

pub(crate) fn by_number<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<[Rune; RUNE_COUNT], D::Error> {
    let mut slots = std::array::from_fn(|_| None);
    for rune in Vec::<Rune>::deserialize(deserializer)? {
        let number = rune.number;
        let slot = number
            .checked_sub(1)
            .and_then(|index| slots.get_mut(index))
            .ok_or_else(|| {
                D::Error::custom(format!("rune number {number} outside 1..={RUNE_COUNT}"))
            })?;
        if slot.replace(rune).is_some() {
            return Err(D::Error::custom(format!("duplicate rune number {number}")));
        }
    }
    if let Some(index) = slots.iter().position(Option::is_none) {
        return Err(D::Error::custom(format!(
            "missing rune number {}",
            index + 1
        )));
    }
    Ok(slots.map(|record| record.expect("all rune slots were checked")))
}
