use eframe::egui::Ui;

use crate::{Region, Settings};

use super::{
    about,
    drops::DropPage,
    options::{Page, PageContent},
    theme,
};

pub(super) struct PageView {
    pub(super) selected: Page,
    drops: DropPage,
}

impl Default for PageView {
    fn default() -> Self {
        Self {
            selected: Page::About,
            drops: DropPage::default(),
        }
    }
}

impl PageView {
    pub(super) fn show(&mut self, ui: &mut Ui, region: Region, settings: &mut Settings) {
        match self.selected.content() {
            PageContent::About => about::show(ui),
            PageContent::Drops(groups) => self.drops.show(ui, settings, groups),
            PageContent::Preferences { toggles, preview } => {
                for option in toggles {
                    option.show_control(ui, settings, |ui, value| {
                        theme::preference(ui, option.label, option.detail, value);
                    });
                }
                if let Some(show) = preview {
                    show(ui, region, &settings.effective());
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui;

    #[test]
    fn cube_page_disables_automatic_open_while_the_cube_is_off() {
        use egui::{Event, Modifiers, PointerButton};
        let ctx = egui::Context::default();
        theme::install(&ctx);
        let mut settings = crate::enabled_features();
        let frame = |s: &mut Settings, events: Vec<Event>| {
            ctx.run(
                egui::RawInput {
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        PageView {
                            selected: Page::Cube,
                            ..PageView::default()
                        }
                        .show(ui, Region::Int, s);
                    });
                },
            )
        };
        let initial = frame(&mut settings, vec![]);
        let center = |label: &str| {
            initial
                .shapes
                .iter()
                .find_map(|clipped| {
                    if let egui::Shape::Text(text) = &clipped.shape {
                        if text.galley.text() == label {
                            return Some(text.pos + egui::vec2(3., 5.));
                        }
                    }
                    None
                })
                .unwrap()
        };
        let click = |s: &mut Settings, pos| {
            for pressed in [true, false] {
                frame(
                    s,
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
        };
        let cube = center("启用迷你魔盒");
        let automatic = center("进入游戏自动打开");
        click(&mut settings, cube);
        assert!(!settings.mini_cube);
        click(&mut settings, automatic);
        assert!(settings.auto_open_cube);
        click(&mut settings, cube);
        click(&mut settings, automatic);
        assert!(settings.mini_cube);
        assert!(!settings.auto_open_cube);
    }
}
