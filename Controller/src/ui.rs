//! Windows desktop controller. Each region owns an independent, unapplied draft.
mod about;
mod drops;
mod license;
mod options;
mod pages;
mod previews;
mod session;
mod theme;

use anyhow::Result;
use eframe::egui::{
    self, pos2, vec2, Align, Color32, FontId, Layout, RichText, Stroke, TextureHandle,
};

use crate::Region;
use options::Page;
use pages::PageView;
use session::{EditingSession, SessionStatus};
use theme::{CANVAS, ERROR, GOLD, INK, LINE, MUTED, SURFACE};

const NAVIGATION_WIDTH: f32 = 186.;
const PANEL_GAP: i8 = 7;
const STATUS_HEIGHT: f32 = 22.;
const ACTIONS_HEIGHT: f32 = 55.;

pub fn run() -> Result<()> {
    let image = image::load_from_memory_with_format(
        include_bytes!("../assets/ui/leah.ico"),
        image::ImageFormat::Ico,
    )?
    .into_rgba8();
    let portrait = egui::ColorImage::from_rgba_unmultiplied(
        [image.width() as usize, image.height() as usize],
        &image,
    );
    let icon = egui::IconData {
        width: image.width(),
        height: image.height(),
        rgba: image.into_raw(),
    };
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_icon(icon)
            .with_drag_and_drop(false)
            .with_title(concat!("MzMod设置器v", env!("CARGO_PKG_VERSION")))
            .with_inner_size([1024.0, 720.0])
            .with_min_inner_size([760.0, 515.0]),
        ..Default::default()
    };
    eframe::run_native(
        "MzMod Controller",
        options,
        Box::new(move |cc| Ok(Box::new(Controller::new(&cc.egui_ctx, portrait)?))),
    )
    .map_err(|e| anyhow::anyhow!("无法打开图形界面：{e}"))
}

struct Controller {
    session: EditingSession,
    pages: PageView,
    portrait: TextureHandle,
    waypoint: TextureHandle,
}

impl Controller {
    fn new(ctx: &egui::Context, portrait: egui::ColorImage) -> Result<Self> {
        theme::install(ctx);
        let decoded =
            eframe::icon_data::from_png_bytes(include_bytes!("../assets/ui/waypoint.png"))?;
        let image = egui::ColorImage::from_rgba_unmultiplied(
            [decoded.width as usize, decoded.height as usize],
            &decoded.rgba,
        );
        let session = EditingSession::open()?;
        license::synchronize(ctx);
        Ok(Self {
            session,
            pages: PageView::default(),
            portrait: ctx.load_texture(
                "leah",
                portrait,
                egui::TextureOptions::LINEAR.with_mipmap_mode(Some(egui::TextureFilter::Linear)),
            ),
            waypoint: ctx.load_texture("waypoint", image, egui::TextureOptions::LINEAR),
        })
    }

    fn location_selector(&mut self, ui: &mut egui::Ui, frame: &eframe::Frame) {
        let game_selected = self.session.game_directory().is_some();
        let (default, game) = theme::output_locations(ui, game_selected);
        if default.clicked() {
            self.session.use_default_location();
        }
        if game.clicked() {
            let mut dialog = rfd::FileDialog::new()
                .set_parent(frame)
                .set_title("选择暗黑破坏神II：重制版游戏目录");
            if let Some(path) = self.session.game_directory() {
                dialog = dialog.set_directory(path);
            }
            if let Some(path) = dialog.pick_folder() {
                self.session.select_game_directory(path);
            }
        }
    }

    fn paint_brand(&self, painter: &egui::Painter, center: egui::Pos2) -> egui::Rect {
        let name = painter.layout_no_wrap(
            "MzMod".into(),
            FontId::new(30., egui::FontFamily::Name("Wordmark".into())),
            INK,
        );
        let portrait_size = 42.;
        let gap = 7.;
        let bounds = egui::Rect::from_center_size(
            center,
            vec2(
                portrait_size + gap + name.mesh_bounds.width(),
                portrait_size.max(name.mesh_bounds.height()),
            ),
        );
        let portrait = egui::Align2::LEFT_CENTER
            .anchor_size(bounds.left_center(), vec2(portrait_size, portrait_size));
        painter.image(
            self.portrait.id(),
            portrait,
            egui::Rect::from_min_max(pos2(0., 0.), pos2(1., 1.)),
            Color32::WHITE,
        );
        let name_origin = pos2(
            portrait.right() + gap - name.mesh_bounds.left(),
            center.y - name.mesh_bounds.center().y,
        );
        painter.galley(name_origin, name, INK);
        bounds
    }

