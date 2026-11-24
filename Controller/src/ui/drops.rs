//! Drop beam selection, including the search terms and selected rune range.
use eframe::egui::{self, RichText, Ui};

use super::options::{DropGroup, Toggle};
use super::theme::{self, GOLD, INK};
use crate::{items::rune_names, rune_catalog::RUNE_COUNT, Settings};

pub(super) struct DropPage {
    search: String,
    rune_range: usize,
}

impl Default for DropPage {
    fn default() -> Self {
        Self {
            search: String::new(),
            rune_range: 2,
        }
    }
}

impl DropPage {
    pub(super) fn show(&mut self, ui: &mut Ui, settings: &mut Settings, groups: &[DropGroup]) {
        drop_search(ui, &mut self.search);
        ui.add_space(7.0);
        let query = self.search.trim().to_lowercase();
        let searching = !query.is_empty();
        let runes: Vec<_> = rune_names()
            .enumerate()
            .filter(|(index, (en, zh))| {
                if searching {
                    query.trim_start_matches('#') == (index + 1).to_string()
                        || en.to_lowercase().contains(&query)
                        || zh.contains(&query)
                        || RUNE_SEARCH_ALIASES[*index].contains(&query)
                } else {
                    index / 11 == self.rune_range
                }
            })
            .collect();
        let mut has_results = !runes.is_empty();
        if has_results {
            ui.horizontal(|ui| {
                ui.label(RichText::new("符文").color(GOLD).size(19.0));
                if !searching {
                    ui.add_space(12.0);
                    for (index, title) in ["1–11", "12–22", "23–33"].iter().enumerate() {
                        ui.selectable_value(&mut self.rune_range, index, *title);
                    }
                }
            });
            drop_grid(ui, &runes, |ui, &(index, (_, zh))| {
                theme::drop_choice(
                    ui,
                    &mut settings.rune_beams[index],
                    &format!("#{}  {zh}", index + 1),
                );
            });
        }
        for group in groups {
            let show_group = group.title.contains(&query) || group.aliases.contains(&query);
            let matches = |option: &Toggle| {
                show_group
                    || option.label.contains(&query)
                    || option.search_aliases.contains(&query)
            };
            let mut rows = group
                .rows
                .iter()
                .copied()
                .filter(|row| row.iter().any(&matches))
                .peekable();
            if rows.peek().is_none() {
                continue;
            }
            if has_results {
                ui.add_space(6.0);
                ui.separator();
            }
            ui.label(RichText::new(group.title).color(GOLD).size(19.0));
            for row in rows {
                ui.columns(row.len(), |cells| {
                    for (cell, option) in cells.iter_mut().zip(row) {
                        if matches(option) {
                            option.show_control(cell, settings, |ui, value| {
                                theme::drop_choice(ui, value, option.label);
                            });
                        }
                    }
                });
            }
            has_results = true;
        }
        if !has_results {
            ui.label(RichText::new("没有匹配的物品").color(INK));
        }
    }
}

fn drop_search(ui: &mut Ui, search: &mut String) {
    let field = egui::Frame::new()
        .fill(theme::CANVAS)
        .stroke(egui::Stroke::new(1_f32, theme::LINE))
        .corner_radius(3.);
    field.show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 0.;
            let width = ui.available_width();
            ui.set_min_width(width);
            let button_width = 34.;
            let margin = egui::Margin::symmetric(10, 7);
            let editor = ui.add(
                egui::TextEdit::singleline(search)
                    .frame(false)
                    .hint_text("搜索物品名称或符文编号")
                    .desired_width(width - button_width - margin.sum().x)
                    .margin(margin),
            );
            if !search.is_empty() {
                let (rect, clear) = ui.allocate_exact_size(
                    egui::vec2(button_width, editor.rect.height()),
                    egui::Sense::click(),
                );
                let center = rect.center();
                let color = if clear.hovered() || clear.has_focus() {
                    GOLD
                } else {
                    theme::MUTED
                };
                let painter = ui.painter();
                painter.circle_stroke(center, 8.5, egui::Stroke::new(1.2_f32, color));
                for diagonal in [egui::vec2(3.3, 3.3), egui::vec2(3.3, -3.3)] {
                    painter.line_segment(
                        [center - diagonal, center + diagonal],
                        egui::Stroke::new(1.4_f32, color),
                    );
                }
                clear.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), "清空搜索")
                });
                if clear
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .on_hover_text("清空搜索")
                    .clicked()
                {
                    search.clear();
                    editor.request_focus();
                }
            }
        });
    });
}

