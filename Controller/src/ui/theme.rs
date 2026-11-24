use eframe::egui::{
    self, pos2, vec2, Align2, Color32, FontData, FontDefinitions, FontFamily, FontId, Margin,
    Painter, Pos2, Rect, Response, RichText, Sense, Stroke, StrokeKind, Ui,
};
use std::path::PathBuf;

use super::options::Page;

pub const OPTION_TITLE_SIZE: f32 = 17.;

pub const CANVAS: Color32 = Color32::from_rgb(19, 19, 18);
pub const SURFACE: Color32 = Color32::from_rgb(28, 28, 26);
pub const PANEL: Color32 = Color32::from_rgb(34, 32, 29);
pub const LINE: Color32 = Color32::from_rgb(84, 74, 59);
pub const INK: Color32 = Color32::from_rgb(235, 226, 206);
pub const MUTED: Color32 = Color32::from_rgb(186, 177, 157);
pub const GOLD: Color32 = Color32::from_rgb(211, 168, 91);
pub const SELECTED: Color32 = Color32::from_rgb(66, 54, 34);
pub const ERROR: Color32 = Color32::from_rgb(255, 163, 139);

pub fn install(ctx: &egui::Context) {
    // An OS theme event can arrive after the window is created.
    ctx.set_theme(egui::Theme::Dark);
    let mut fonts = FontDefinitions::default();
    let cjk = "Chinese UI fallback".to_owned();
    fonts.font_data.insert(
        cjk.clone(),
        FontData::from_static(include_bytes!("../../assets/ui/ui-glyphs.ttf")).into(),
    );
    for family in [FontFamily::Proportional, FontFamily::Monospace] {
        fonts.families.entry(family).or_default().push(cjk.clone());
    }
    let directory = std::env::var_os("WINDIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Windows"))
        .join("Fonts");
    for name in ["msyh.ttc", "msjh.ttc", "simsun.ttc"] {
        if let Ok(data) = std::fs::read(directory.join(name)) {
            fonts
                .font_data
                .insert(name.into(), FontData::from_owned(data).into());
            for family in [FontFamily::Proportional, FontFamily::Monospace] {
                let list = fonts.families.entry(family).or_default();
                list.insert(list.len().saturating_sub(1), name.into());
            }
            break;
        }
    }
    let mut wordmark_fonts = fonts.families[&FontFamily::Proportional].clone();
    for name in ["georgia.ttf", "cambria.ttc", "times.ttf"] {
        if let Ok(data) = std::fs::read(directory.join(name)) {
            fonts
                .font_data
                .insert(name.into(), FontData::from_owned(data).into());
            wordmark_fonts.insert(0, name.into());
            break;
        }
    }
    fonts
        .families
        .insert(FontFamily::Name("Wordmark".into()), wordmark_fonts);
    ctx.set_fonts(fonts);
    let mut style = (*ctx.style()).clone();
    style.visuals = egui::Visuals::dark();
    style.visuals.override_text_color = Some(INK);
    style.visuals.panel_fill = SURFACE;
    style.visuals.window_fill = PANEL;
    style.visuals.extreme_bg_color = CANVAS;
    style.visuals.selection.bg_fill = SELECTED;
    style.visuals.selection.stroke = Stroke::new(1.5_f32, GOLD);
    for w in [
        &mut style.visuals.widgets.inactive,
        &mut style.visuals.widgets.noninteractive,
    ] {
        w.bg_fill = PANEL;
        w.weak_bg_fill = PANEL;
        w.bg_stroke = Stroke::new(1.0_f32, LINE);
        w.fg_stroke = Stroke::new(1.0_f32, INK);
    }
    style.visuals.widgets.hovered.weak_bg_fill = SELECTED;
    style.visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, GOLD);
    style.visuals.widgets.active.weak_bg_fill = SELECTED;
    style.visuals.widgets.active.bg_stroke = Stroke::new(1.0_f32, GOLD);
    style.visuals.hyperlink_color = GOLD;
    style
        .text_styles
        .insert(egui::TextStyle::Body, FontId::proportional(16.0));
    style
        .text_styles
        .insert(egui::TextStyle::Button, FontId::proportional(16.0));
    style.spacing.item_spacing = vec2(8.0, 6.0);
    style.spacing.button_padding = vec2(10.0, 6.0);
    let style = std::sync::Arc::new(style);
    ctx.set_style_of(egui::Theme::Light, style.clone());
    ctx.set_style_of(egui::Theme::Dark, style);
}

pub fn panel(margin: i8) -> egui::Frame {
    egui::Frame::new()
        .fill(SURFACE)
        .stroke(Stroke::new(1.0_f32, LINE))
        .corner_radius(4)
        .inner_margin(Margin::same(margin))
}

