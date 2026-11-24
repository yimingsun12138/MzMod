use eframe::egui::{self, RichText, Ui};

use super::{
    license,
    theme::{GOLD, MUTED},
};
include!(concat!(env!("OUT_DIR"), "/about_copy.rs"));

pub(super) fn show(ui: &mut Ui) {
    let paragraph_spacing = ui.text_style_height(&egui::TextStyle::Body);
    ui.add(egui::Label::new(DESCRIPTION).wrap());
    ui.add_space(paragraph_spacing);

    egui::Grid::new("mod_information")
        .num_columns(2)
        .spacing([16., 4.])
        .show(ui, |ui| {
            for &(label, value) in MOD_INFO {
                ui.label(RichText::new(label).color(MUTED));
                ui.label(value);
                ui.end_row();
            }
            ui.label(RichText::new("项目地址").color(MUTED));
            ui.hyperlink(env!("CARGO_PKG_REPOSITORY"));
            ui.end_row();
        });
    ui.add_space(paragraph_spacing);
    ui.label("如果MzMod对你有帮助，欢迎在GitHub上点亮Star。");
    ui.collapsing(concat!(env!("CARGO_PKG_LICENSE"), "开源许可证"), |ui| {
        let text = license::text(ui.ctx());
        ui.add(egui::Label::new(RichText::new(text.trim()).size(14.)).wrap());
    });

    section(ui, "使用方法");
    for (index, step) in INSTALLATION.iter().enumerate() {
        ui.horizontal_top(|ui| {
            ui.label(RichText::new(format!("{}.", index + 1)).color(MUTED));
            ui.add(egui::Label::new(*step).wrap());
        });
    }

    section(ui, "免责声明");
    ui.add(egui::Label::new(DISCLAIMER).wrap());
}

fn section(ui: &mut Ui, title: &str) {
    ui.add_space(10.);
    ui.separator();
    ui.add_space(3.);
    ui.label(RichText::new(title).size(19.).color(GOLD));
    ui.add_space(3.);
}