fn drop_grid<T>(ui: &mut Ui, options: &[T], mut draw: impl FnMut(&mut Ui, &T)) {
    let columns = ((ui.available_width() / 180.).floor() as usize).clamp(1, 3);
    for row in options.chunks(columns) {
        ui.columns(columns, |cells| {
            for (cell, option) in cells.iter_mut().zip(row) {
                draw(cell, option);
            }
        });
    }
}

const RUNE_SEARCH_ALIASES: [&str; RUNE_COUNT] = [
    "艾尔",
    "艾德",
    "特尔 提尔",
    "那夫 奈夫",
    "爱斯 艾斯",
    "伊司",
    "塔尔",
    "拉尔",
    "欧特",
    "书尔 图尔",
    "安姆",
    "索尔",
    "夏 沙伊",
    "多尔",
    "海尔",
    "埃欧 艾欧",
    "卢姆",
    "科",
    "法尔",
    "蓝姆 兰姆",
    "普尔",
    "乌姆",
    "马尔 玛尔",
    "伊司特",
    "古尔",
    "伐克斯",
    "欧姆",
    "罗",
    "瑟",
    "贝",
    "乔 扎哈",
    "查姆",
    "萨德 佐德",
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::options::DROP_GROUPS;

    fn drop_labels(
        width: f32,
        query: &str,
        range: usize,
    ) -> std::collections::HashMap<String, (egui::Pos2, usize)> {
        let ctx = egui::Context::default();
        theme::install(&ctx);
        let output = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(width, 1000.),
                )),
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    DropPage {
                        search: query.to_owned(),
                        rune_range: range,
                    }
                    .show(ui, &mut Settings::default(), DROP_GROUPS);
                });
            },
        );
        output
            .shapes
            .iter()
            .filter_map(|clipped| match &clipped.shape {
                egui::Shape::Text(text) => Some((
                    text.galley.text().to_owned(),
                    (text.pos, text.galley.rows.len()),
                )),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn drops_keep_charm_pairs_and_the_grand_charm_note_on_one_line() {
        for width in [518., 840.] {
            let labels = drop_labels(width, "", 0);
            let at = |label: &str| labels[label].0;
            let grand = "特大咒符（含基德、破免）";
            assert_eq!(at("魔法小型咒符").y, at("滅絕（暗金小型咒符）").y);
            assert_eq!(at("魔法大型咒符").y, at("地獄火炬（暗金大型咒符）").y);
            assert!(
                at("魔法小型咒符").y < at("魔法大型咒符").y && at("魔法大型咒符").y < at(grand).y
            );
            assert!(at("魔法小型咒符").x < at("滅絕（暗金小型咒符）").x);
            for left in ["#1  艾爾", "魔法大型咒符", grand, "魔法与稀有珠宝"] {
                assert_eq!(at(left).x, at("魔法小型咒符").x, "{left}");
            }
            for right in ["地獄火炬（暗金大型咒符）", "暗金珠宝"] {
                assert_eq!(at(right).x, at("滅絕（暗金小型咒符）").x, "{right}");
            }
            for option in DROP_GROUPS.iter().flat_map(DropGroup::toggles) {
                assert_eq!(labels[option.label].1, 1, "{}", option.label);
            }
            let torch = drop_labels(width, "hellfire", 0);
            assert!(
                (torch["地獄火炬（暗金大型咒符）"].0.x - at("滅絕（暗金小型咒符）").x).abs() < 0.5
            );
            assert!(!torch.contains_key("魔法大型咒符"));
        }
    }

    #[test]
    fn drop_choices_show_only_item_names() {
        for range in 0..3 {
            let labels = drop_labels(518., "", range);
            let mut expected: std::collections::HashSet<_> = rune_names()
                .enumerate()
                .skip(range * 11)
                .take(11)
                .map(|(index, (_, chinese))| format!("#{}  {chinese}", index + 1))
                .collect();
            for option in DROP_GROUPS.iter().flat_map(DropGroup::toggles) {
                expected.insert(option.label.to_owned());
            }
            for label in [
                "搜索物品名称或符文编号",
                "符文",
                "1–11",
                "12–22",
                "23–33",
                "咒符",
                "珠宝",
            ] {
                expected.insert(label.to_owned());
            }
            assert_eq!(
                labels
                    .keys()
                    .filter(|s| !s.is_empty())
                    .cloned()
                    .collect::<std::collections::HashSet<_>>(),
                expected
            );
        }
    }

    #[test]
    fn clear_search_icon_is_centered_and_returns_focus_to_the_field() {
        use egui::{Event, Key, Modifiers, PointerButton, Shape};
        let ctx = egui::Context::default();
        theme::install(&ctx);
        let frame = |query: &mut String, events| {
            ctx.run(
                egui::RawInput {
                    events,
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(540., 180.),
                    )),
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| drop_search(ui, query));
                },
            )
        };
        let mut query = String::from("ber");
        let output = frame(&mut query, vec![]);
        let mut circle = None;
        let mut field = None;
        let mut diagonals = Vec::new();
        for shape in output.shapes {
            match shape.shape {
                Shape::Circle(ring) => circle = Some(ring),
                Shape::Rect(rect) if rect.fill == theme::CANVAS => field = Some(rect.rect),
                Shape::LineSegment { points, .. } => diagonals.push(points),
                _ => {}
            }
        }
        let circle = circle.unwrap();
        assert_eq!(circle.fill, egui::Color32::TRANSPARENT);
        assert!((circle.center.y - field.unwrap().center().y).abs() <= 0.5);
        assert_eq!(diagonals.len(), 2);
        for [start, end] in diagonals {
            assert!(start.lerp(end, 0.5).distance(circle.center) < 0.01);
        }
        let pos = circle.center + egui::vec2(circle.radius + 3., 0.);
        for pressed in [true, false] {
            frame(
                &mut query,
                vec![
                    Event::PointerMoved(pos),
                    Event::PointerButton {
                        pos,
                        button: PointerButton::Primary,
                        pressed,
                        modifiers: Modifiers::NONE,
                    },
                ],
            );
        }
        assert!(query.is_empty());
        frame(&mut query, vec![Event::Text("ort".into())]);
        assert_eq!(query, "ort");
        for key in [Key::Tab, Key::Enter] {
            for pressed in [true, false] {
                frame(
                    &mut query,
                    vec![Event::Key {
                        key,
                        physical_key: None,
                        pressed,
                        repeat: false,
                        modifiers: Modifiers::NONE,
                    }],
                );
            }
        }
        assert!(query.is_empty());
        frame(&mut query, vec![Event::Text("贝".into())]);
        assert_eq!(query, "贝");
    }

    #[test]
    fn search_shows_game_names_for_native_and_familiar_terms() {
        for (query, label) in [
            ("#30", "#30  貝"),
            ("ber", "#30  貝"),
            ("贝", "#30  貝"),
            ("扎哈", "#31  喬"),
            ("乔", "#31  喬"),
            ("图尔", "#10  書爾"),
            ("護身符", "滅絕（暗金小型咒符）"),
            ("巨型神符", "特大咒符（含基德、破免）"),
            ("黄色珠宝", "魔法与稀有珠宝"),
            ("獨特珠寶", "暗金珠宝"),
        ] {
            assert!(
                drop_labels(840., query, 0).contains_key(label),
                "{query}: {label}"
            );
        }
    }
}
