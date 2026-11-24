//! Render player choices against one regional template. Installation is a separate disk operation.
use anyhow::Result;

#[cfg(test)]
#[path = "../build_support/base_suffixes.rs"]
mod base_suffixes;
#[cfg(test)]
#[path = "../build_support/charm_routes.rs"]
mod charm_routes;
#[cfg(test)]
mod chinese_overlay;
#[cfg(test)]
#[path = "../build_support/cube_layout.rs"]
mod cube_layout;
mod file_fingerprints;
#[cfg(test)]
#[path = "../build_support/game_json.rs"]
mod game_json;
mod game_path;
mod ground_beam;
mod ground_model;
pub mod install;
mod item_catalog;
mod item_colors;
mod items;
mod jewel_routes;
mod mod_info;
#[cfg(test)]
#[path = "../build_support/native_missiles.rs"]
mod native_missiles;
#[cfg(test)]
#[path = "../build_support/output_paths.rs"]
mod output_paths;
#[cfg(windows)]
mod preview_items;
mod region;
mod resources;
mod rune_catalog;
mod settings;
mod template;
#[cfg(windows)]
pub mod ui;
mod unique_map;
mod visuals;

pub use region::Region;
pub use resources::ModFiles;
pub use settings::Settings;

pub use resources::ModFiles as Files;

#[cfg(test)]
use settings::enabled_features;

/// Generate the complete MzMod.mpq payload in memory for one region.
pub fn render_mod(region: Region, settings: &Settings) -> Result<ModFiles> {
    template::RegionalTemplate::read(region)?.render(settings)
}