    fn header(&mut self, ctx: &egui::Context, busy: bool) {
        egui::TopBottomPanel::top("title")
            .exact_height(68.)
            .frame(egui::Frame::NONE.fill(CANVAS))
            .show(ctx, |ui| {
                let rect = ui.max_rect();
                let center_y = rect.center().y;
                let navigation_right = rect.left() + NAVIGATION_WIDTH;
                let brand_center = pos2(rect.left() + NAVIGATION_WIDTH / 2., center_y);
                let divider_x = navigation_right - f32::from(PANEL_GAP) / 2.;
                let painter = ui.painter();
                let brand = self.paint_brand(painter, brand_center);
                let gap = divider_x - brand.right();
                let section = if self.pages.selected == Page::About {
                    self.pages.selected.title()
                } else {
                    "功能设置"
                };
                let title =
                    painter.layout_no_wrap(section.into(), FontId::proportional(20.), MUTED);
                let title_origin = pos2(
                    divider_x + gap - title.mesh_bounds.left(),
                    center_y - title.mesh_bounds.center().y,
                );

                let region_text: egui::WidgetText = region_name(self.session.region()).into();
                let region_text =
                    region_text.into_galley(ui, None, f32::INFINITY, egui::TextStyle::Button);
                let spacing = ui.spacing();
                let picker_height = (region_text.size().y.max(spacing.icon_width)
                    + 2. * spacing.button_padding.y)
                    .max(spacing.interact_size.y);
                let picker_rect = egui::Align2::RIGHT_CENTER
                    .anchor_size(pos2(rect.right() - 18., center_y), vec2(95., picker_height));

                let art_left = title_origin.x + title.mesh_bounds.right() + 12.;
                let art_right = picker_rect.left() - 12.;
                let art = egui::Rect::from_center_size(
                    pos2((art_left + art_right) / 2., center_y),
                    vec2((art_right - art_left).clamp(0., 515.), 63.),
                );
                painter.image(
                    self.waypoint.id(),
                    art,
                    egui::Rect::from_min_max(pos2(0., 0.45), pos2(1., 0.93)),
                    Color32::WHITE,
                );
                painter.vline(
                    divider_x,
                    (center_y - 12.)..=(center_y + 12.),
                    Stroke::new(1_f32, LINE),
                );
                painter.galley(title_origin, title, MUTED);

                let mut selected = self.session.region();
                let mut picker = ui.new_child(egui::UiBuilder::new().max_rect(picker_rect));
                if busy {
                    picker.disable();
                }
                egui::ComboBox::from_id_salt("region")
                    .width(picker_rect.width())
                    .selected_text(region_text)
                    .show_ui(&mut picker, |ui| {
                        for region in Region::ALL {
                            ui.selectable_value(&mut selected, region, region_name(region));
                        }
                    });
                if selected != self.session.region() {
                    self.session.select_region(selected);
                }
            });
    }

