//! User preferences and settings dialog.

use crate::theme::{ThemePreset, Tokens};
use egui::{Align2, Context, RichText, Vec2, Window};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Preferences {
    pub theme: ThemePreset,
    pub show_net_web: bool,
    pub show_part_labels: bool,
    pub show_pin_annotations: bool,
    pub high_density_lod: bool,
    pub show_opposite_elements: bool,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            theme: ThemePreset::CadcraftDark,
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
            ui.add_space(4.0);

            ui.heading(RichText::new("Appearance & Theme").color(tokens.text));
            ui.separator();

            ui.horizontal(|ui| {
                ui.label(RichText::new("Color Scheme:").strong().color(tokens.text));
                egui::ComboBox::from_id_salt("theme_preset_combo")
                    .selected_text(prefs.theme.display_name())
                    .show_ui(ui, |ui| {
                        for preset in ThemePreset::ALL {
                            if ui
                                .selectable_value(&mut prefs.theme, preset, preset.display_name())
                                .clicked()
                            {
                                changed = true;
                            }
                        }
                    });
            });

            ui.add_space(12.0);
            ui.heading(RichText::new("Visualization").color(tokens.text));
            ui.separator();

            if ui
                .checkbox(
                    &mut prefs.show_net_web,
                    RichText::new("Show Net Web (Ratline connection lines)").color(tokens.text),
                )
                .on_hover_text("Draw connecting spiderweb lines between pins and vias on the selected net.")
                .changed()
            {
                changed = true;
            }

            if ui
                .checkbox(
                    &mut prefs.show_part_labels,
                    RichText::new("Show Part Designators").color(tokens.text),
                )
                .on_hover_text("Render part name labels on components when zoomed in.")
                .changed()
            {
                changed = true;
            }

            if ui
                .checkbox(
                    &mut prefs.show_pin_annotations,
                    RichText::new("Show Pin Annotations & Numbers").color(tokens.text),
                )
                .on_hover_text("Render pin numbers and descriptions when zooming into components.")
                .changed()
            {
                changed = true;
            }

            if ui
                .checkbox(
                    &mut prefs.high_density_lod,
                    RichText::new("Adaptive LOD Decimation").color(tokens.text),
                )
                .on_hover_text("Simplify circles to quads/points on dense nets and high zoom outs for 60+ FPS performance.")
                .changed()
            {
                changed = true;
            }

            if ui
                .checkbox(
                    &mut prefs.show_opposite_elements,
                    RichText::new("Show Opposite Side Elements (X-Ray)").color(tokens.text),
                )
                .on_hover_text("When enabled, dimmed pins and vias on the reverse side of the board are visible.")
                .changed()
            {
                changed = true;
            }

            ui.add_space(16.0);
            ui.separator();
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Close").clicked() {
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
