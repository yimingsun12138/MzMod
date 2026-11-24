//! Tooltip examples rendered from the same item records as the installation.
use eframe::egui::{
    self, pos2, text::LayoutJob, vec2, Align2, Color32, FontId, Painter, Rect, Sense, Stroke,
    TextFormat, TextureHandle, Ui,
};

use super::theme::{self, GOLD, LINE, MUTED, OPTION_TITLE_SIZE};
use crate::preview_items::{PreviewLabel, PreviewLanguage, RegionalPreview};
use crate::{items::preview_label, Region, Settings};
use std::{collections::BTreeMap, ops::Range, sync::OnceLock};

const TEXT_SCALE: f32 = OPTION_TITLE_SIZE / 22.;
const CELL_PADDING: f32 = 14.;
const HEADING_GAP: f32 = 5.;
const CAPTION_HEIGHT: f32 = 18. * TEXT_SCALE;
const LABEL_LINE_HEIGHT: f32 = 24. * TEXT_SCALE;
const RUNE_ICON_SIZE: f32 = 36.;

fn preview(region: Region) -> &'static RegionalPreview {
    static EXAMPLES: OnceLock<BTreeMap<String, RegionalPreview>> = OnceLock::new();
    &EXAMPLES.get_or_init(|| {
        serde_json::from_slice(include_bytes!(concat!(
            env!("OUT_DIR"),
            "/item-previews.json"
        )))
        .expect("compiled item previews")
    })[region.code()]
}

impl PreviewLabel {
    fn text(&self, language: PreviewLanguage, settings: &Settings) -> String {
        preview_label(&self.bank, &self.row, language.locale(), settings)
    }
}

fn group_height(heading_height: f32, label_lines: usize) -> f32 {
    heading_height + HEADING_GAP + LABEL_LINE_HEIGHT * label_lines as f32
}

fn language_previews(ui: &mut Ui, mut draw: impl FnMut(&mut Ui, PreviewLanguage)) {
    for language in PreviewLanguage::ALL {
        let title = match language {
            PreviewLanguage::TraditionalChinese => "中文界面预览",
            PreviewLanguage::English => "英文界面预览",
        };
        theme::panel(14).fill(Color32::from_gray(8)).show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            let (heading, _) = ui.allocate_exact_size(
                vec2(ui.available_width(), LABEL_LINE_HEIGHT),
                Sense::hover(),
            );
            ui.painter().text(
                heading.center(),
                Align2::CENTER_CENTER,
                title,
                FontId::proportional(17. * TEXT_SCALE),
                GOLD,
            );
            ui.add_space(3.);
            draw(ui, language);
        });
    }
}

fn columns(ui: &mut Ui, height: f32) -> [Rect; 2] {
    let (row, _) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
    let x = row.center().x;
    ui.painter().line_segment(
        [pos2(x, row.top()), pos2(x, row.bottom())],
        Stroke::new(1_f32, LINE),
    );
    let (left, right) = row.split_left_right_at_x(x);
    [left, right]
}

fn game_color([r, g, b, a]: [u8; 4]) -> Color32 {
    Color32::from_rgba_unmultiplied(r, g, b, a)
}

fn tooltip_line(line: &str, base: Color32, inline: &BTreeMap<char, [u8; 4]>) -> LayoutJob {
    let mut job = LayoutJob::default();
    let mut format = TextFormat::simple(FontId::proportional(OPTION_TITLE_SIZE), base);
    let mut start = 0;
    for (offset, marker) in line.match_indices("ÿc") {
        let code_start = offset + marker.len();
        let Some(code) = line[code_start..].chars().next() else {
            break;
        };
        let Some(&color) = inline.get(&code) else {
            continue;
        };
        if start < offset {
            job.append(&line[start..offset], 0., format.clone());
        }
        format.color = game_color(color);
        start = code_start + code.len_utf8();
    }
    if start < line.len() {
        job.append(&line[start..], 0., format);
    }
    if job.text.starts_with('[') {
        for section in &mut job.sections {
            section.format.font_id.size = 19. * TEXT_SCALE;
        }
    }
    job
}

fn tooltip(
    painter: &Painter,
    cell: Rect,
    top: f32,
    label: &str,
    base: [u8; 4],
    inline: &BTreeMap<char, [u8; 4]>,
) {
    let painter = painter.with_clip_rect(cell.intersect(painter.clip_rect()));
    let base = game_color(base);
    // TooltipStyle displays the last stored line first and centers each line independently.
    for (index, line) in label.lines().rev().enumerate() {
        let mut galley = painter.layout_job(tooltip_line(line, base, inline));
        let width = (cell.width() - 2. * CELL_PADDING).max(1.);
        if galley.size().x > width {
            let scale = width / galley.size().x;
            let mut job = (*galley.job).clone();
            for section in &mut job.sections {
                section.format.font_id.size *= scale;
            }
            galley = painter.layout_job(job);
        }
        let center = pos2(
            cell.center().x,
            top + LABEL_LINE_HEIGHT * (index as f32 + 0.5),
        );
        painter.galley(center - galley.size() * 0.5, galley, base);
    }
}

