//! Side inspection panel for BoardViewCraft.

use crate::i18n::{self, Language};
use crate::overlay::OverlayManager;
use crate::overlay_align::{AlignmentSession, MAX_PAIRS};
use crate::theme::Tokens;
use bvc_core::{Board, Point, Side};
use egui::{Button, Color32, DragValue, RichText, ScrollArea, Slider, TextEdit, Ui};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SearchSubTab {
    #[default]
    Parts,
    Nets,
    Notes,
}

#[derive(Default)]
pub struct SidePanelState {
    pub filter_part: String,
    pub filter_net: String,
    pub filter_notes: String,
    pub search_tab: SearchSubTab,
    /// Quick component prefix filter (e.g. "U", "R", "C", "Q", "D", "L", "F", "J").
    pub selected_prefix: Option<&'static str>,
    /// Photo overlay adjustment is in progress: the panel shows the overlay view
    /// (and the canvas manipulates the photo) until the user presses "Done".
    pub overlay_editing: bool,
    /// Part the board view should be centered on, requested by a search click.
    /// Consumed (and cleared) by the app right after `show_side_panel` runs.
    pub focus_part: Option<usize>,
    /// Part whose details stay on screen while one of its pins, picked from those
    /// details, is selected. Dropped as soon as a different part (or none) is selected.
    pub stay_on_part: Option<usize>,
    /// Search matches, recomputed only when the filter text or the board changes.
    pub search: SearchCache,
    /// Draft inputs for editing a technician note.
    pub note_edit_text: String,
    pub note_edit_voltage: String,
    pub note_edit_resistance: String,
    pub note_edit_diode: String,
    pub note_active_target: Option<bvc_core::NoteTarget>,
    pub note_target_is_pin: Option<bool>,
    pub note_dirty: bool,
}

/// Identity of the board a cached search belongs to (address plus sizes, so a new board
/// loaded at the same address is still detected).
type BoardStamp = (usize, usize, usize, usize);

fn board_stamp(b: &Board) -> BoardStamp {
    (b as *const Board as usize, b.parts.len(), b.pins.len(), b.nets.len())
}

fn get_clipboard_text() -> Option<String> {
    arboard::Clipboard::new().ok().and_then(|mut cb| cb.get_text().ok())
}

/// Cached results of the part and net search lists.
///
/// The lists are virtualised (only visible rows are laid out), so the per-frame cost
/// no longer grows with the number of parts and nets; the filtering itself only runs
/// when the filter text or the board changes.
#[derive(Default)]
pub struct SearchCache {
    parts: Option<(BoardStamp, String, Option<&'static str>, Vec<usize>)>,
    nets: Option<(BoardStamp, String, Vec<String>)>,
    /// Number of times the match lists were recomputed (for tests).
    pub recomputes: usize,
}

impl SearchCache {
    /// Forgets cached matches (e.g. after a board is opened or closed).
    pub fn invalidate(&mut self) {
        self.parts = None;
        self.nets = None;
    }

    /// Indices of the parts matching `filter` and optional `prefix`, in board order.
    pub fn parts(&mut self, b: &Board, filter: &str, prefix: Option<&'static str>) -> &[usize] {
        let stamp = board_stamp(b);
        let needle = filter.trim().to_ascii_uppercase();
        let fresh = matches!(&self.parts, Some((s, n, p, _)) if *s == stamp && *n == needle && *p == prefix);
        if !fresh {
            self.recomputes += 1;
            self.parts = Some((stamp, needle.clone(), prefix, filter_part_indices(b, &needle, prefix)));
        }
        self.parts.as_ref().map(|(_, _, _, v)| v.as_slice()).unwrap_or_default()
    }

    /// Names of the nets matching `filter`, in name order.
    pub fn nets(&mut self, b: &Board, filter: &str) -> &[String] {
        let stamp = board_stamp(b);
        let needle = filter.trim().to_ascii_uppercase();
        let fresh = matches!(&self.nets, Some((s, n, _)) if *s == stamp && *n == needle);
        if !fresh {
            self.recomputes += 1;
            self.nets = Some((stamp, needle.clone(), filter_net_names(b, &needle)));
        }
        self.nets.as_ref().map(|(_, _, v)| v.as_slice()).unwrap_or_default()
    }
}

/// Matches a string against a pattern supporting wildcards (`*` and `?`).
/// If the pattern contains no wildcard characters, checks if `text` contains `pattern`.
pub fn wildcard_match(pattern: &str, text: &str) -> bool {
    if pattern.is_empty() {
        return true;
    }
    if !pattern.contains('*') && !pattern.contains('?') {
        return text.contains(pattern);
    }

    let p_bytes = pattern.as_bytes();
    let t_bytes = text.as_bytes();
    let mut p_idx = 0;
    let mut t_idx = 0;
    let mut star_idx = None;
    let mut match_idx = 0;

    while t_idx < t_bytes.len() {
        if p_idx < p_bytes.len() && (p_bytes[p_idx] == b'?' || p_bytes[p_idx] == t_bytes[t_idx]) {
            p_idx += 1;
            t_idx += 1;
        } else if p_idx < p_bytes.len() && p_bytes[p_idx] == b'*' {
            star_idx = Some(p_idx);
            p_idx += 1;
            match_idx = t_idx;
        } else if let Some(s) = star_idx {
            p_idx = s + 1;
            match_idx += 1;
            t_idx = match_idx;
        } else {
            return false;
        }
    }

    while p_idx < p_bytes.len() && p_bytes[p_idx] == b'*' {
        p_idx += 1;
    }

    p_idx == p_bytes.len()
}

/// Parts whose name matches the upper-case `needle` (with wildcard or substring)
/// and optional `prefix`.
pub fn filter_part_indices(b: &Board, needle: &str, prefix: Option<&str>) -> Vec<usize> {
    b.parts
        .iter()
        .enumerate()
        .filter(|(_, part)| {
            let name_upper = part.name.to_ascii_uppercase();
            if let Some(pfx) = prefix {
                if !name_upper.starts_with(pfx) {
                    return false;
                }
            }
            wildcard_match(needle, &name_upper)
        })
        .map(|(idx, _)| idx)
        .collect()
}

/// Nets whose name matches the upper-case `needle` (with wildcard or substring).
pub fn filter_net_names(b: &Board, needle: &str) -> Vec<String> {
    b.nets
        .keys()
        .filter(|name| {
            let name_upper = name.to_ascii_uppercase();
            wildcard_match(needle, &name_upper)
        })
        .cloned()
        .collect()
}

/// The selection-driven view of the side panel. The photo overlay view is not a
/// variant: it is shown while `SidePanelState::overlay_editing` is set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Net,
    Part,
    Search,
}

/// The detail view the side panel shows for the current selection: a pin, via or
/// net shows the net details, a part alone (or an unconnected pin) shows the part
/// details and an empty selection shows the search. `stay_on_part` keeps the part
/// details while browsing the pins of that part.
pub fn auto_view(part: bool, pin: bool, via: bool, net: bool, stay_on_part: bool) -> Tab {
    if part && stay_on_part {
        Tab::Part
    } else if net || via {
        Tab::Net
    } else if part || pin {
        Tab::Part
    } else {
        Tab::Search
    }
}