pub fn action(text: &str, primary: bool) -> egui::Button<'_> {
    let (fill, ink, edge) = if primary {
        (GOLD, CANVAS, GOLD)
    } else {
        (PANEL, INK, LINE)
    };
    egui::Button::new(RichText::new(text).color(ink))
        .fill(fill)
        .stroke(Stroke::new(1.0_f32, edge))
        .corner_radius(4)
}

pub fn output_locations(ui: &mut Ui, game_selected: bool) -> (Response, Response) {
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.x = 0.;
        let mut segment = |label, selected, corners| {
            let widgets = &mut ui.visuals_mut().widgets;
            for state in [
                &mut widgets.inactive,
                &mut widgets.hovered,
                &mut widgets.active,
            ] {
                state.corner_radius = corners;
                state.bg_stroke = Stroke::NONE;
            }
            let color = if selected { GOLD } else { MUTED };
            ui.selectable_label(selected, RichText::new(label).color(color))
        };
        let left = egui::CornerRadius {
            nw: 4,
            sw: 4,
            ..Default::default()
        };
        let right = egui::CornerRadius {
            ne: 4,
            se: 4,
            ..Default::default()
        };
        let default = segment("默认位置", !game_selected, left);
        let game = segment("选择游戏目录", game_selected, right);
        let bounds = default.rect.union(game.rect);
        let edge = Stroke::new(1.0_f32, LINE);
        ui.painter()
            .rect_stroke(bounds, 4, edge, StrokeKind::Inside);
        ui.painter().vline(game.rect.left(), bounds.y_range(), edge);
        for (option, corners) in [(&default, left), (&game, right)] {
            if option.has_focus() {
                ui.painter().rect_stroke(
                    option.rect,
                    corners,
                    Stroke::new(1.0_f32, GOLD),
                    StrokeKind::Inside,
                );
            }
        }
        (default, game)
    })
    .inner
}

fn toggle_row(ui: &mut Ui, height: f32, label: &str, value: &mut bool) -> Rect {
    let (rect, mut response) =
        ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::click());
    if response.clicked() {
        *value = !*value;
        response.mark_changed();
    }
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Checkbox, ui.is_enabled(), *value, label)
    });
    if response.hovered() || response.has_focus() {
        ui.painter().rect_filled(rect, 3., PANEL);
    }
    if response.has_focus() {
        ui.painter()
            .rect_stroke(rect, 3., Stroke::new(1_f32, GOLD), StrokeKind::Inside);
    }
    response.on_hover_cursor(egui::CursorIcon::PointingHand);
    rect
}

pub fn preference(ui: &mut Ui, label: &str, description: &str, value: &mut bool) {
    let width = ui.available_width();
    let padding = 7.0;
    let description = ui.painter().layout(
        description.to_owned(),
        FontId::proportional(13.0),
        MUTED,
        (width - 2.0 * padding).max(1.0),
    );
    let title_height = 22.0;
    let vertical_padding = 5.0;
    let description_y = vertical_padding + title_height + 2.0;
    let height = description_y + description.size().y + vertical_padding;
    let rect = toggle_row(ui, height, label, value);
    let p = ui.painter();
    let left = rect.left() + padding;
    let title_y = rect.top() + vertical_padding + title_height * 0.5;
    p.text(
        pos2(left, title_y),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(OPTION_TITLE_SIZE),
        INK,
    );
    p.galley(pos2(left, rect.top() + description_y), description, MUTED);
    let rail = Rect::from_center_size(pos2(rect.right() - padding - 21., title_y), vec2(42., 22.));
    p.rect_filled(rail, 11.0, if *value { GOLD } else { LINE });
    p.circle_filled(
        pos2(
            if *value {
                rail.right() - 11.0
            } else {
                rail.left() + 11.0
            },
            rail.center().y,
        ),
        7.2,
        INK,
    );
    p.line_segment(
        [rect.left_bottom(), rect.right_bottom()],
        Stroke::new(0.6_f32, LINE),
    );
}

