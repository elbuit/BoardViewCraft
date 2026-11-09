//! Quick toolbar row under the menu bar (NexusBV style): navigation history, fit, side
//! toggles, searchable part/net pickers, flips, layer mode and a quick-preferences popup.
//!
//! The row only reports what was requested ([`ToolbarAction`]); the app applies it, so the
//! toolbar shares the exact code paths of the menus, hotkeys and side panel.

use crate::i18n::{self, Language};
use crate::keymap::Action;
use crate::preferences::Preferences;
use bvc_core::{Board, Side};
use egui::{Button, ComboBox, PopupCloseBehavior, RichText, ScrollArea, TextEdit, Ui};

/// Icon glyphs. Each one is covered by egui's bundled proportional fonts (Ubuntu-Light plus the
/// monochrome emoji fonts); `toolbar_glyphs_render_with_bundled_fonts` checks it.
pub const ICON_BACK: &str = "⏴";
pub const ICON_FORWARD: &str = "⏵";
pub const ICON_FIT: &str = "⛶";
pub const ICON_FLIP_H: &str = "⬌";
pub const ICON_FLIP_V: &str = "⬍";
pub const ICON_LAYERS: &str = "🗐";
pub const ICON_MORE: &str = "…";

const COMBO_WIDTH: f32 = 170.0;

/// Something the user asked for from the toolbar this frame.
#[derive(Debug, Clone, PartialEq)]
pub enum ToolbarAction {
    NavBack,
    NavForward,
    Fit,
    ShowSide(Side),
    SelectPart(usize),
    SelectNet(String),
    MirrorX,
    MirrorY,
    /// A quick preference changed: canvas caches must be rebuilt and preferences saved.
    PrefsChanged,
}

/// Filter texts of the two pickers, kept between frames.
#[derive(Debug, Default)]
pub struct ToolbarState {
    pub part_filter: String,
    pub net_filter: String,
}

/// Read-only app state the toolbar row needs.
pub struct ToolbarContext<'a> {
    pub board: Option<&'a Board>,
    pub lang: Language,
    pub can_back: bool,
    pub can_forward: bool,
    /// Side highlighted by the Top/Bottom toggles (`None` when neither single side is shown).
    pub current_side: Option<Side>,
    pub mirror_x: bool,
    pub mirror_y: bool,
    pub selected_part: Option<usize>,
    pub selected_net: Option<&'a str>,
}

/// Positions of the `names` containing `needle`, ignoring case (all of them for an empty needle).
pub fn filter_names<'a>(names: impl IntoIterator<Item = &'a str>, needle: &str) -> Vec<usize> {
    let needle = needle.trim().to_lowercase();
    names
        .into_iter()
        .enumerate()
        .filter(|(_, name)| needle.is_empty() || name.to_lowercase().contains(&needle))
        .map(|(i, _)| i)
        .collect()
}

/// Draws the toolbar row and returns the actions requested this frame. Quick preferences are
/// edited in place on `prefs` (reported with [`ToolbarAction::PrefsChanged`]).
pub fn show_toolbar(
    ui: &mut Ui,
    cx: &ToolbarContext<'_>,
    state: &mut ToolbarState,
    prefs: &mut Preferences,
) -> Vec<ToolbarAction> {
    let lang = cx.lang;
    let tr = |key| i18n::tr(lang, key);
    let has_board = cx.board.is_some();
    let mut actions = Vec::new();

    ui.horizontal(|ui| {
        if icon_button(ui, has_board && cx.can_back, ICON_BACK, false, prefs.keymap.hint(tr("toolbar.nav_back"), Action::NavBack)) {
            actions.push(ToolbarAction::NavBack);
        }
        if icon_button(ui, has_board && cx.can_forward, ICON_FORWARD, false, prefs.keymap.hint(tr("toolbar.nav_forward"), Action::NavForward)) {
            actions.push(ToolbarAction::NavForward);
        }
        if icon_button(ui, has_board, ICON_FIT, false, prefs.keymap.hint(tr("toolbar.fit"), Action::FitView)) {
            actions.push(ToolbarAction::Fit);
        }

        for (side, label, tip) in [(Side::Top, "tabs.top", "toolbar.show_top"), (Side::Bottom, "tabs.bottom", "toolbar.show_bottom")] {
            let button = Button::new(tr(label)).selected(cx.current_side == Some(side));
            if ui.add_enabled(has_board, button).on_hover_text(tr(tip)).clicked() {
                actions.push(ToolbarAction::ShowSide(side));
            }
        }

        ui.label(tr("toolbar.parts"));
        let part_text = cx.board.zip(cx.selected_part).and_then(|(b, i)| b.parts.get(i)).map_or("", |p| p.name.as_str());
        if let Some(i) = search_combo(ui, "toolbar_parts", has_board, part_text, &mut state.part_filter, lang, |needle| {
            let Some(b) = cx.board else { return Vec::new() };
            filter_names(b.parts.iter().map(|p| p.name.as_str()), needle).into_iter().map(|i| (i, b.parts[i].name.as_str())).collect()
        }) {
            actions.push(ToolbarAction::SelectPart(i));
        }

        ui.label(tr("toolbar.nets"));
        let net_text = cx.selected_net.unwrap_or("");
        if let Some(name) = search_combo(ui, "toolbar_nets", has_board, net_text, &mut state.net_filter, lang, |needle| {
            let Some(b) = cx.board else { return Vec::new() };
            let names: Vec<&str> = b.nets.keys().map(String::as_str).collect();
            filter_names(names.iter().copied(), needle).into_iter().map(|i| (names[i], names[i])).collect()
        }) {
            actions.push(ToolbarAction::SelectNet(name.to_string()));
        }

        ui.separator();
        if icon_button(ui, has_board, ICON_FLIP_H, cx.mirror_x, prefs.keymap.hint(tr("toolbar.flip_h"), Action::MirrorX)) {
            actions.push(ToolbarAction::MirrorX);
        }
        if icon_button(ui, has_board, ICON_FLIP_V, cx.mirror_y, prefs.keymap.hint(tr("toolbar.flip_v"), Action::MirrorY)) {
            actions.push(ToolbarAction::MirrorY);
        }

        ui.separator();
        ui.add_enabled(false, Button::new(RichText::new(ICON_LAYERS).size(15.0)))
            .on_disabled_hover_text(tr("toolbar.layers_unavailable"));

        let more = ui.add(Button::new(RichText::new(ICON_MORE).size(15.0))).on_hover_text(tr("toolbar.quick_prefs"));
        egui::Popup::from_toggle_button_response(&more)
            .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
            .show(|ui| {
                let mut changed = false;
                for (value, key) in [
                    (&mut prefs.show_pins, "prefs.show_pins"),
                    (&mut prefs.show_vias, "prefs.show_vias"),
                    (&mut prefs.show_pin_annotations, "prefs.show_pin_annotations"),
                    (&mut prefs.show_part_labels, "prefs.show_part_labels"),
                    (&mut prefs.show_net_web, "prefs.show_net_web"),
                ] {
                    changed |= ui.checkbox(value, tr(key)).changed();
                }
                if changed {
                    actions.push(ToolbarAction::PrefsChanged);
                }
            });
    });
    actions
}