/// Selects only `part_id`, clearing any pin, via or net selection.
pub fn select_part_only(
    part_id: usize,
    selected_pin_idx: &mut Option<usize>,
    selected_via_idx: &mut Option<usize>,
    selected_part_idx: &mut Option<usize>,
    selected_net: &mut Option<String>,
) {
    *selected_part_idx = Some(part_id);
    *selected_pin_idx = None;
    *selected_via_idx = None;
    *selected_net = None;
}

/// Where the board view should move to show `part_id`: the world-space center of
/// the part (bounding box center, falling back to its first pin) and the view
/// side to display (unchanged when the part is already visible on `view_side`).
pub fn part_focus_target(board: &Board, part_id: usize, view_side: Side) -> Option<(Point, Side)> {
    let part = board.parts.get(part_id)?;
    let center = match part.bounding_box {
        Some(bbox) => bbox.center(),
        None => board.pins.get(*part.pin_indices.first()?)?.pos,
    };
    let side = if part.side.visible_on(view_side) { view_side } else { part.side };
    Some((center, side))
}

pub fn show_side_panel(
    ui: &mut Ui,
    tokens: &Tokens,
    board: Option<&Board>,
    state: &mut SidePanelState,
    notes_store: &mut bvc_core::BoardNotesStore,
    overlay_manager: &mut OverlayManager,
    alignment_session: &mut AlignmentSession,
    selected_pin_idx: &mut Option<usize>,
    selected_via_idx: &mut Option<usize>,
    selected_part_idx: &mut Option<usize>,
    selected_net: &mut Option<String>,
    current_side: Side,
    current_rotation_rad: f32,
    current_sx: f32,
    current_sy: f32,
    lang: Language,
    on_load_photo: &mut Option<Side>,
) {
    // Overlay adjustment stays on screen while a pin alignment is running, so it
    // can always be finished or cancelled.
    let calibrating = alignment_session.is_active();
    if state.overlay_editing || calibrating {
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let done = ui.add_enabled(!calibrating, Button::new(i18n::tr(lang, "overlay.done")));
                let done = if calibrating {
                    done.on_disabled_hover_text(i18n::tr(lang, "overlay.done_calibrating_hint"))
                } else {
                    done
                };
                if done.clicked() {
                    state.overlay_editing = false;
                }
            });
        });
        show_overlay_panel(
            ui,
            tokens,
            overlay_manager,
            alignment_session,
            current_side,
            current_rotation_rad,
            current_sx,
            current_sy,
            lang,
            on_load_photo,
        );
        return;
    }

    let Some(b) = board else {
        ui.label(RichText::new(i18n::tr(lang, "side_panel.no_board_loaded")).color(tokens.text_dim()));
        return;
    };

    if state.stay_on_part != *selected_part_idx {
        state.stay_on_part = None;
    }
    let view = auto_view(
        selected_part_idx.is_some(),
        selected_pin_idx.is_some(),
        selected_via_idx.is_some(),
        selected_net.is_some(),
        state.stay_on_part.is_some(),
    );

    // Detail views get a way back to the search, which clears the selection.
    if view != Tab::Search {
        if ui.button(i18n::tr(lang, "side_panel.back_to_search")).clicked() {
            *selected_part_idx = None;
            *selected_pin_idx = None;
            *selected_via_idx = None;
            *selected_net = None;
            return;
        }
        ui.separator();
    }

    match view {
        Tab::Net => {
            if let Some(net_name) = selected_net {
                ui.horizontal(|ui| {
                    ui.heading(RichText::new(format!("Net: {net_name}")).color(tokens.accent()));
                    if ui.button("📋").on_hover_text("Copy full net details (TSV) to clipboard").clicked() {
                        let text = export_net_details_tsv(b, net_name);
                        ui.ctx().copy_text(text);
                    }
                });
                if let Some(net) = b.get_net(net_name) {
                    ui.label(
                        RichText::new(format!(
                            "Connected: {} pins, {} vias",
                            net.pin_indices.len(),
                            net.via_indices.len()
                        ))
                        .color(tokens.text_dim()),
                    );

                    ui.add_space(8.0);
                    ui.label(RichText::new(i18n::tr(lang, "side_panel.pins_on_net")).strong());

                    let row_height = 20.0;
                    ScrollArea::vertical()
                        .id_salt("net_pins_scroll")
                        .max_height(280.0)
                        .show_rows(ui, row_height, net.pin_indices.len(), |ui, row_range| {
                            for row_idx in row_range {
                                let pin_idx = net.pin_indices[row_idx];
                                if let Some(pin) = b.pins.get(pin_idx) {
                                    let part_name = b
                                        .parts
                                        .get(pin.part_id)
                                        .map(|p| p.name.as_str())
                                        .unwrap_or("?");

                                    let side_str = match pin.side {
                                        Side::Top => "Top",
                                        Side::Bottom => "Bottom",
                                        Side::Both => "Both",
                                    };

                                    let is_cur_side = pin.side.visible_on(current_side);
                                    let label_text = format!(
                                        "{}.{} ({}) [{:.1}, {:.1}]",
                                        part_name, pin.name, side_str, pin.pos.x, pin.pos.y
                                    );

                                    let color = if is_cur_side {
                                        tokens.pin_highlight_cur_side()
                                    } else {
                                        tokens.pin_highlight_opp_side()
                                    };

                                    let is_selected = *selected_pin_idx == Some(pin_idx);
                                    let response = ui.selectable_label(
                                        is_selected,
                                        RichText::new(&label_text).color(color),
                                    );
                                    if response.clicked() {
                                        *selected_pin_idx = Some(pin_idx);
                                        *selected_via_idx = None;
                                        *selected_part_idx = Some(pin.part_id);
                                    }
                                    response.context_menu(|ui| {
                                        if ui.button("Copy Pin Details").clicked() {
                                            ui.ctx().copy_text(label_text.clone());
                                            ui.close();
                                        }
                                        if ui.button("Copy Coordinates").clicked() {
                                            ui.ctx().copy_text(format!("{:.3}, {:.3}", pin.pos.x, pin.pos.y));
                                            ui.close();
                                        }
                                    });
                                }
                            }
                        });

                    if !net.via_indices.is_empty() {
                        ui.add_space(8.0);
                        ui.label(RichText::new(i18n::tr(lang, "side_panel.vias_on_net")).strong());
                        let via_row_height = 18.0;
                        ScrollArea::vertical()
                            .id_salt("net_vias_scroll")
                            .max_height(150.0)
                            .show_rows(ui, via_row_height, net.via_indices.len(), |ui, row_range| {
                                for row_idx in row_range {
                                    let via_idx = net.via_indices[row_idx];
                                    if let Some(via) = b.vias.get(via_idx) {
                                        let side_str = match via.side {
                                            Side::Top => "Top",
                                            Side::Bottom => "Bottom",
                                            Side::Both => "Both",
                                        };
                                        ui.label(
                                            RichText::new(format!(
                                                "Via #{} ({}) [{:.1}, {:.1}]",
                                                via.id, side_str, via.pos.x, via.pos.y
                                            ))
                                            .color(tokens.text_dim()),
                                        );
                                    }
                                }
                            });
                    }
                }
            } else {
                ui.label(RichText::new(i18n::tr(lang, "side_panel.click_pin_view_net")).color(tokens.text_dim()));
            }
        }
        Tab::Part => {
            if let Some(part_id) = selected_part_idx {
                if let Some(part) = b.parts.get(*part_id) {
                    ui.horizontal(|ui| {
                        ui.heading(RichText::new(format!("Part: {}", part.name)).color(tokens.accent()));
                        if ui.button("📋").on_hover_text("Copy full part details (TSV) to clipboard").clicked() {
                            let text = export_part_details_tsv(b, part);
                            ui.ctx().copy_text(text);
                        }
                    });
                    let side_str = match part.side {
                        Side::Top => "Top",
                        Side::Bottom => "Bottom",
                        Side::Both => "Both",
                    };
                    ui.label(
                        RichText::new(format!("Side: {side_str} | Pins: {}", part.pin_indices.len()))
                            .color(tokens.text_dim()),
                    );

                    ui.add_space(8.0);
                    ui.label(RichText::new(i18n::tr(lang, "side_panel.pins")).strong());

                    let row_height = 20.0;
                    ScrollArea::vertical()
                        .id_salt("part_pins_scroll")
                        .max_height(220.0)
                        .show_rows(ui, row_height, part.pin_indices.len(), |ui, row_range| {
                            for row_idx in row_range {
                                let pin_idx = part.pin_indices[row_idx];
                                if let Some(pin) = b.pins.get(pin_idx) {
                                    let is_selected = *selected_pin_idx == Some(pin_idx);
                                    let has_note = notes_store.notes.iter().any(|n| {
                                        n.target.component_name == part.name && n.target.pin_name.as_deref() == Some(&pin.name)
                                    });
                                    let note_badge = if has_note { " 📝" } else { "" };
                                    let pin_label = format!(
                                        "Pin {}: Net '{}'{}",
                                        pin.name,
                                        if pin.net_name.is_empty() {
                                            "UNCONNECTED"
                                        } else {
                                            &pin.net_name
                                        },
                                        note_badge
                                    );

                                    if is_selected {
                                        *selected_pin_idx = Some(pin_idx);
                                        *selected_net = if pin.net_name.is_empty() {
                                            None
                                        } else {
                                            Some(pin.net_name.clone())
                                        };
                                    }

                                    let response = ui.selectable_label(
                                        is_selected,
                                        RichText::new(&pin_label).color(tokens.text()),
                                    );
                                    if response.clicked() {
                                        *selected_pin_idx = Some(pin_idx);
                                        *selected_via_idx = None;
                                        state.stay_on_part = *selected_part_idx;
                                        *selected_net = if pin.net_name.is_empty() {
                                            None
                                        } else {
                                            Some(pin.net_name.clone())
                                        };
                                    }
                                    response.context_menu(|ui| {
                                        if ui.button("Copy Pin Label").clicked() {
                                            ui.ctx().copy_text(pin_label.clone());
                                            ui.close();
                                        }
                                        if !pin.net_name.is_empty() {
                                            if ui.button("Copy Net Name").clicked() {
                                                ui.ctx().copy_text(pin.net_name.clone());
                                                ui.close();
                                            }
                                        }
                                    });
                                }
                            }
                        });

                    // Technician Notes & Reference Measurements Section
                    let pin_target = (*selected_pin_idx)
                        .and_then(|idx| b.pins.get(idx))
                        .map(|pin| bvc_core::NoteTarget::pin(&part.name, &pin.name));
                    let comp_target = bvc_core::NoteTarget::component(&part.name);

                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(4.0);

                    // If a pin is selected, allow toggling between editing the Pin's note or the Component's note
                    let current_target = if let Some(pt) = pin_target {
                        let is_pin_target = state.note_target_is_pin.unwrap_or(true);
                        ui.horizontal(|ui| {
                            let pin_btn = ui.selectable_label(is_pin_target, format!("📌 {}", pt.display_label()));
                            if pin_btn.clicked() {
                                state.note_target_is_pin = Some(true);
                            }
                            let comp_btn = ui.selectable_label(!is_pin_target, format!("📦 {}", comp_target.display_label()));
                            if comp_btn.clicked() {
                                state.note_target_is_pin = Some(false);
                            }
                        });
                        if is_pin_target { pt } else { comp_target }
                    } else {
                        state.note_target_is_pin = Some(false);
                        comp_target
                    };

                    let target_label = current_target.display_label();
                    ui.heading(RichText::new(format!("📝 {}: {target_label}", i18n::tr(lang, "notes.title"))).color(tokens.accent()).small());

                    if state.note_active_target.as_ref() != Some(&current_target) {
                        state.note_active_target = Some(current_target.clone());
                        if let Some(existing) = notes_store.notes.iter().find(|n| n.target == current_target) {
                            state.note_edit_text = existing.text.clone();
                            state.note_edit_voltage = existing.measurements.voltage.clone().unwrap_or_default();
                            state.note_edit_resistance = existing.measurements.resistance.clone().unwrap_or_default();
                            state.note_edit_diode = existing.measurements.diode_drop.clone().unwrap_or_default();
                        } else {
                            state.note_edit_text.clear();
                            state.note_edit_voltage.clear();
                            state.note_edit_resistance.clear();
                            state.note_edit_diode.clear();
                        }
                        state.note_dirty = false;
                    }

                    ui.horizontal(|ui| {
                        ui.label(i18n::tr(lang, "notes.voltage"));
                        if ui.add(TextEdit::singleline(&mut state.note_edit_voltage).desired_width(70.0).hint_text("e.g. 3.3V")).changed() {
                            state.note_dirty = true;
                        }
                        ui.label(i18n::tr(lang, "notes.resistance"));
                        if ui.add(TextEdit::singleline(&mut state.note_edit_resistance).desired_width(70.0).hint_text("e.g. 4.7k")).changed() {
                            state.note_dirty = true;
                        }
                    });

                    ui.horizontal(|ui| {
                        ui.label(i18n::tr(lang, "notes.diode_drop"));
                        if ui.add(TextEdit::singleline(&mut state.note_edit_diode).desired_width(70.0).hint_text("e.g. 0.45V")).changed() {
                            state.note_dirty = true;
                        }
                    });

                    ui.add_space(4.0);
                    ui.label(i18n::tr(lang, "notes.observation"));
                    if ui.add(TextEdit::multiline(&mut state.note_edit_text).desired_rows(2).desired_width(ui.available_width())).changed() {
                        state.note_dirty = true;
                    }

                    ui.horizontal(|ui| {
                        let save_btn = format!("💾 {}", i18n::tr(lang, "notes.save"));
                        if ui.button(save_btn).clicked() || state.note_dirty {
                            let measurements = bvc_core::TechnicalMeasurements {
                                voltage: (!state.note_edit_voltage.trim().is_empty()).then(|| state.note_edit_voltage.trim().to_string()),
                                resistance: (!state.note_edit_resistance.trim().is_empty()).then(|| state.note_edit_resistance.trim().to_string()),
                                diode_drop: (!state.note_edit_diode.trim().is_empty()).then(|| state.note_edit_diode.trim().to_string()),
                                other: None,
                            };
                            if state.note_edit_text.trim().is_empty() && measurements.is_empty() {
                                notes_store.remove(&current_target);
                            } else {
                                notes_store.upsert(current_target.clone(), state.note_edit_text.trim().to_string(), measurements);
                            }
                            state.note_dirty = false;
                        }

                        if notes_store.notes.iter().any(|n| n.target == current_target) {
                            let del_btn = format!("🗑 {}", i18n::tr(lang, "notes.delete"));
                            if ui.button(del_btn).clicked() {
                                notes_store.remove(&current_target);
                                state.note_edit_text.clear();
                                state.note_edit_voltage.clear();
                                state.note_edit_resistance.clear();
                                state.note_edit_diode.clear();
                                state.note_dirty = false;
                            }
                        }
                    });
                }
            } else {
                ui.label(RichText::new(i18n::tr(lang, "side_panel.click_pin_view_part")).color(tokens.text_dim()));
            }
        }
        Tab::Search => {
            // Sub-tabs: Components, Nets, Notes
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(4.0, 0.0);
                let parts_active = state.search_tab == SearchSubTab::Parts;
                let nets_active = state.search_tab == SearchSubTab::Nets;
                let notes_active = state.search_tab == SearchSubTab::Notes;

                let notes_count = notes_store.notes.len();
                let notes_label = if notes_count > 0 {
                    format!("📝 {} ({})", i18n::tr(lang, "side_panel.tab_notes"), notes_count)
                } else {
                    format!("📝 {}", i18n::tr(lang, "side_panel.tab_notes"))
                };

                if ui.selectable_label(parts_active, format!("🔍 {}", i18n::tr(lang, "side_panel.tab_parts"))).clicked() {
                    state.search_tab = SearchSubTab::Parts;
                }
                if ui.selectable_label(nets_active, format!("🌐 {}", i18n::tr(lang, "side_panel.tab_nets"))).clicked() {
                    state.search_tab = SearchSubTab::Nets;
                }
                if ui.selectable_label(notes_active, notes_label).clicked() {
                    state.search_tab = SearchSubTab::Notes;
                }
            });
            ui.separator();

            match state.search_tab {
                SearchSubTab::Parts => {
                    ui.label(RichText::new(i18n::tr(lang, "side_panel.search_parts")).strong());
                    let part_input = ui.add(
                        TextEdit::singleline(&mut state.filter_part)
                            .hint_text(i18n::tr(lang, "side_panel.search_parts_hint")),
                    );
                    part_input.context_menu(|ui| {
                        if ui.button(i18n::tr(lang, "context_menu.paste")).clicked() {
                            if let Some(text) = get_clipboard_text() {
                                let text = text.replace("\r\n", "\n").replace('\n', " ");
                                state.filter_part.push_str(text.trim());
                            } else {
                                ui.ctx().send_viewport_cmd(egui::ViewportCommand::RequestPaste);
                            }
                            ui.close();
                        }
                        if !state.filter_part.is_empty() && ui.button(i18n::tr(lang, "context_menu.clear")).clicked() {
                            state.filter_part.clear();
                            ui.close();
                        }
                    });

                    // Quick prefix filter chips: All, U, R, C, Q, D, L, F, J
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing = egui::vec2(3.0, 3.0);
                        let all_selected = state.selected_prefix.is_none();
                        if ui.selectable_label(all_selected, "All").clicked() {
                            state.selected_prefix = None;
                        }
                        const PREFIXES: [&str; 8] = ["U", "R", "C", "Q", "D", "L", "F", "J"];
                        for &pfx in &PREFIXES {
                            let is_active = state.selected_prefix == Some(pfx);
                            if ui.selectable_label(is_active, pfx).clicked() {
                                state.selected_prefix = if is_active { None } else { Some(pfx) };
                            }
                        }
                    });

                    let row_height = ui.spacing().interact_size.y;
                    let part_matches = state.search.parts(b, &state.filter_part, state.selected_prefix);
                    let mut clicked_part = None;
                    ScrollArea::vertical()
                        .id_salt("search_parts_scroll")
                        .max_height(340.0)
                        .show_rows(ui, row_height, part_matches.len(), |ui, row_range| {
                            for &idx in &part_matches[row_range] {
                                let Some(part) = b.parts.get(idx) else { continue };
                                let is_selected = *selected_part_idx == Some(part.id);
                                let has_note = notes_store.notes.iter().any(|n| n.target.component_name == part.name);
                                let note_badge = if has_note { " 📝" } else { "" };
                                let response = ui.selectable_label(
                                    is_selected,
                                    format!("{} ({:?}){}", part.name, part.side, note_badge),
                                );
                                if response.clicked() {
                                    clicked_part = Some(part.id);
                                }
                                response.context_menu(|ui| {
                                    if ui.button(i18n::tr(lang, "context_menu.copy_part_name")).clicked() {
                                        ui.ctx().copy_text(part.name.clone());
                                        ui.close();
                                    }
                                    if ui.button(i18n::tr(lang, "context_menu.copy_part_details")).clicked() {
                                        let tsv = export_part_details_tsv(b, part);
                                        ui.ctx().copy_text(tsv);
                                        ui.close();
                                    }
                                });
                            }
                        });
                    if let Some(part_id) = clicked_part {
                        select_part_only(part_id, selected_pin_idx, selected_via_idx, selected_part_idx, selected_net);
                        state.focus_part = Some(part_id);
                    }
                }
                SearchSubTab::Nets => {
                    ui.label(RichText::new(i18n::tr(lang, "side_panel.search_nets")).strong());
                    let net_input = ui.add(
                        TextEdit::singleline(&mut state.filter_net)
                            .hint_text(i18n::tr(lang, "side_panel.search_nets_hint")),
                    );
                    net_input.context_menu(|ui| {
                        if ui.button(i18n::tr(lang, "context_menu.paste")).clicked() {
                            if let Some(text) = get_clipboard_text() {
                                let text = text.replace("\r\n", "\n").replace('\n', " ");
                                state.filter_net.push_str(text.trim());
                            } else {
                                ui.ctx().send_viewport_cmd(egui::ViewportCommand::RequestPaste);
                            }
                            ui.close();
                        }
                        if !state.filter_net.is_empty() && ui.button(i18n::tr(lang, "context_menu.clear")).clicked() {
                            state.filter_net.clear();
                            ui.close();
                        }
                    });

                    let row_height = ui.spacing().interact_size.y;
                    let net_matches = state.search.nets(b, &state.filter_net);
                    ScrollArea::vertical()
                        .id_salt("search_nets_scroll")
                        .max_height(340.0)
                        .show_rows(ui, row_height, net_matches.len(), |ui, row_range| {
                            for name in &net_matches[row_range] {
                                let Some(net) = b.nets.get(name) else { continue };
                                let is_selected = selected_net.as_deref() == Some(name.as_str());
                                let response = ui.selectable_label(
                                    is_selected,
                                    format!("{} ({} pins)", name, net.pin_indices.len()),
                                );
                                if response.clicked() {
                                    *selected_net = Some(name.clone());
                                    *selected_pin_idx = net.pin_indices.first().copied();
                                    if let Some(p_idx) = *selected_pin_idx {
                                        *selected_part_idx = b.pins.get(p_idx).map(|p| p.part_id);
                                    }
                                }
                                response.context_menu(|ui| {
                                    if ui.button(i18n::tr(lang, "context_menu.copy_net")).clicked() {
                                        ui.ctx().copy_text(name.clone());
                                        ui.close();
                                    }
                                    if ui.button(i18n::tr(lang, "context_menu.copy_net_details")).clicked() {
                                        let tsv = export_net_details_tsv(b, name);
                                        ui.ctx().copy_text(tsv);
                                        ui.close();
                                    }
                                });
                            }
                        });
                }
                SearchSubTab::Notes => {
                    ui.label(RichText::new(i18n::tr(lang, "side_panel.notes_list")).strong());
                    ui.add(
                        TextEdit::singleline(&mut state.filter_notes)
                            .hint_text(i18n::tr(lang, "side_panel.filter_notes_hint")),
                    );

                    let filter_lower = state.filter_notes.trim().to_ascii_lowercase();
                    let matching_notes: Vec<_> = notes_store
                        .notes
                        .iter()
                        .filter(|note| {
                            if filter_lower.is_empty() {
                                return true;
                            }
                            let label = note.target.display_label().to_ascii_lowercase();
                            let text = note.text.to_ascii_lowercase();
                            label.contains(&filter_lower) || text.contains(&filter_lower)
                        })
                        .collect();

                    if matching_notes.is_empty() {
                        ui.add_space(8.0);
                        ui.label(RichText::new(i18n::tr(lang, "side_panel.no_notes")).color(tokens.text_dim()).italics());
                    } else {
                        let mut target_to_select = None;
                        ScrollArea::vertical()
                            .id_salt("search_notes_scroll")
                            .max_height(340.0)
                            .show(ui, |ui| {
                                for note in matching_notes {
                                    let target_label = note.target.display_label();
                                    let icon = if note.target.pin_name.is_some() { "📌" } else { "📦" };
                                    let mut summary_parts = Vec::new();
                                    if let Some(ref v) = note.measurements.voltage {
                                        summary_parts.push(v.as_str());
                                    }
                                    if let Some(ref r) = note.measurements.resistance {
                                        summary_parts.push(r.as_str());
                                    }
                                    if let Some(ref d) = note.measurements.diode_drop {
                                        summary_parts.push(d.as_str());
                                    }
                                    let meas_str = if summary_parts.is_empty() {
                                        String::new()
                                    } else {
                                        format!(" [{}]", summary_parts.join(", "))
                                    };

                                    let header = format!("{icon} {target_label}{meas_str}");
                                    let is_active = state.note_active_target.as_ref() == Some(&note.target);
                                    let resp = ui.selectable_label(is_active, RichText::new(header).strong());
                                    if !note.text.is_empty() {
                                        ui.label(RichText::new(&note.text).color(tokens.text_dim()).small());
                                    }
                                    ui.add_space(2.0);

                                    if resp.clicked() {
                                        target_to_select = Some(note.target.clone());
                                    }
                                }
                            });

                        if let Some(target) = target_to_select {
                            if let Some(part) = b.parts.iter().find(|p| p.name == target.component_name) {
                                let part_id = part.id;
                                if let Some(ref pin_name) = target.pin_name {
                                    let pin_opt = part.pin_indices.iter().find_map(|&pidx| {
                                        b.pins.get(pidx).filter(|pin| &pin.name == pin_name).map(|pin| (pidx, pin))
                                    });
                                    if let Some((pidx, pin)) = pin_opt {
                                        *selected_pin_idx = Some(pidx);
                                        *selected_via_idx = None;
                                        *selected_part_idx = Some(part_id);
                                        *selected_net = if pin.net_name.is_empty() {
                                            None
                                        } else {
                                            Some(pin.net_name.clone())
                                        };
                                        state.stay_on_part = Some(part_id);
                                        state.note_target_is_pin = Some(true);
                                    } else {
                                        select_part_only(part_id, selected_pin_idx, selected_via_idx, selected_part_idx, selected_net);
                                        state.note_target_is_pin = Some(false);
                                    }
                                } else {
                                    select_part_only(part_id, selected_pin_idx, selected_via_idx, selected_part_idx, selected_net);
                                    state.note_target_is_pin = Some(false);
                                }
                                state.focus_part = Some(part_id);
                            }
                        }
                    }
                }
            }
        }
    }
}

