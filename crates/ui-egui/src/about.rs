//! "About" window: application version and build/runtime information.

use crate::i18n::{self, Language};
use crate::theme::Tokens;
use egui::{Align2, Context, RichText, Vec2, Window};

/// Application display name.
pub const APP_NAME: &str = "BoardViewCraft";

/// Build profile the binary was compiled with.
fn build_profile() -> &'static str {
    if cfg!(debug_assertions) { "debug" } else { "release" }
}

/// Label/value rows shown in the About window, labels translated to `lang`.
pub fn about_rows(lang: Language) -> Vec<(&'static str, String)> {
    let platform = format!("{} ({})", std::env::consts::OS, std::env::consts::ARCH);
    vec![
        (i18n::tr(lang, "about.version"), env!("CARGO_PKG_VERSION").to_string()),
        (i18n::tr(lang, "about.license"), env!("CARGO_PKG_LICENSE").to_string()),
        (i18n::tr(lang, "about.platform"), platform),
        (i18n::tr(lang, "about.build_profile"), build_profile().to_string()),
        (i18n::tr(lang, "about.ui_toolkit"), "egui / eframe (wgpu)".to_string()),
        (i18n::tr(lang, "about.pdf_engine"), "pdfcraft-render".to_string()),
    ]
}

/// Shows the About window while `*open` is true. Escape or the Close button closes it.
pub fn show_about_window(ctx: &Context, open: &mut bool, tokens: &Tokens, lang: Language) {
    if !*open {
        return;
    }
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        *open = false;
        return;
    }

    let mut is_open = *open;
    let mut close_requested = false;

    Window::new(RichText::new(i18n::tr(lang, "about.title")).strong().color(tokens.accent()))
        .open(&mut is_open)
        .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
        .resizable(false)
        .collapsible(false)
        .min_width(360.0)
        .show(ctx, |ui| {
            ui.add_space(4.0);
            ui.heading(RichText::new(APP_NAME).strong().color(tokens.accent()));
            ui.label(RichText::new(i18n::tr(lang, "about.description")).color(tokens.text_dim()));
            ui.add_space(8.0);
            ui.separator();

            egui::Grid::new("about_grid")
                .num_columns(2)
                .spacing([16.0, 6.0])
                .show(ui, |ui| {
                    for (label, value) in about_rows(lang) {
                        ui.label(RichText::new(label).strong().color(tokens.text()));
                        ui.label(RichText::new(value).color(tokens.text()));
                        ui.end_row();
                    }
                });

            ui.add_space(12.0);
            ui.separator();
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button(i18n::tr(lang, "about.close")).clicked() {
                    close_requested = true;
                }
            });
        });

    if !is_open || close_requested {
        *open = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn value_of(rows: &[(&'static str, String)], label: &str) -> String {
        rows.iter()
            .find(|(l, _)| *l == label)
            .map(|(_, v)| v.clone())
            .unwrap_or_else(|| panic!("missing About row {label}"))
    }

    #[test]
    fn test_about_rows_contain_version_license_platform_and_profile() {
        let rows = about_rows(Language::English);
        assert_eq!(value_of(&rows, "Version"), env!("CARGO_PKG_VERSION"));
        assert_eq!(value_of(&rows, "License"), env!("CARGO_PKG_LICENSE"));
        let platform = value_of(&rows, "Platform");
        assert!(platform.contains(std::env::consts::OS));
        assert!(platform.contains(std::env::consts::ARCH));
        assert_eq!(value_of(&rows, "Build profile"), build_profile());
        assert!(value_of(&rows, "PDF engine").contains("pdfcraft"));
    }

    #[test]
    fn test_about_rows_are_translated() {
        let rows = about_rows(Language::Spanish);
        assert_eq!(value_of(&rows, "Versión"), env!("CARGO_PKG_VERSION"));
    }
}
