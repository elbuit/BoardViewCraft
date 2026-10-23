//! User preferences and settings dialog.

use crate::i18n::{self, Language};
use crate::theme::{ThemePreset, Tokens};
use egui::{Align2, Color32, Context, RichText, Vec2, Window};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Preferences {
    pub language: Language,
    pub theme: ThemePreset,
    pub custom_tokens: Option<Tokens>,
    pub show_net_web: bool,
    pub show_part_labels: bool,
    pub show_pin_annotations: bool,
    pub high_density_lod: bool,
    pub show_opposite_elements: bool,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            language: Language::English,
            theme: ThemePreset::CadcraftDark,
            custom_tokens: None,
            show_net_web: true,
            show_part_labels: true,
            show_pin_annotations: true,
            high_density_lod: true,
            show_opposite_elements: false,
        }
    }
}

/// Displays the preferences window dialog.
/// Returns `true` if any preference was changed this frame.
pub fn show_preferences_window(
    ctx: &Context,
    open: &mut bool,
    prefs: &mut Preferences,
    tokens: &Tokens,
) -> bool {
    if !*open {
        return false;
    }

    let mut changed = false;

    // Check Escape key to close
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        *open = false;
        return false;
    }

    let mut is_open = *open;
    let mut close_requested = false;

    Window::new(RichText::new("Preferences").strong().color(tokens.accent))
        .open(&mut is_open)
        .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
        .resizable(true)
        .min_size(Vec2::new(420.0, 320.0))
        .default_size(Vec2::new(440.0, 360.0))
        .show(ctx, |ui| {
            let lang = prefs.language;
            ui.add_space(4.0);

            // Language Selector
            ui.horizontal(|ui| {
                ui.label(RichText::new(i18n::tr(lang, "prefs.language")).strong().color(tokens.text));
                egui::ComboBox::from_id_salt("language_combo")
                    .selected_text(prefs.language.display_name())
                    .show_ui(ui, |ui| {
                        for l in Language::ALL {
                            if ui.selectable_value(&mut prefs.language, l, l.display_name()).clicked() {
                                changed = true;
                            }
                        }
                    });
            });

            ui.add_space(8.0);
            ui.heading(RichText::new(i18n::tr(lang, "prefs.appearance")).color(tokens.text));
            ui.separator();

            ui.horizontal(|ui| {
                ui.label(RichText::new(i18n::tr(lang, "prefs.color_scheme")).strong().color(tokens.text));
                egui::ComboBox::from_id_salt("theme_preset_combo")
                    .selected_text(prefs.theme.display_name())
                    .show_ui(ui, |ui| {
                        for preset in ThemePreset::ALL {
                            if ui
                                .selectable_value(&mut prefs.theme, preset, preset.display_name())
                                .clicked()
                            {
                                changed = true;
                                if preset == ThemePreset::Custom && prefs.custom_tokens.is_none() {
                                    prefs.custom_tokens = Some(*tokens);
                                }
                            }
                        }
                    });
            });

            // If Custom theme selected, expose color pickers
            if prefs.theme == ThemePreset::Custom {
                let custom = prefs.custom_tokens.get_or_insert_with(|| *tokens);
                ui.add_space(4.0);
                ui.label(RichText::new(i18n::tr(lang, "prefs.custom_colors")).strong().color(tokens.accent));
                egui::Grid::new("custom_colors_grid").num_columns(2).spacing([12.0, 4.0]).show(ui, |ui| {
                    ui.label("Canvas Background:");
                    let mut c = custom.canvas.to_array();
                    if ui.color_edit_button_srgba_unmultiplied(&mut c).changed() {
                        custom.canvas = Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3]);
                        changed = true;
                    }
                    ui.end_row();

                    ui.label("Pin Default Color:");
                    let mut c = custom.pin_default.to_array();
                    if ui.color_edit_button_srgba_unmultiplied(&mut c).changed() {
                        custom.pin_default = Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3]);
                        changed = true;
                    }
                    ui.end_row();

                    ui.label("Pin Highlight (Active Side):");
                    let mut c = custom.pin_highlight_cur_side.to_array();
                    if ui.color_edit_button_srgba_unmultiplied(&mut c).changed() {
                        custom.pin_highlight_cur_side = Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3]);
                        changed = true;
                    }
                    ui.end_row();

                    ui.label("Via Color:");
                    let mut c = custom.via_cur_side.to_array();
                    if ui.color_edit_button_srgba_unmultiplied(&mut c).changed() {
                        custom.via_cur_side = Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3]);
                        changed = true;
                    }
                    ui.end_row();

                    ui.label("Net Web Lines:");
                    let mut c = custom.net_web.to_array();
                    if ui.color_edit_button_srgba_unmultiplied(&mut c).changed() {
                        custom.net_web = Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3]);
                        changed = true;
                    }
                    ui.end_row();

                    ui.label("Board Outline:");
                    let mut c = custom.board_outline.to_array();
                    if ui.color_edit_button_srgba_unmultiplied(&mut c).changed() {
                        custom.board_outline = Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3]);
                        changed = true;
                    }
                    ui.end_row();
                });
            }

            ui.add_space(12.0);
            ui.heading(RichText::new(i18n::tr(lang, "prefs.visualization")).color(tokens.text));
            ui.separator();

            if ui
                .checkbox(
                    &mut prefs.show_net_web,
                    RichText::new(i18n::tr(lang, "prefs.show_net_web")).color(tokens.text),
                )
                .on_hover_text("Draw connecting spiderweb lines between pins and vias on the selected net.")
                .changed()
            {
                changed = true;
            }

            if ui
                .checkbox(
                    &mut prefs.show_part_labels,
                    RichText::new(i18n::tr(lang, "prefs.show_part_labels")).color(tokens.text),
                )
                .on_hover_text("Render part name labels on components when zoomed in.")
                .changed()
            {
                changed = true;
            }

            if ui
                .checkbox(
                    &mut prefs.show_pin_annotations,
                    RichText::new(i18n::tr(lang, "prefs.show_pin_annotations")).color(tokens.text),
                )
                .on_hover_text("Render pin numbers and descriptions when zooming into components.")
                .changed()
            {
                changed = true;
            }

            if ui
                .checkbox(
                    &mut prefs.high_density_lod,
                    RichText::new(i18n::tr(lang, "prefs.adaptive_lod")).color(tokens.text),
                )
                .on_hover_text("Simplify circles to quads/points on dense nets and high zoom outs for 60+ FPS performance.")
                .changed()
            {
                changed = true;
            }

            if ui
                .checkbox(
                    &mut prefs.show_opposite_elements,
                    RichText::new(i18n::tr(lang, "prefs.show_opposite")).color(tokens.text),
                )
                .on_hover_text("When enabled, dimmed pins and vias on the reverse side of the board are visible.")
                .changed()
            {
                changed = true;
            }

            ui.add_space(16.0);
            ui.separator();
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button(i18n::tr(lang, "prefs.close")).clicked() {
                    close_requested = true;
                }
            });
        });

    if !is_open || close_requested {
        *open = false;
    }

    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preferences_serde() {
        let prefs = Preferences {
            theme: ThemePreset::OpenBoardViewClassic,
            custom_tokens: Some(Tokens::DARK),
            language: Language::Catalan,
            show_net_web: false,
            show_part_labels: true,
            show_pin_annotations: false,
            high_density_lod: true,
            show_opposite_elements: false,
        };

        let json = serde_json::to_string(&prefs).expect("Serialize to JSON");
        let deserialized: Preferences = serde_json::from_str(&json).expect("Deserialize from JSON");

        assert_eq!(prefs, deserialized);
    }
}
