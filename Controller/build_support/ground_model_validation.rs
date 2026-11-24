//! Verify each model's enabled and disabled states before the regional templates are embedded.
use crate::archive::RegionalResources;
use crate::game_path::is_valid_game_path;
use crate::ground_beam::GroundBeam;
use crate::ground_model::ModelSource;
use crate::item_catalog::ItemCatalog;
use crate::output_paths::{FileSource, OutputPaths};
use crate::region::Region;
use std::{fs, io};

pub(super) fn validate_ground_models(
    catalog: &ItemCatalog,
    resources: &RegionalResources,
    paths: &mut OutputPaths,
) -> io::Result<()> {
    let beam = GroundBeam::from_entity(&catalog.beam)
        .map_err(|error| io::Error::other(format!("items.json beam: {error}")))?;
    for model in catalog.ground_models() {
        let selection = model.selection;
        if model.route.contains('\\') || !is_valid_game_path(model.route) {
            return Err(io::Error::other(format!(
                "items.json: invalid {selection} ground model route {:?}",
                model.route
            )));
        }
        let path = model.path();
        let owner = format!("{selection} drop beam");
        match model.source {
            ModelSource::Native(native) => {
                for region in Region::ALL {
                    paths.claim(region, &path, FileSource::Generated, &owner)?;
                }
                beam.attach(&mut native.clone()).map_err(|error| {
                    io::Error::other(format!("items.json: {selection} {path}: {error}"))
                })?;
            }
            ModelSource::BeamOnlyOverride | ModelSource::PrivateAppearance => {
                for region in Region::ALL {
                    paths.claim(region, &path, FileSource::Template, &owner)?;
                    let context = |error| {
                        io::Error::other(format!("{}: {selection} {path}: {error}", region.code()))
                    };
                    let source = resources.resolve(region, &path)?;
                    let mut document = serde_json::from_reader(fs::File::open(source)?)
                        .map_err(io::Error::from)
                        .map_err(context)?;
                    beam.detach(&mut document).map_err(context)?;
                }
            }
        }
    }
    Ok(())
}
