//! Compile the selected inventory sprites into individually addressable PNGs.
use std::{
    collections::BTreeMap,
    fs, io,
    ops::Range,
    path::{Path, PathBuf},
};

use crate::archive::RegionalResources;
use crate::item_catalog::{inventory_icon_path, inventory_sprite_name};
use crate::preview_items::RunePreviewIcons;
use crate::region::Region;

const NATIVE_ICON_ROOT: &str = "assets/ui/native-rune-icons";

pub(super) struct RuneIconCompiler<'a> {
    template: &'a RegionalResources,
    native: RegionalResources,
    source_ranges: BTreeMap<PathBuf, Range<usize>>,
    png_bytes: Vec<u8>,
}

impl<'a> RuneIconCompiler<'a> {
    pub(super) fn new(template: &'a RegionalResources) -> io::Result<Self> {
        println!("cargo:rerun-if-changed={NATIVE_ICON_ROOT}");
        Ok(Self {
            template,
            native: RegionalResources::read(Path::new(NATIVE_ICON_ROOT))?,
            source_ranges: BTreeMap::new(),
            png_bytes: Vec::new(),
        })
    }

    pub(super) fn compile(&mut self, region: Region, route: &str) -> io::Result<RunePreviewIcons> {
        let sprite = inventory_sprite_name(route);
        let native_path = self.native.find(region, &sprite).ok_or_else(|| {
            io::Error::other(format!(
                "missing native inventory sprite {sprite} in {NATIVE_ICON_ROOT}"
            ))
        })?;
        let numbered_path = self
            .template
            .resolve(region, &inventory_icon_path(&sprite))?;
        Ok(RunePreviewIcons {
            native: self.encode_source(native_path)?,
            numbered: self.encode_source(numbered_path)?,
        })
    }

    fn encode_source(&mut self, source: PathBuf) -> io::Result<Range<usize>> {
        if let Some(range) = self.source_ranges.get(&source) {
            return Ok(range.clone());
        }
        let start = self.png_bytes.len();
        fs::read(&source)
            .and_then(|bytes| encode_inventory_sprite(&bytes, &mut self.png_bytes))
            .map_err(|error| io::Error::other(format!("{}: {error}", source.display())))?;
        let range = start..self.png_bytes.len();
        self.source_ranges.insert(source, range.clone());
        Ok(range)
    }

    pub(super) fn write(&self, output: &Path) -> io::Result<()> {
        fs::write(output.join("rune-preview-icons.bin"), &self.png_bytes)
    }
}

fn encode_inventory_sprite(sprite: &[u8], output: &mut Vec<u8>) -> io::Result<()> {
    let (header, pixels) = sprite
        .split_at_checked(40)
        .ok_or_else(|| io::Error::other("truncated inventory sprite header"))?;
    let word = |offset| u32::from_le_bytes(header[offset..offset + 4].try_into().unwrap());
    if !header[..4].eq_ignore_ascii_case(b"SpA1")
        || header[4..6] != 31u16.to_le_bytes()
        || word(20) != 1
        || word(36) != 4
    {
        return Err(io::Error::other(
            "expected a single-frame RGBA inventory sprite",
        ));
    }
    let (width, height) = (word(8), word(12));
    // egui's portable texture limit is 2048 pixels per side.
    if !(1..=2048).contains(&width) || !(1..=2048).contains(&height) {
        return Err(io::Error::other(format!(
            "unsupported preview dimensions {width}x{height}"
        )));
    }
    let bytes = width * height * 4;
    if word(32) != bytes || pixels.len() != bytes as usize {
        return Err(io::Error::other(
            "inventory sprite RGBA payload length mismatch",
        ));
    }
    let mut encoder = png::Encoder::new(output, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(io::Error::other)?;
    writer.write_image_data(pixels).map_err(io::Error::other)?;
    writer.finish().map_err(io::Error::other)
}
