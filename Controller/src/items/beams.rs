//! Apply each selected drop beam to the game resource owned by that item.
use super::jewels;
use crate::ground_beam::GroundBeam;
use crate::ground_model::{BeamSelection, GroundModel, ModelSource};
use crate::item_catalog::ItemCatalog;
use crate::resources::{read_document, write_document};
use crate::{ModFiles, Region, Settings};
use anyhow::{Context, Result};

pub(super) fn render(
    files: &mut ModFiles,
    catalog: &ItemCatalog,
    settings: &Settings,
    region: Region,
) -> Result<()> {
    let beam = GroundBeam::from_entity(&catalog.beam)?;
    let jewel_routes = jewels::install_routes_and_icons(files, &catalog.jewels, settings, region)?;
    for model in catalog.ground_models() {
        let enabled = match model.selection {
            BeamSelection::RuneSlot(slot) => settings.rune_beams[slot],
            BeamSelection::SmallCharm => settings.small_charm_beam,
            BeamSelection::LargeCharm => settings.large_charm_beam,
            BeamSelection::GrandCharm => settings.grand_charm_beam,
            BeamSelection::Annihilus => settings.annihilus_beam,
            BeamSelection::HellfireTorch => settings.torch_beam,
            BeamSelection::Jewel => jewel_routes.contains(model.route),
        };
        apply_beam(files, &model, &beam, enabled)?;
    }
    Ok(())
}

fn apply_beam(
    files: &mut ModFiles,
    model: &GroundModel<'_>,
    beam: &GroundBeam<'_>,
    enabled: bool,
) -> Result<()> {
    match (model.source, enabled) {
        (ModelSource::Native(native), true) => {
            let path = model.path();
            let mut document = native.clone();
            beam.attach(&mut document).with_context(|| path.clone())?;
            write_document(files, &path, &document)
        }
        (ModelSource::PrivateAppearance, false) => {
            let path = model.path();
            let mut document = read_document(files, &path)?;
            beam.detach(&mut document).with_context(|| path.clone())?;
            write_document(files, &path, &document)
        }
        (ModelSource::BeamOnlyOverride, false) => {
            files.remove(&model.path());
            Ok(())
        }
        (ModelSource::Native(_), false)
        | (ModelSource::BeamOnlyOverride | ModelSource::PrivateAppearance, true) => Ok(()),
    }
}