/// An icon button with a tooltip (shown when disabled too). Returns whether it was clicked.
fn icon_button(ui: &mut Ui, enabled: bool, glyph: &str, selected: bool, tip: String) -> bool {
    let button = Button::new(RichText::new(glyph).size(15.0)).selected(selected);
    ui.add_enabled(enabled, button).on_hover_text(&tip).on_disabled_hover_text(&tip).clicked()
}

/// A combo box whose popup has a filter field (case-insensitive substring) above a virtualized
/// list. `candidates` maps the filter text to `(value, label)` pairs; it only runs while the
/// popup is open. Returns the value picked this frame.
fn search_combo<'a, K>(
    ui: &mut Ui,
    id_salt: &str,
    enabled: bool,
    selected_text: &str,
    filter: &mut String,
    lang: Language,
    candidates: impl FnOnce(&str) -> Vec<(K, &'a str)>,
) -> Option<K> {
    let mut picked = None;
    ui.add_enabled_ui(enabled, |ui| {
        ComboBox::from_id_salt(id_salt)
            .width(COMBO_WIDTH)
            .selected_text(selected_text)
            .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
            .show_ui(ui, |ui| {
                ui.set_min_width(COMBO_WIDTH + 60.0);
                let edit = ui.add(TextEdit::singleline(filter).hint_text(i18n::tr(lang, "toolbar.filter_hint")));
                if ui.memory(|m| m.focused().is_none()) {
                    edit.request_focus();
                }
                let mut items = candidates(filter);
                if items.is_empty() {
                    ui.label(RichText::new(i18n::tr(lang, "toolbar.no_matches")).weak());
                    return;
                }
                let row_height = ui.spacing().interact_size.y;
                let mut clicked = None;
                ScrollArea::vertical().id_salt((id_salt, "rows")).max_height(320.0).show_rows(ui, row_height, items.len(), |ui, range| {
                    for idx in range {
                        let label = items[idx].1;
                        if ui.selectable_label(label == selected_text, label).clicked() {
                            clicked = Some(idx);
                        }
                    }
                });
                if let Some(idx) = clicked {
                    picked = Some(items.swap_remove(idx).0);
                    ui.close();
                }
            });
    });
    picked
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_is_a_case_insensitive_substring_match() {
        let names = ["U1", "u12", "R5", "PP3V3_S5"];
        assert_eq!(filter_names(names, ""), vec![0, 1, 2, 3]);
        assert_eq!(filter_names(names, "u1"), vec![0, 1]);
        assert_eq!(filter_names(names, " 3v3 "), vec![3]);
        assert!(filter_names(names, "zz").is_empty());
    }

    #[test]
    fn toolbar_glyphs_render_with_bundled_fonts() {
        let ctx = egui::Context::default();
        let font = egui::FontId::proportional(15.0);
        let mut missing = Vec::new();
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            for glyph in [ICON_BACK, ICON_FORWARD, ICON_FIT, ICON_FLIP_H, ICON_FLIP_V, ICON_LAYERS, ICON_MORE] {
                if !ui.fonts_mut(|f| f.has_glyphs(&font, glyph)) {
                    missing.push(glyph);
                }
            }
        });
        // The font atlas upload is not rendered anywhere in a test.
        output.textures_delta.clear();
        assert!(missing.is_empty(), "these glyphs would render as boxes: {missing:?}");
    }
}