pub fn show_overlay_panel(
    ui: &mut Ui,
    tokens: &Tokens,
    overlay_manager: &mut OverlayManager,
    alignment_session: &mut AlignmentSession,
    current_side: Side,
    current_rotation_rad: f32,
    current_sx: f32,
    current_sy: f32,
    lang: Language,
    on_load_photo: &mut Option<Side>,
) {
    let side_name = match current_side {
        Side::Top => i18n::tr(lang, "overlay.side_top"),
        Side::Bottom => i18n::tr(lang, "overlay.side_bottom"),
        Side::Both => i18n::tr(lang, "overlay.side_both"),
    };

    ui.heading(RichText::new(format!("{}: {}", i18n::tr(lang, "overlay.title"), side_name)).color(tokens.accent()));
    ui.add_space(4.0);

    let slot = overlay_manager.slot_for_side(current_side);

    if !slot.is_loaded() {
        ui.label(RichText::new(i18n::tr(lang, "overlay.no_photo_loaded")).color(tokens.text_dim()));
        ui.add_space(8.0);
        if ui.button(format!("📷 {}", i18n::tr(lang, "overlay.load_photo"))).clicked() {
            *on_load_photo = Some(current_side);
        }
        return;
    }

    if let Some(path) = &slot.file_path {
        let file_name = path.file_name().and_then(|f| f.to_str()).unwrap_or("photo");
        ui.label(RichText::new(format!("{}: {}", i18n::tr(lang, "overlay.file"), file_name)).color(tokens.text_dim()));
        ui.label(RichText::new(format!("{}x{} px", slot.orig_dimensions.0, slot.orig_dimensions.1)).color(tokens.text_dim()));
    }

    ui.add_space(8.0);

    // Pin Alignment Calibration Section
    if !alignment_session.is_active() {
        if ui.button(format!("🎯 {}", i18n::tr(lang, "overlay.align_pins"))).clicked() {
            let snap = slot.snapshot();
            alignment_session.start(snap, current_side);
        }
    } else {
        ui.group(|ui| {
            let pairs_count = alignment_session.pairs.len();
            ui.horizontal(|ui| {
                ui.heading(
                    RichText::new(format!(
                        "🎯 {} ({}/{})",
                        i18n::tr(lang, "overlay.align_active_title"),
                        pairs_count,
                        MAX_PAIRS
                    ))
                    .color(tokens.accent())
                    .strong(),
                );
            });
            ui.add_space(4.0);

            // Table of pin pairs
            let mut repick_idx = None;
            let mut delete_idx = None;

            ScrollArea::vertical()
                .max_height(140.0)
                .show(ui, |ui| {
                    egui::Grid::new("pin_alignment_table")
                        .striped(true)
                        .spacing([6.0, 4.0])
                        .show(ui, |ui| {
                            ui.label(RichText::new(i18n::tr(lang, "overlay.align_col_num")).strong());
                            ui.label(RichText::new(i18n::tr(lang, "overlay.align_col_pin")).strong());
                            ui.label(RichText::new(i18n::tr(lang, "overlay.align_col_residual")).strong());
                            ui.label("");
                            ui.end_row();

                            for (i, pair) in alignment_session.pairs.iter().enumerate() {
                                ui.label(format!("{}", i + 1));
                                ui.label(format!("{}.{}", pair.part_name, pair.pin_name));

                                if let Some(Ok(ref res)) = alignment_session.live_result {
                                    if let Some(res_val) = res.residuals_mils.get(i) {
                                        let col = if *res_val < 2.0 {
                                            Color32::from_rgb(0x4c, 0xaf, 0x50)
                                        } else if *res_val <= 5.0 {
                                            Color32::from_rgb(0xff, 0xeb, 0x3b)
                                        } else {
                                            Color32::from_rgb(0xf4, 0x43, 0x36)
                                        };
                                        ui.label(RichText::new(format!("{:.1} mil", res_val)).color(col));
                                    } else {
                                        ui.label("-");
                                    }
                                } else {
                                    ui.label(if pair.photo_pos.is_some() { "OK" } else { "..." });
                                }

                                ui.horizontal(|ui| {
                                    if ui.small_button(i18n::tr(lang, "overlay.align_repick")).clicked() {
                                        repick_idx = Some(i);
                                    }
                                    if ui.small_button(RichText::new(i18n::tr(lang, "overlay.align_delete")).color(Color32::from_rgb(0xd0, 0x50, 0x50))).clicked() {
                                        delete_idx = Some(i);
                                    }
                                });
                                ui.end_row();
                            }
                        });
                });

            if let Some(idx) = repick_idx {
                alignment_session.repick_pair(idx);
            }
            if let Some(idx) = delete_idx {
                alignment_session.delete_pair(idx);
            }

            ui.add_space(4.0);

            // RMSE metric badge
            if let Some(Ok(ref res)) = alignment_session.live_result {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(i18n::tr(lang, "overlay.align_rmse")).strong());
                    ui.label(RichText::new(format!("{:.2} mils", res.rmse_mils)).strong().color(tokens.accent()));
                });
            } else if pairs_count < 2 {
                ui.label(RichText::new(i18n::tr(lang, "overlay.align_status_min_points")).color(tokens.text_dim()).small());
            }

            // Add pin pair button (if < 5)
            if pairs_count < MAX_PAIRS {
                if ui.button(format!("➕ {}", i18n::tr(lang, "overlay.align_add_pair"))).clicked() {
                    alignment_session.add_pair();
                }
            }

            // Auto-lock toggle
            ui.checkbox(&mut alignment_session.auto_lock_on_apply, i18n::tr(lang, "overlay.align_auto_lock"));

            ui.add_space(4.0);

            // Action row: Apply | Reset | Cancel
            ui.horizontal(|ui| {
                let can_apply = alignment_session.live_result.as_ref().map(|r| r.is_ok()).unwrap_or(false);
                let apply_btn = Button::new(RichText::new(i18n::tr(lang, "overlay.align_apply")).color(Color32::BLACK))
                    .fill(Color32::from_rgb(0x4c, 0xaf, 0x50));

                if ui.add_enabled(can_apply, apply_btn).clicked() {
                    if let Some(Ok(ref res)) = alignment_session.live_result {
                        slot.apply_alignment_result(
                            res,
                            alignment_session.auto_lock_on_apply,
                            current_rotation_rad,
                            current_sx,
                            current_sy,
                        );
                        alignment_session.cancel(); // Clears session markers and resets to Idle
                    }
                }

                if ui.button(i18n::tr(lang, "overlay.align_reset")).clicked() {
                    if let Some(snap) = alignment_session.snapshot {
                        slot.restore_snapshot(snap);
                        alignment_session.pairs.clear();
                        alignment_session.live_result = None;
                        alignment_session.state = crate::overlay_align::AlignmentState::SelectingCadPin { pair_index: 0 };
                    }
                }

                if ui.button(i18n::tr(lang, "overlay.align_cancel")).clicked() {
                    if let Some(snap) = alignment_session.cancel() {
                        slot.restore_snapshot(snap);
                    }
                }
            });
        });
    }

    ui.add_space(8.0);
    ui.horizontal(|ui| {
        let lock_icon = if slot.locked { "🔒" } else { "🔓" };
        let lock_label = format!("{} {}", lock_icon, if slot.locked { i18n::tr(lang, "overlay.locked") } else { i18n::tr(lang, "overlay.unlocked") });
        if ui.selectable_label(slot.locked, lock_label).clicked() {
            slot.toggle_lock(current_rotation_rad, current_sx, current_sy);
        }

        ui.checkbox(&mut slot.visible, i18n::tr(lang, "overlay.visible"));
    });

    ui.add_space(8.0);
    ui.separator();
    ui.add_space(4.0);

    // Opacity
    ui.horizontal(|ui| {
        ui.label(i18n::tr(lang, "overlay.opacity"));
        ui.add(Slider::new(&mut slot.opacity, 0.0..=1.0).show_value(true).custom_formatter(|n, _| format!("{:.0}%", n * 100.0)));
    });

    ui.add_space(6.0);

    // Position X / Y
    ui.label(RichText::new(i18n::tr(lang, "overlay.position")).strong());
    ui.horizontal(|ui| {
        ui.label("X (mil):");
        ui.add_enabled(!slot.locked, DragValue::new(&mut slot.center.x).speed(1.0));
        ui.label("Y (mil):");
        ui.add_enabled(!slot.locked, DragValue::new(&mut slot.center.y).speed(1.0));
    });

    ui.add_space(6.0);

    // Size / Scaling
    ui.label(RichText::new(i18n::tr(lang, "overlay.size")).strong());

    // Zoom slider (% scaling)
    ui.horizontal(|ui| {
        ui.label(format!("{}:", i18n::tr(lang, "overlay.zoom")));
        let mut zoom_pct = slot.zoom * 100.0;
        if ui
            .add_enabled(
                !slot.locked,
                Slider::new(&mut zoom_pct, 10.0..=500.0)
                    .suffix("%")
                    .logarithmic(true)
                    .show_value(true),
            )
            .changed()
        {
            slot.set_zoom(zoom_pct / 100.0);
        }
        if ui.add_enabled(!slot.locked, Button::new("100%")).clicked() {
            slot.set_zoom(1.0);
        }
    });

    ui.horizontal(|ui| {
        let aspect = if slot.orig_dimensions.1 > 0 {
            slot.orig_dimensions.0 as f32 / slot.orig_dimensions.1 as f32
        } else {
            1.0
        };

        ui.label("W:");
        let mut w = slot.size.x;
        if ui.add_enabled(!slot.locked, DragValue::new(&mut w).speed(2.0).range(10.0..=1_000_000.0)).changed() {
            slot.size.x = w;
            if slot.aspect_locked {
                slot.size.y = w / aspect.max(0.001);
            }
            if slot.base_size.x > 0.0 {
                slot.zoom = slot.size.x / slot.base_size.x;
            }
        }

        ui.label("H:");
        let mut h = slot.size.y;
        if ui.add_enabled(!slot.locked, DragValue::new(&mut h).speed(2.0).range(10.0..=1_000_000.0)).changed() {
            slot.size.y = h;
            if slot.aspect_locked {
                slot.size.x = h * aspect;
            }
            if slot.base_size.y > 0.0 {
                slot.zoom = slot.size.y / slot.base_size.y;
            }
        }
    });

    ui.checkbox(&mut slot.aspect_locked, i18n::tr(lang, "overlay.aspect_locked"));

    ui.add_space(6.0);

    // Mirror horizontal & vertical
    ui.label(RichText::new(i18n::tr(lang, "overlay.mirror_title")).strong());
    ui.horizontal(|ui| {
        let mx_label = format!("{} [{}]", i18n::tr(lang, "overlay.mirror_x"), if slot.mirror_x { "ON" } else { "OFF" });
        if ui.add_enabled(!slot.locked, Button::new(mx_label)).clicked() {
            slot.mirror_x = !slot.mirror_x;
        }
        let my_label = format!("{} [{}]", i18n::tr(lang, "overlay.mirror_y"), if slot.mirror_y { "ON" } else { "OFF" });
        if ui.add_enabled(!slot.locked, Button::new(my_label)).clicked() {
            slot.mirror_y = !slot.mirror_y;
        }
    });

    ui.add_space(6.0);

    // Fine rotation
    ui.label(RichText::new(i18n::tr(lang, "overlay.fine_rotation")).strong());
    ui.horizontal(|ui| {
        ui.add_enabled(
            !slot.locked,
            Slider::new(&mut slot.fine_rotation_deg, -180.0..=180.0)
                .suffix("°")
                .show_value(true),
        );
        if ui.add_enabled(!slot.locked, Button::new(i18n::tr(lang, "overlay.reset_rot"))).clicked() {
            slot.fine_rotation_deg = 0.0;
        }
    });

    ui.add_space(10.0);
    ui.separator();
    ui.add_space(6.0);

    ui.horizontal(|ui| {
        if ui.button(format!("🔄 {}", i18n::tr(lang, "overlay.replace"))).clicked() {
            *on_load_photo = Some(current_side);
        }

        if ui.button(RichText::new(format!("🗑 {}", i18n::tr(lang, "overlay.remove"))).color(Color32::from_rgb(0xd0, 0x50, 0x50))).clicked() {
            slot.clear();
        }
    });

    ui.add_space(8.0);
    ui.label(RichText::new(i18n::tr(lang, "overlay.hint_drag")).color(tokens.text_dim()).small());
}

