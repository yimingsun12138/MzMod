//! Player choices and their persisted wire format. Defaults are compiled from the template.
use serde::{Deserialize, Serialize};

use crate::rune_catalog::RUNE_COUNT;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    #[serde(with = "rune_selection")]
    pub rune_beams: [bool; RUNE_COUNT],
    pub annihilus_beam: bool,
    pub torch_beam: bool,
    pub small_charm_beam: bool,
    pub large_charm_beam: bool,
    pub grand_charm_beam: bool,
    pub jewel_beam: bool,
    pub unique_jewel_beam: bool,
    pub english_names: bool,
    pub max_hints: bool,
    pub base_tiers: bool,
    pub armor_weights: bool,
    pub item_colors: bool,
    pub rune_numbers: bool,
    pub rune_icons: bool,
    pub rune_recipes: bool,
    pub sorceress_eye_care: bool,
    pub assassin_eye_care: bool,
    pub waypoints: bool,
    pub entrances: bool,
    pub tomb_symbols: bool,
    pub hall_directions: bool,
    pub horazon_journal: bool,
    pub quest_books: bool,
    pub mini_cube: bool,
    pub auto_open_cube: bool,
    pub custom_font: bool,
    pub hud_style: bool,
    pub health_bars: bool,
}

impl Default for Settings {
    fn default() -> Self {
        include!(concat!(env!("OUT_DIR"), "/default_settings.rs"))
    }
}

struct ConditionalSetting {
    available: fn(&Settings) -> bool,
    value_mut: fn(&mut Settings) -> &mut bool,
}

const CONDITIONAL_SETTINGS: &[(&str, ConditionalSetting)] =
    include!(concat!(env!("OUT_DIR"), "/setting_prerequisites.rs"));

impl Settings {
    #[cfg(any(windows, test))]
    pub(crate) fn prerequisites_met(&self, field: &str) -> bool {
        CONDITIONAL_SETTINGS
            .iter()
            .find(|(name, _)| *name == field)
            .is_none_or(|(_, rule)| (rule.available)(self))
    }

    /// Suspend unavailable choices for rendering while preserving the saved profile.
    pub(crate) fn effective(&self) -> Self {
        let mut effective = self.clone();
        for (_, rule) in CONDITIONAL_SETTINGS {
            if !(rule.available)(self) {
                *(rule.value_mut)(&mut effective) = false;
            }
        }
        effective
    }

    /// Apply the player's edits to the values currently held by this destination.
    pub(crate) fn merge_edits(&mut self, baseline: &Self, edited: &Self) {
        *self = include!(concat!(env!("OUT_DIR"), "/merged_settings.rs"));
    }
}

fn merge_toggle(current: bool, baseline: bool, edited: bool) -> bool {
    if edited != baseline {
        edited
    } else {
        current
    }
}

fn merge_selection<const N: usize>(
    current: [bool; N],
    baseline: [bool; N],
    edited: [bool; N],
) -> [bool; N] {
    std::array::from_fn(|index| merge_toggle(current[index], baseline[index], edited[index]))
}

#[cfg(test)]
pub(crate) fn enabled_features() -> Settings {
    // Exercise individual toggles independently of the release's player defaults.
    let mut fields = serde_json::to_value(Settings::default()).unwrap();
    for value in fields.as_object_mut().unwrap().values_mut() {
        if value.is_boolean() {
            *value = true.into();
        }
    }
    serde_json::from_value(fields).unwrap()
}

mod rune_selection {
    use super::RUNE_COUNT;
    use serde::{de::Error, Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S: Serializer>(
        values: &[bool; RUNE_COUNT],
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        values.as_slice().serialize(serializer)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<[bool; RUNE_COUNT], D::Error> {
        Vec::<bool>::deserialize(deserializer)?
            .try_into()
            .map_err(|_| D::Error::custom(format!("expected exactly {RUNE_COUNT} rune choices")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_saved_choice_can_be_merged_in_both_directions() {
        let defaults = Settings::default();
        let mut fields = serde_json::to_value(&defaults).unwrap();
        for value in fields.as_object_mut().unwrap().values_mut() {
            let choices = match value {
                serde_json::Value::Array(values) => values.as_mut_slice(),
                scalar => std::slice::from_mut(scalar),
            };
            for choice in choices {
                *choice = (!choice.as_bool().unwrap()).into();
            }
        }
        let edited: Settings = serde_json::from_value(fields).unwrap();
        let mut current = defaults.clone();
        current.merge_edits(&defaults, &edited);
        assert_eq!(current, edited);
        current.merge_edits(&edited, &defaults);
        assert_eq!(current, defaults);
    }

    #[test]
    fn an_edit_keeps_independent_changes_in_the_destination() {
        let baseline = Settings::default();
        let mut edited = baseline.clone();
        edited.rune_beams[29] = !baseline.rune_beams[29];
        edited.mini_cube = !baseline.mini_cube;

        let mut current = baseline.clone();
        current.rune_beams[8] = !baseline.rune_beams[8];
        current.rune_icons = !baseline.rune_icons;
        let mut expected = current.clone();
        expected.rune_beams[29] = edited.rune_beams[29];
        expected.mini_cube = edited.mini_cube;

        current.merge_edits(&baseline, &edited);
        assert_eq!(current, expected);
    }
}
