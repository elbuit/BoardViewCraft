//! Side inspection panel for BoardViewCraft.

use crate::i18n::{self, Language};
use crate::overlay::OverlayManager;
use crate::overlay_align::{AlignmentSession, MAX_PAIRS};
use crate::theme::Tokens;
use bvc_core::{Board, Side};
use egui::{Button, Color32, DragValue, RichText, ScrollArea, Slider, TextEdit, Ui};

pub struct SidePanelState {
    pub filter_part: String,
    pub filter_net: String,
    pub active_tab: Tab,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Net,
    Part,
    Search,
    Overlay,
}

impl Default for SidePanelState {
    fn default() -> Self {
        Self {
            filter_part: String::new(),
            filter_net: String::new(),
            active_tab: Tab::Net,
        }
    }
}

pub fn show_side_panel(
    ui: &mut Ui,
    tokens: &Tokens,
    board: Option<&Board>,
    state: &mut SidePanelState,
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
    ui.horizontal(|ui| {
        if ui
            .selectable_label(state.active_tab == Tab::Net, i18n::tr(lang, "side_panel.net_details"))
            .clicked()
        {
            state.active_tab = Tab::Net;
        }
        if ui
            .selectable_label(state.active_tab == Tab::Part, i18n::tr(lang, "side_panel.part_details"))
            .clicked()
        {
            state.active_tab = Tab::Part;
        }
        if ui
            .selectable_label(state.active_tab == Tab::Search, i18n::tr(lang, "side_panel.search"))
            .clicked()
        {
            state.active_tab = Tab::Search;
        }
        if ui
            .selectable_label(state.active_tab == Tab::Overlay, i18n::tr(lang, "side_panel.overlay"))
            .clicked()
        {
            state.active_tab = Tab::Overlay;
        }
    });

    ui.separator();

    if state.active_tab == Tab::Overlay {
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
        ui.label(RichText::new("No board loaded").color(tokens.text_dim()));
        return;
    };

    match state.active_tab {
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
                    ui.label(RichText::new("Pins on Net:").strong());

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
                        ui.label(RichText::new("Vias on Net:").strong());
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
                ui.label(RichText::new("Click a pin or via to view its net.").color(tokens.text_dim()));
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
                    ui.label(RichText::new("Pins:").strong());

                    let row_height = 20.0;
                    ScrollArea::vertical()
                        .id_salt("part_pins_scroll")
                        .show_rows(ui, row_height, part.pin_indices.len(), |ui, row_range| {
                            for row_idx in row_range {
                                let pin_idx = part.pin_indices[row_idx];
                                if let Some(pin) = b.pins.get(pin_idx) {
                                    let is_selected = *selected_pin_idx == Some(pin_idx);
                                    let pin_label = format!(
                                        "Pin {}: Net '{}'",
                                        pin.name,
                                        if pin.net_name.is_empty() {
                                            "UNCONNECTED"
                                        } else {
                                            &pin.net_name
                                        }
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
                }
            } else {
                ui.label(RichText::new("Click a component pin to view part details.").color(tokens.text_dim()));
            }
        }
        Tab::Search => {
            ui.label(RichText::new("Search Parts:").strong());
            ui.add(
                TextEdit::singleline(&mut state.filter_part)
                    .hint_text("Type part name (e.g. U1, R1)..."),
            );

            ScrollArea::vertical()
                .id_salt("search_parts_scroll")
                .max_height(200.0)
                .show(ui, |ui| {
                    let needle = state.filter_part.trim().to_ascii_uppercase();
                    for part in &b.parts {
                        if needle.is_empty() || part.name.to_ascii_uppercase().contains(&needle) {
                            let is_selected = *selected_part_idx == Some(part.id);
                            if ui
                                .selectable_label(
                                    is_selected,
                                    format!("{} ({:?})", part.name, part.side),
                                )
                                .clicked()
                            {
                                *selected_part_idx = Some(part.id);
                                if let Some(&first_pin) = part.pin_indices.first() {
                                    *selected_pin_idx = Some(first_pin);
                                    if let Some(pin) = b.pins.get(first_pin) {
                                        *selected_net = if pin.net_name.is_empty() {
                                            None
                                        } else {
                                            Some(pin.net_name.clone())
                                        };
                                    }
                                }
                                state.active_tab = Tab::Part;
                            }
                        }
                    }
                });

            ui.separator();
            ui.label(RichText::new("Search Nets:").strong());
            ui.add(
                TextEdit::singleline(&mut state.filter_net)
                    .hint_text("Type net name (e.g. VCC, GND)..."),
            );

            ScrollArea::vertical()
                .id_salt("search_nets_scroll")
                .show(ui, |ui| {
                let needle = state.filter_net.trim().to_ascii_uppercase();
                for (name, net) in &b.nets {
                    if needle.is_empty() || name.to_ascii_uppercase().contains(&needle) {
                        let is_selected = selected_net.as_deref() == Some(name);
                        if ui
                            .selectable_label(
                                is_selected,
                                format!("{} ({} pins)", name, net.pin_indices.len()),
                            )
                            .clicked()
                        {
                            *selected_net = Some(name.clone());
                            *selected_pin_idx = net.pin_indices.first().copied();
                            if let Some(p_idx) = *selected_pin_idx {
                                *selected_part_idx = b.pins.get(p_idx).map(|p| p.part_id);
                            }
                        }
                    }
                }
            });
        }
        Tab::Overlay => {
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
            alignment_session.start(snap);
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
        });
        board.pins.push(Pin {
            id: 0,
            name: "1".to_string(),
            part_id: 0,
            net_name: "VCC".to_string(),
            pos: Point::new(10.5, 20.25),
            radius: 15.0,
            side: Side::Top,
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
        });
        board.rebuild_indices();

        let tsv = export_part_details_tsv(&board, &part);
        assert!(tsv.contains("Part:\tR1"));
        assert!(tsv.contains("Side:\tBottom"));
        assert!(tsv.contains("1\tGND\tBottom\t5.00\t5.00"));
    }
}