pub fn export_net_details_tsv(b: &Board, net_name: &str) -> String {
    let mut out = String::new();
    out.push_str(&format!("Net:\t{}\n", net_name));
    if let Some(net) = b.get_net(net_name) {
        out.push_str(&format!("Pins:\t{}\tVias:\t{}\n\n", net.pin_indices.len(), net.via_indices.len()));
        out.push_str("Pins on Net:\n");
        out.push_str("Part\tPin\tSide\tX (mil)\tY (mil)\n");
        for &pin_idx in &net.pin_indices {
            if let Some(pin) = b.pins.get(pin_idx) {
                let part_name = b.parts.get(pin.part_id).map(|p| p.name.as_str()).unwrap_or("?");
                let side_str = match pin.side {
                    Side::Top => "Top",
                    Side::Bottom => "Bottom",
                    Side::Both => "Both",
                };
                out.push_str(&format!("{}\t{}\t{}\t{:.2}\t{:.2}\n", part_name, pin.name, side_str, pin.pos.x, pin.pos.y));
            }
        }

        if !net.via_indices.is_empty() {
            out.push_str("\nVias on Net:\n");
            out.push_str("Via ID\tSide\tX (mil)\tY (mil)\n");
            for &via_idx in &net.via_indices {
                if let Some(via) = b.vias.get(via_idx) {
                    let side_str = match via.side {
                        Side::Top => "Top",
                        Side::Bottom => "Bottom",
                        Side::Both => "Both",
                    };
                    out.push_str(&format!("{}\t{}\t{:.2}\t{:.2}\n", via.id, side_str, via.pos.x, via.pos.y));
                }
            }
        }
    }
    out
}