pub fn drop_choice(ui: &mut Ui, selected: &mut bool, label: &str) {
    let width = ui.available_width();
    let text_width = (width - 36.).max(1.);
    let title = ui.painter().layout(
        label.to_owned(),
        FontId::proportional(OPTION_TITLE_SIZE),
        INK,
        text_width,
    );
    let padding = ((32. - title.rows[0].height()) / 2.).max(3.);
    let height = title.size().y + padding * 2.;
    let rect = toggle_row(ui, height, label, selected);
    let p = ui.painter();
    let center = pos2(
        rect.left() + 13.,
        rect.top() + padding + title.rows[0].height() / 2.,
    );
    let square = Rect::from_center_size(center, vec2(18., 18.));
    p.rect_filled(square, 2., if *selected { GOLD } else { CANVAS });
    p.rect_stroke(
        square,
        2.,
        Stroke::new(1_f32, if *selected { GOLD } else { MUTED }),
        StrokeKind::Inside,
    );
    if *selected {
        p.add(egui::Shape::line(
            vec![
                center + vec2(-5., 0.),
                center + vec2(-1., 4.),
                center + vec2(5., -5.),
            ],
            Stroke::new(2_f32, CANVAS),
        ));
    }
    let origin = pos2(rect.left() + 30., rect.top() + padding);
    p.galley(origin, title, INK);
}

pub fn navigation(ui: &mut Ui, page: Page, selected: bool, height: f32) -> Response {
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::click());
    let p = ui.painter();
    if selected || response.hovered() || response.has_focus() {
        p.rect_filled(rect, 3.0, if selected { SELECTED } else { PANEL });
        if selected || response.has_focus() {
            p.rect_stroke(rect, 3.0, Stroke::new(1.0_f32, GOLD), StrokeKind::Inside);
        }
    }
    let color = if selected { GOLD } else { MUTED };
    icon(
        p,
        pos2(rect.left() + 13.0, rect.center().y - 11.0),
        page,
        color,
    );
    p.text(
        pos2(rect.left() + 48.0, rect.center().y),
        Align2::LEFT_CENTER,
        page.title(),
        FontId::proportional(18.0),
        INK,
    );
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::SelectableLabel,
            true,
            selected,
            page.title(),
        )
    });
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn icon(p: &Painter, origin: Pos2, page: Page, color: Color32) {
    let pt = |x: f32, y: f32| origin + vec2(x, y);
    let stroke = Stroke::new(1.5_f32, color);
    let line = |coords: &[(f32, f32)]| {
        p.add(egui::Shape::line(
            coords.iter().map(|&(x, y)| pt(x, y)).collect(),
            stroke,
        ));
    };
    match page {
        Page::About => {
            line(&[
                (11., 5.),
                (7., 3.),
                (1., 3.),
                (1., 18.),
                (7., 18.),
                (11., 20.),
                (15., 18.),
                (21., 18.),
                (21., 3.),
                (15., 3.),
                (11., 5.),
                (11., 20.),
            ]);
        }
        Page::Drops => {
            line(&[(11., 1.), (9., 17.), (13., 17.), (11., 1.)]);
            line(&[
                (7., 16.),
                (2., 18.),
                (4., 20.),
                (18., 20.),
                (20., 18.),
                (15., 16.),
            ]);
        }
        Page::Names => {
            line(&[
                (2., 6.),
                (11., 2.),
                (20., 6.),
                (20., 16.),
                (11., 21.),
                (2., 16.),
                (2., 6.),
                (11., 11.),
                (20., 6.),
            ]);
            line(&[(11., 11.), (11., 21.)]);
        }
        Page::Runes => {
            line(&[(6., 2.), (16., 2.), (20., 20.), (2., 20.), (6., 2.)]);
            line(&[(8., 6.), (11., 10.), (15., 7.), (11., 10.), (11., 16.)]);
        }
        Page::Skills => {
            line(&[
                (1., 11.),
                (5., 6.),
                (11., 4.),
                (17., 6.),
                (21., 11.),
                (17., 16.),
                (11., 18.),
                (5., 16.),
                (1., 11.),
            ]);
            p.circle_stroke(pt(11., 11.), 3.0, stroke);
        }
        Page::Guides => {
            p.circle_stroke(pt(11., 11.), 8.0, stroke);
            line(&[(11., 0.), (9., 12.), (11., 22.), (13., 10.), (11., 0.)]);
            line(&[(0., 11.), (7., 11.)]);
            line(&[(16., 11.), (22., 11.)]);
        }
        Page::Cube => {
            line(&[
                (3., 7.),
                (5., 3.),
                (17., 3.),
                (19., 7.),
                (19., 19.),
                (3., 19.),
                (3., 7.),
                (19., 7.),
            ]);
            line(&[(10., 7.), (10., 11.), (12., 11.), (12., 7.)]);
        }
        Page::Appearance => {
            p.rect_stroke(
                Rect::from_min_max(pt(1., 3.), pt(21., 19.)),
                2.0,
                stroke,
                StrokeKind::Inside,
            );
            line(&[(2., 8.), (20., 8.)]);
            line(&[(8., 8.), (8., 18.)]);
        }
    }
}
