//! Associate each controlled ground model with its item selection and its source.
use crate::item_catalog::{ground_model_path, ItemCatalog};
use serde_json::Value;
use std::fmt;

#[derive(Clone, Copy, Debug)]
pub(crate) enum BeamSelection {
    RuneSlot(usize),
    SmallCharm,
    LargeCharm,
    GrandCharm,
    Annihilus,
    HellfireTorch,
    Jewel,
}

impl fmt::Display for BeamSelection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RuneSlot(slot) => write!(formatter, "rune #{}", slot + 1),
            selection => write!(formatter, "{selection:?}"),
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) enum ModelSource<'a> {
    Native(&'a Value),
    BeamOnlyOverride,
    PrivateAppearance,
}

pub(crate) struct GroundModel<'a> {
    pub(crate) selection: BeamSelection,
    pub(crate) route: &'a str,
    pub(crate) source: ModelSource<'a>,
}

impl GroundModel<'_> {
    pub(crate) fn path(&self) -> String {
        ground_model_path(self.route)
    }
}

impl ItemCatalog {
    pub(crate) fn ground_models(&self) -> impl Iterator<Item = GroundModel<'_>> {
        use BeamSelection::*;
        use ModelSource::*;
        let runes = self
            .runes
            .iter()
            .enumerate()
            .map(|(slot, rune)| GroundModel {
                selection: RuneSlot(slot),
                route: &rune.route,
                source: rune
                    .native_ground_model
                    .as_ref()
                    .map_or(BeamOnlyOverride, Native),
            });
        let charms = &self.charms;
        let charms = [
            (SmallCharm, &charms.small.route, Native(&charms.small.model)),
            (LargeCharm, &charms.large.route, Native(&charms.large.model)),
            (GrandCharm, &charms.grand.route, Native(&charms.grand.model)),
            (Annihilus, &charms.annihilus_route, PrivateAppearance),
            (
                HellfireTorch,
                &charms.hellfire_torch_route,
                PrivateAppearance,
            ),
        ]
        .into_iter()
        .map(|(selection, route, source)| GroundModel {
            selection,
            route,
            source,
        });
        let jewels = self.jewels.models.iter().map(|(route, model)| GroundModel {
            selection: Jewel,
            route,
            source: Native(&model.native),
        });
        runes.chain(charms).chain(jewels)
    }
}