    fn footer(&mut self, ctx: &egui::Context, frame: &eframe::Frame, busy: bool) {
        let (text, color) = match self.session.status() {
            SessionStatus::Error(message) => (message, ERROR),
            SessionStatus::Applying => ("正在生成MzMod…", MUTED),
            SessionStatus::Unapplied => ("有尚未应用的更改", GOLD),
            SessionStatus::Installed => ("设置已应用。下次启动游戏时生效。", GOLD),
            SessionStatus::Exported => (
                "已生成MzMod。关闭游戏，将旧MzMod移出mods后，复制新文件夹到mods。",
                GOLD,
            ),
            SessionStatus::ReadyToInstall => (
                "应用后，会在游戏目录的mods文件夹中安装或更新MzMod，保留其他模组。",
                MUTED,
            ),
            SessionStatus::ReadyToExport => (
                "应用后，将生成的MzMod文件夹复制到对应区服的游戏mods文件夹。",
                MUTED,
            ),
        };
        egui::TopBottomPanel::bottom("status")
            .exact_height(STATUS_HEIGHT)
            .show_separator_line(false)
            .frame(egui::Frame::NONE.inner_margin(egui::Margin::symmetric(13, 0)))
            .show(ctx, |ui| {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.add(egui::Label::new(RichText::new(text).color(color).size(13.)).truncate());
                });
            });

        egui::TopBottomPanel::bottom("actions")
            .exact_height(ACTIONS_HEIGHT)
            .show_separator_line(false)
            .frame(egui::Frame::NONE)
            .show(ctx, |ui| {
                theme::panel(0)
                    .outer_margin(egui::Margin::symmetric(PANEL_GAP, 0))
                    .inner_margin(egui::Margin::symmetric(8, 0))
                    .show(ui, |ui| {
                        ui.set_min_size(ui.max_rect().size());
                        ui.spacing_mut().interact_size.y = 32.;
                        ui.visuals_mut().widgets.hovered.expansion = 0.;
                        ui.visuals_mut().widgets.active.expansion = 0.;
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            let apply = if busy {
                                "正在应用…"
                            } else {
                                "应用全部更改"
                            };
                            if ui.add_enabled(!busy, theme::action(apply, true)).clicked() {
                                let ctx = ctx.clone();
                                self.session.apply(move || ctx.request_repaint());
                            }
                            if ui
                                .add_enabled(!busy, theme::action("恢复本页默认", false))
                                .clicked()
                            {
                                self.session.restore_page(self.pages.selected);
                            }
                            ui.add_space(12.);
                            ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                                ui.add_enabled_ui(!busy, |ui| self.location_selector(ui, frame));
                                let path = self.session.output_path().display().to_string();
                                ui.label(RichText::new("输出到").color(MUTED).size(14.));
                                ui.add(egui::Label::new(RichText::new(&path).size(14.)).truncate())
                                    .on_hover_text(&path);
                            });
                        });
                    });
            });
    }
}

fn region_name(region: Region) -> &'static str {
    match region {
        Region::Int => "国际服",
        Region::Cn => "国服",
    }
}

impl eframe::App for Controller {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.session.poll_apply();
        let busy = self.session.is_applying();
        if ctx.input(|input| input.viewport().close_requested()) {
            if busy {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            } else if self.session.has_edits() {
                let choice = rfd::MessageDialog::new()
                    .set_parent(frame)
                    .set_title("退出MzMod设置")
                    .set_description("仍有尚未应用的更改。确定放弃这些更改并退出？")
                    .set_buttons(rfd::MessageButtons::YesNo)
                    .show();
                if choice != rfd::MessageDialogResult::Yes {
                    ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                }
            }
        }
        self.header(ctx, busy);
        let about_open = self.pages.selected == Page::About;
        if !about_open {
            self.footer(ctx, frame, busy);
        }
        egui::SidePanel::left("navigation")
            .exact_width(NAVIGATION_WIDTH)
            .resizable(false)
            .show_separator_line(false)
            .frame(egui::Frame::NONE)
            .show(ctx, |ui| {
                theme::panel(8).outer_margin(PANEL_GAP).show(ui, |ui| {
                    ui.set_min_size(ui.max_rect().size());
                    ui.add_enabled_ui(!busy, |ui| {
                        let rows = Page::ALL.len() as f32;
                        let gaps = (rows - 1.) * ui.spacing().item_spacing.y;
                        // Keep navigation targets in place when the settings footer is hidden.
                        let height = ui.available_height()
                            - if about_open {
                                STATUS_HEIGHT + ACTIONS_HEIGHT
                            } else {
                                0.
                            };
                        let row_height = ((height - gaps) / rows).clamp(34., 51.);
                        for page in Page::ALL {
                            if theme::navigation(ui, page, self.pages.selected == page, row_height)
                                .clicked()
                            {
                                self.pages.selected = page;
                                ctx.request_repaint();
                            }
                        }
                    });
                });
            });
        egui::CentralPanel::default()
            .frame(theme::panel(15).outer_margin(egui::Margin {
                top: PANEL_GAP,
                bottom: PANEL_GAP,
                left: 0,
                right: PANEL_GAP,
            }))
            .show(ctx, |ui| {
                ui.label(
                    RichText::new(self.pages.selected.title())
                        .font(FontId::proportional(27.))
                        .color(INK),
                );
                ui.separator();
                ui.add_space(3.);
                ui.add_enabled_ui(!busy && !self.session.is_applying(), |ui| {
                    egui::ScrollArea::vertical()
                        .id_salt(self.pages.selected)
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            self.session
                                .edit_page(|region, draft| self.pages.show(ui, region, draft));
                        });
                });
            });
    }

    fn clear_color(&self, _: &egui::Visuals) -> [f32; 4] {
        egui::Rgba::from(SURFACE).to_array()
    }
}