pub fn export_part_details_tsv(b: &Board, part: &bvc_core::Part) -> String {
    let mut out = String::new();
    let side_str = match part.side {
        Side::Top => "Top",
        Side::Bottom => "Bottom",
        Side::Both => "Both",
    };
    out.push_str(&format!("Part:\t{}\nSide:\t{}\nPins:\t{}\n\n", part.name, side_str, part.pin_indices.len()));
    out.push_str("Pins:\n");
    out.push_str("Pin\tNet\tSide\tX (mil)\tY (mil)\n");
    for &pin_idx in &part.pin_indices {
        if let Some(pin) = b.pins.get(pin_idx) {
            let net = if pin.net_name.is_empty() { "UNCONNECTED" } else { &pin.net_name };
            let pin_side = match pin.side {
                Side::Top => "Top",
                Side::Bottom => "Bottom",
                Side::Both => "Both",
            };
            out.push_str(&format!("{}\t{}\t{}\t{:.2}\t{:.2}\n", pin.name, net, pin_side, pin.pos.x, pin.pos.y));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use bvc_core::{Part, PartType, Pin, Point};

    fn search_board() -> Board {
        let mut b = Board::new();
        for name in ["U1", "R10", "C3", "R2"] {
            b.parts.push(Part { name: name.into(), ..Default::default() });
        }
        for (i, net) in ["GND", "VCC", "PCIE_RX0"].into_iter().enumerate() {
            b.pins.push(Pin { id: i, part_id: 0, net_name: net.into(), ..Default::default() });
        }
        b.rebuild_indices();
        b
    }

    #[test]
    fn search_filters_parts_and_nets_case_insensitively() {
        let b = search_board();
        assert_eq!(filter_part_indices(&b, "", None), vec![0, 1, 2, 3]);
        assert_eq!(filter_part_indices(&b, "R", None), vec![1, 3]);
        assert_eq!(filter_part_indices(&b, "", Some("R")), vec![1, 3]);
        assert_eq!(filter_part_indices(&b, "10", Some("R")), vec![1]);
        assert_eq!(filter_part_indices(&b, "R*0", None), vec![1]);
        assert_eq!(filter_part_indices(&b, "*2", None), vec![3]);
        assert_eq!(filter_part_indices(&b, "R?", None), vec![3]);
        assert_eq!(filter_net_names(&b, ""), vec!["GND", "PCIE_RX0", "VCC"]);
        assert_eq!(filter_net_names(&b, "C"), vec!["PCIE_RX0", "VCC"]);
        assert_eq!(filter_net_names(&b, "PCIE*"), vec!["PCIE_RX0"]);
        assert_eq!(filter_net_names(&b, "*RX0"), vec!["PCIE_RX0"]);
        assert_eq!(filter_net_names(&b, "V?C"), vec!["VCC"]);
    }

    /// The match lists are recomputed only when the filter text, prefix or the board changes,
    /// not on every frame.
    #[test]
    fn search_cache_recomputes_only_on_change() {
        let b = search_board();
        let mut cache = SearchCache::default();
        assert_eq!(cache.parts(&b, "r", None), &[1, 3]);
        assert_eq!(cache.nets(&b, ""), &["GND", "PCIE_RX0", "VCC"]);
        assert_eq!(cache.recomputes, 2);
        for _ in 0..10 {
            cache.parts(&b, " r ", None);
            cache.nets(&b, "");
        }
        assert_eq!(cache.recomputes, 2, "idle frames reuse the cached matches");
        assert_eq!(cache.parts(&b, "r", Some("R")), &[1, 3]);
        assert_eq!(cache.recomputes, 3);
        assert_eq!(cache.parts(&b, "u", None), &[0]);
        assert_eq!(cache.recomputes, 4);
        cache.invalidate();
        cache.nets(&b, "");
        assert_eq!(cache.recomputes, 5);
    }

    #[test]
    fn test_auto_view_empty_selection_shows_search() {
        assert_eq!(auto_view(false, false, false, false, false), Tab::Search);
    }

    #[test]
    fn test_auto_view_part_only_shows_part() {
        assert_eq!(auto_view(true, false, false, false, false), Tab::Part);
    }

    #[test]
    fn test_auto_view_pin_via_or_net_shows_net() {
        assert_eq!(auto_view(true, true, false, true, false), Tab::Net);
        assert_eq!(auto_view(false, false, true, true, false), Tab::Net);
        assert_eq!(auto_view(false, false, false, true, false), Tab::Net);
        assert_eq!(auto_view(false, false, true, false, false), Tab::Net);
    }

    #[test]
    fn test_auto_view_unconnected_pin_shows_its_part() {
        // A pin without a net has no net details to show; its part is shown instead.
        assert_eq!(auto_view(true, true, false, false, false), Tab::Part);
    }

    #[test]
    fn test_auto_view_stays_on_part_while_browsing_its_pins() {
        // A pin picked from the part details keeps the part details on screen.
        assert_eq!(auto_view(true, true, false, true, true), Tab::Part);
        assert_eq!(auto_view(false, false, false, true, true), Tab::Net);
    }

    #[test]
    fn test_export_net_details_tsv() {
        let mut board = Board::default();
        board.parts.push(Part {
            id: 0,
            name: "U1".to_string(),
            mfg_code: String::new(),
            side: Side::Top,
            part_type: PartType::Smd,
            p1: Point::ZERO,
            p2: Point::ZERO,
            bounding_box: None,
            pin_indices: vec![0],
            ..Default::default()
        });
        board.pins.push(Pin {
            id: 0,
            name: "1".to_string(),
            part_id: 0,
            net_name: "VCC".to_string(),
            pos: Point::new(10.5, 20.25),
            radius: 15.0,
            side: Side::Top,
            ..Default::default()
        });
        board.rebuild_indices();

        let tsv = export_net_details_tsv(&board, "VCC");
        assert!(tsv.contains("Net:\tVCC"));
        assert!(tsv.contains("U1\t1\tTop\t10.50\t20.25"));
    }

    #[test]
    fn test_export_part_details_tsv() {
        let mut board = Board::default();
        let part = Part {
            id: 0,
            name: "R1".to_string(),
            mfg_code: String::new(),
            side: Side::Bottom,
            part_type: PartType::Smd,
            p1: Point::ZERO,
            p2: Point::ZERO,
            bounding_box: None,
            pin_indices: vec![0],
            ..Default::default()
        };
        board.parts.push(part.clone());
        board.pins.push(Pin {
            id: 0,
            name: "1".to_string(),
            part_id: 0,
            net_name: "GND".to_string(),
            pos: Point::new(5.0, 5.0),
            radius: 10.0,
            side: Side::Bottom,
            ..Default::default()
        });
        board.rebuild_indices();

        let tsv = export_part_details_tsv(&board, &part);
        assert!(tsv.contains("Part:\tR1"));
        assert!(tsv.contains("Side:\tBottom"));
        assert!(tsv.contains("1\tGND\tBottom\t5.00\t5.00"));
    }

    fn board_with_part(side: Side, bbox: Option<bvc_core::Rect>) -> Board {
        let mut board = Board::default();
        board.parts.push(Part {
            id: 0,
            name: "U7".to_string(),
            mfg_code: String::new(),
            side,
            part_type: PartType::Smd,
            p1: Point::ZERO,
            p2: Point::ZERO,
            bounding_box: bbox,
            pin_indices: vec![0],
            ..Default::default()
        });
        board.pins.push(Pin {
            id: 0,
            name: "1".to_string(),
            part_id: 0,
            net_name: "VCC".to_string(),
            pos: Point::new(3.0, 4.0),
            radius: 1.0,
            side,
            ..Default::default()
        });
        board.rebuild_indices();
        board
    }

    #[test]
    fn test_select_part_only_clears_pin_via_and_net() {
        let mut pin = Some(5);
        let mut via = Some(2);
        let mut part = Some(9);
        let mut net = Some("GND".to_string());
        select_part_only(3, &mut pin, &mut via, &mut part, &mut net);
        assert_eq!(part, Some(3));
        assert_eq!(pin, None);
        assert_eq!(via, None);
        assert_eq!(net, None);
    }

    #[test]
    fn test_part_focus_target_uses_bbox_center_and_keeps_visible_side() {
        let bbox = bvc_core::Rect { min_x: 10.0, min_y: 20.0, max_x: 30.0, max_y: 60.0 };
        let board = board_with_part(Side::Top, Some(bbox));
        let (center, side) = part_focus_target(&board, 0, Side::Top).unwrap();
        assert_eq!(center, Point::new(20.0, 40.0));
        assert_eq!(side, Side::Top);
        // Viewing both sides already shows the part: keep the view side.
        assert_eq!(part_focus_target(&board, 0, Side::Both).unwrap().1, Side::Both);
    }

    #[test]
    fn test_part_focus_target_flips_to_part_side() {
        let board = board_with_part(Side::Bottom, None);
        let (center, side) = part_focus_target(&board, 0, Side::Top).unwrap();
        // No bounding box: falls back to the first pin position.
        assert_eq!(center, Point::new(3.0, 4.0));
        assert_eq!(side, Side::Bottom);
        assert!(part_focus_target(&board, 42, Side::Top).is_none());
    }
}



