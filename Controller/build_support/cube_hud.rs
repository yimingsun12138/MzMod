//! Compile each region's gameplay HUDs and assign the resulting archive paths.
use crate::archive::RegionalResources;
use crate::cube_layout::{controls, CubeHud};
use crate::region::{Region, COMMON_GROUP};
use std::{
    collections::BTreeMap,
    fs, io,
    path::{Path, PathBuf},
};

fn layout_path(panel: &str, controller: bool) -> String {
    let input = if controller { "controller/" } else { "" };
    format!(
        "data/global/ui/layouts/{input}{}hd.json",
        panel.to_ascii_lowercase()
    )
}

fn read_layout<T>(
    resources: &RegionalResources,
    region: Region,
    path: &str,
    read: impl FnOnce(&[u8]) -> io::Result<T>,
) -> io::Result<T> {
    let source = resources.resolve(region, path)?;
    fs::read(source)
        .and_then(|bytes| read(&bytes))
        .map_err(|error| io::Error::other(format!("{}: {path}: {error}", region.code())))
}

pub(super) fn compile(
    template: &Path,
    resources: RegionalResources,
    output: &Path,
) -> io::Result<RegionalResources> {
    let declaration = template.join("cube-hud.json");
    let panels = fs::read(&declaration)
        .and_then(|bytes| CubeHud::from_slice(&bytes))
        .map_err(|error| io::Error::other(format!("{}: {error}", declaration.display())))?;
    let startup = layout_path(&panels.startup_panel, false);
    for region in Region::ALL {
        read_layout(&resources, region, &startup, |bytes| {
            panels.check_startup(bytes)
        })?;
        read_layout(
            &resources,
            region,
            &layout_path("PlayerInventoryExpansionLayout", false),
            controls::check_inventory,
        )?;
        read_layout(
            &resources,
            region,
            &layout_path("HoradricCubeMiniLayout", false),
            controls::check_panel,
        )?;
    }
    let mut generated = BTreeMap::new();
    for controller in [false, true] {
        let entry = layout_path(&panels.entry_panel, controller);
        let gameplay = layout_path(&panels.gameplay_panel, controller);
        let mut variants = Vec::new();
        for region in Region::ALL {
            if resources.find(region, &gameplay).is_some() {
                return Err(io::Error::other(format!(
                    "{}: generated HUD conflicts with template {gameplay}",
                    region.code()
                )));
            }
            let bytes = read_layout(&resources, region, &entry, |bytes| {
                panels.render_gameplay(bytes, controller)
            })?;
            variants.push((region.code(), bytes));
        }
        if variants.iter().all(|(_, bytes)| bytes == &variants[0].1) {
            variants.truncate(1);
            variants[0].0 = COMMON_GROUP;
        }
        for (group, bytes) in variants {
            let member = PathBuf::from(group).join(&gameplay);
            let source = output.join("cube-hud").join(&member);
            fs::create_dir_all(source.parent().expect("HUD layout directory"))?;
            fs::write(&source, bytes)?;
            generated.insert(member, source);
        }
    }
    resources.with_generated(generated)
}
