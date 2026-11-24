//! The examples displayed in the two preview panels, in display order.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::BTreeMap, ops::Range};

use crate::item_colors::EquipmentColors;

#[derive(Clone, Copy)]
pub(crate) enum PreviewLanguage {
    TraditionalChinese,
    English,
}

impl PreviewLanguage {
    pub(crate) const ALL: [Self; 2] = [Self::TraditionalChinese, Self::English];

    pub(crate) fn locale(self) -> &'static str {
        match self {
            Self::TraditionalChinese => "zhTW",
            Self::English => "enUS",
        }
    }
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PreviewItems<T, R = T> {
    pub runeword: T,
    pub shield: T,
    pub sword: T,
    pub runes: [R; 2],
}

#[derive(Deserialize, Serialize)]
pub(crate) struct PreviewLabel {
    pub bank: String,
    pub row: Value,
}

/// Byte ranges of the two PNGs in the compiled preview image resource.
#[derive(Deserialize, Serialize)]
pub(crate) struct RunePreviewIcons {
    pub native: Range<usize>,
    pub numbered: Range<usize>,
}

#[derive(Deserialize, Serialize)]
pub(crate) struct RunePreview {
    pub label: PreviewLabel,
    pub icons: RunePreviewIcons,
}

#[derive(Deserialize, Serialize)]
pub(crate) struct TooltipColors {
    pub default_item: [u8; 4],
    pub runeword: [u8; 4],
    pub rune: [u8; 4],
    pub inline: BTreeMap<char, [u8; 4]>,
}

#[derive(Deserialize, Serialize)]
pub(crate) struct RegionalPreview {
    pub items: PreviewItems<PreviewLabel, RunePreview>,
    pub tooltip_colors: TooltipColors,
    pub enhanced_colors: EquipmentColors<[u8; 4]>,
    pub native_colors: EquipmentColors<[u8; 4]>,
}