fn item_example(
    painter: &Painter,
    cell: Rect,
    caption: &str,
    label: &str,
    base: [u8; 4],
    inline: &BTreeMap<char, [u8; 4]>,
    line_slots: usize,
) {
    // Reserve this item's optional lines so toggling a hint keeps the heading and name anchored.
    let top = cell.center().y - group_height(CAPTION_HEIGHT, line_slots) * 0.5;
    painter.text(
        pos2(cell.center().x, top + CAPTION_HEIGHT * 0.5),
        Align2::CENTER_CENTER,
        caption,
        FontId::proportional(15. * TEXT_SCALE),
        MUTED,
    );
    tooltip(
        painter,
        cell,
        top + CAPTION_HEIGHT + HEADING_GAP,
        label,
        base,
        inline,
    );
}

pub fn names(ui: &mut Ui, region: Region, settings: &Settings) {
    let regional = preview(region);
    let examples = &regional.items;
    let tooltip_colors = &regional.tooltip_colors;
    let colors = if settings.item_colors {
        &regional.enhanced_colors
    } else {
        &regional.native_colors
    };
    language_previews(ui, |ui, language| {
        let (captions, spirit_lines) = match language {
            PreviewLanguage::TraditionalChinese => {
                (["符文之语", "精英轻盾", "无形无孔", "有孔"], 3)
            }
            PreviewLanguage::English => (
                [
                    "Runeword",
                    "Elite light shield",
                    "Ethereal, no sockets",
                    "Socketed",
                ],
                2,
            ),
        };
        let [runeword, shield_kind, ethereal, socketed] = captions;
        let row_height = group_height(CAPTION_HEIGHT, 3) + 2. * CELL_PADDING;
        let [left, right] = columns(ui, row_height * 2.);
        let (spirit, ethereal_sword) = left.split_top_bottom_at_fraction(0.5);
        let (shield, socketed_sword) = right.split_top_bottom_at_fraction(0.5);
        ui.painter().line_segment(
            [spirit.left_bottom(), shield.right_bottom()],
            Stroke::new(1_f32, LINE),
        );
        item_example(
            ui.painter(),
            spirit,
            runeword,
            &examples.runeword.text(language, settings),
            tooltip_colors.runeword,
            &tooltip_colors.inline,
            spirit_lines,
        );
        item_example(
            ui.painter(),
            shield,
            shield_kind,
            &examples.shield.text(language, settings),
            tooltip_colors.default_item,
            &tooltip_colors.inline,
            1,
        );
        let sword = examples.sword.text(language, settings);
        for (cell, (kind, color)) in [ethereal_sword, socketed_sword]
            .into_iter()
            .zip([(ethereal, colors.ethereal), (socketed, colors.socketed)])
        {
            item_example(
                ui.painter(),
                cell,
                kind,
                &sword,
                color,
                &tooltip_colors.inline,
                1,
            );
        }
    });
}

const RUNE_ICON_BYTES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/rune-preview-icons.bin"));

fn rune_texture(ui: &Ui, range: &Range<usize>) -> TextureHandle {
    let id = egui::Id::new(("rune-preview-icon", range.start));
    if let Some(texture) = ui.ctx().data(|data| data.get_temp::<TextureHandle>(id)) {
        return texture;
    }
    let icon = eframe::icon_data::from_png_bytes(&RUNE_ICON_BYTES[range.clone()])
        .expect("Cargo encodes the selected inventory sprites as PNG");
    let texture = ui.ctx().load_texture(
        "rune-preview-icon",
        egui::ColorImage::from_rgba_unmultiplied(
            [icon.width as usize, icon.height as usize],
            &icon.rgba,
        ),
        egui::TextureOptions::LINEAR,
    );
    ui.ctx()
        .data_mut(|data| data.insert_temp(id, texture.clone()));
    texture
}

pub fn runes(ui: &mut Ui, region: Region, settings: &Settings) {
    let regional = preview(region);
    language_previews(ui, |ui, language| {
        // Reserve the material line in both cells to keep the icons and names aligned.
        let height = group_height(RUNE_ICON_SIZE, 2) + 2. * CELL_PADDING;
        for (cell, rune) in columns(ui, height).into_iter().zip(&regional.items.runes) {
            let texture = rune_texture(
                ui,
                if settings.rune_icons {
                    &rune.icons.numbered
                } else {
                    &rune.icons.native
                },
            );
            let size = texture.size_vec2();
            let icon = Rect::from_center_size(
                pos2(
                    cell.center().x,
                    cell.top() + CELL_PADDING + RUNE_ICON_SIZE / 2.,
                ),
                size * (RUNE_ICON_SIZE / size.max_elem()),
            );
            ui.painter().image(
                texture.id(),
                icon,
                Rect::from_min_max(pos2(0., 0.), pos2(1., 1.)),
                Color32::WHITE,
            );
            let label = rune.label.text(language, settings);
            tooltip(
                ui.painter(),
                cell,
                cell.top() + CELL_PADDING + RUNE_ICON_SIZE + HEADING_GAP,
                &label,
                regional.tooltip_colors.rune,
                &regional.tooltip_colors.inline,
            );
        }
    });
}

#[cfg(test)]
mod tests;
