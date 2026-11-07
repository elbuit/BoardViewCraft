//! CAD Schematic tab UI and vector canvas renderer.

use bvc_core::schematic::SchGraphic;
use crate::i18n::Language;
use crate::schematic_viewer::CadSchematicViewer;
use crate::theme::Tokens;
use egui::{Color32, Pos2, Rect, RichText, Stroke, Ui, Vec2};

#[derive(Default)]
pub struct CadSchematicTabOutput {
    pub open_requested: bool,
    pub close_requested: bool,
    pub clicked_token: Option<String>,
    pub navigate_to_pcb: Option<String>,
}

pub fn show_cad_schematic_tab(
    ui: &mut Ui,
    tokens: &Tokens,
    _lang: Language,
    viewer: &mut CadSchematicViewer,
) -> CadSchematicTabOutput {
    let mut out = CadSchematicTabOutput::default();

    if !viewer.is_loaded() {
        ui.vertical_centered(|ui| {
            ui.add_space((ui.available_height() * 0.4).max(0.0));
            ui.label(RichText::new("No CAD schematic document loaded").color(tokens.text_dim()));
            ui.add_space(8.0);
            if ui.button(RichText::new("Open CAD Schematic (*.kicad_sch, *.sch)").strong()).clicked() {
                out.open_requested = true;
            }
        });
        return out;
    }

    // Top control bar
    ui.horizontal(|ui| {
        ui.heading(RichText::new("CAD Schematic").strong().color(tokens.accent()));
        if let Some(fn_str) = &viewer.filename {
            ui.label(RichText::new(fn_str).small().color(tokens.text_dim()));
        }

        // Sheet selector if multi-sheet
        if let Some(sch) = &viewer.schematic {
            if sch.defs.len() > 1 {
                ui.separator();
                ui.label("Sheet:");
                for (idx, sheet) in sch.defs.iter().enumerate() {
                    let is_active = idx == viewer.current_sheet_idx;
                    let name = if sheet.name.is_empty() { format!("Page {}", idx + 1) } else { sheet.name.clone() };
                    if ui.selectable_label(is_active, name).clicked() {
                        viewer.current_sheet_idx = idx;
                    }
                }
            }
        }

        // Search bar
        ui.separator();
        let search_edit = ui.add(
            egui::TextEdit::singleline(&mut viewer.search_input)
                .desired_width(120.0)
                .hint_text("Find ref / net"),
        );
        if search_edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            viewer.cross_probe(&viewer.search_input.clone());
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("✖").on_hover_text("Close schematic").clicked() {
                out.close_requested = true;
            }
            if ui.button("Fit").on_hover_text("Zoom to fit").clicked() {
                if let Some(sheet) = viewer.current_sheet() {
                    if !sheet.bounds.is_empty() {
                        let available = ui.available_size();
                        let scale_x = available.x / (sheet.bounds.width() as f32).max(10.0);
                        let scale_y = available.y / (sheet.bounds.height() as f32).max(10.0);
                        viewer.zoom = (scale_x.min(scale_y) * 0.9).clamp(0.1, 50.0);
                        viewer.pan = Vec2::ZERO;
                    }
                }
            }
        });
    });
    ui.separator();

    // Canvas drawing area
    let available_rect = ui.available_rect_before_wrap();
    let response = ui.allocate_rect(available_rect, egui::Sense::click_and_drag());

    // Handle Pan and Zoom
    if response.dragged_by(egui::PointerButton::Primary) || response.dragged_by(egui::PointerButton::Middle) {
        viewer.pan += response.drag_delta();
    }
    let zoom_delta = ui.input(|i| i.zoom_delta());
    if zoom_delta != 1.0 {
        viewer.zoom = (viewer.zoom * zoom_delta).clamp(0.05, 100.0);
    }

    let painter = ui.painter_at(available_rect);
    let center_offset = available_rect.min;

    // Background
    painter.rect_filled(available_rect, 0.0, Color32::from_rgb(20, 24, 28));

    let Some(sheet) = viewer.current_sheet() else { return out };

    // Colors
    let wire_color = Color32::from_rgb(46, 204, 113); // Green wire
    let wire_highlight = Color32::from_rgb(241, 196, 15); // Yellow highlight
    let sym_color = Color32::from_rgb(220, 220, 220); // Silver/White symbol
    let pin_color = Color32::from_rgb(231, 76, 60); // Red pin circle/line
    let label_color = Color32::from_rgb(52, 152, 219); // Blue text/label

    // 1. Draw Wires
    for wire in &sheet.wires {
        let p1 = viewer.world_to_screen(wire.a, center_offset);
        let p2 = viewer.world_to_screen(wire.b, center_offset);

        let is_selected_net = viewer.selected_net.as_ref().map_or(false, |net_name| {
            if let Some(sch) = &viewer.schematic {
                if let Some(net) = sch.find_net_by_name(net_name) {
                    return net.wire_ids.iter().any(|(_def, wid)| wid == &wire.id);
                }
            }
            false
        });

        let stroke = if is_selected_net {
            Stroke::new(2.5, wire_highlight)
        } else {
            Stroke::new(1.5, wire_color)
        };
        painter.line_segment([p1, p2], stroke);
    }

    // 2. Draw Junctions
    for j in &sheet.junctions {
        let p = viewer.world_to_screen(j.at, center_offset);
        painter.circle_filled(p, 2.5, wire_color);
    }

    // 3. Draw Symbols & Graphics
    for sym in &sheet.symbols {
        let is_sym_selected = viewer.selected_symbol.as_ref().map_or(false, |s| {
            s.eq_ignore_ascii_case(&sym.ref_default)
                || sym.instances.values().any(|r| r.eq_ignore_ascii_case(s))
        });

        let stroke_color = if is_sym_selected { wire_highlight } else { sym_color };
        let stroke = Stroke::new(if is_sym_selected { 2.0 } else { 1.2 }, stroke_color);

        // Graphics
        for g in &sym.graphics {
            match g {
                SchGraphic::Polyline { points, .. } => {
                    if points.len() >= 2 {
                        let screen_pts: Vec<Pos2> = points.iter().map(|pt| viewer.world_to_screen(*pt, center_offset)).collect();
                        for i in 0..screen_pts.len() - 1 {
                            painter.line_segment([screen_pts[i], screen_pts[i + 1]], stroke);
                        }
                    }
                }
                SchGraphic::Rect { min, max, .. } => {
                    let p1 = viewer.world_to_screen(*min, center_offset);
                    let p2 = viewer.world_to_screen(*max, center_offset);
                    let r = Rect::from_two_pos(p1, p2);
                    painter.rect_stroke(r, 0.0, stroke, egui::StrokeKind::Middle);
                }
                SchGraphic::Circle { center, radius, .. } => {
                    let c = viewer.world_to_screen(*center, center_offset);
                    let r = (*radius as f32) * viewer.zoom;
                    painter.circle_stroke(c, r, stroke);
                }
                _ => {}
            }
        }

        // Symbol pins
        for pin in &sym.pins {
            let p_tip = viewer.world_to_screen(pin.at, center_offset);
            let p_body = viewer.world_to_screen(pin.body, center_offset);
            painter.line_segment([p_tip, p_body], Stroke::new(1.0, pin_color));
            painter.circle_filled(p_tip, 2.0, pin_color);

            // Pin number text
            if viewer.zoom > 1.5 && !pin.number.is_empty() {
                painter.text(
                    p_tip + Vec2::new(3.0, -3.0),
                    egui::Align2::LEFT_BOTTOM,
                    &pin.number,
                    egui::FontId::monospace(10.0),
                    tokens.text_dim(),
                );
            }
        }

        // Reference & Value labels
        let sym_center = viewer.world_to_screen(sym.at, center_offset);
        painter.text(
            sym_center + Vec2::new(0.0, -10.0),
            egui::Align2::CENTER_BOTTOM,
            &sym.ref_default,
            egui::FontId::proportional(12.0),
            if is_sym_selected { wire_highlight } else { Color32::WHITE },
        );
        if !sym.value.is_empty() && viewer.zoom > 0.8 {
            painter.text(
                sym_center + Vec2::new(0.0, 10.0),
                egui::Align2::CENTER_TOP,
                &sym.value,
                egui::FontId::proportional(10.0),
                tokens.text_dim(),
            );
        }
    }

    // 4. Draw Labels
    for lbl in &sheet.labels {
        let p = viewer.world_to_screen(lbl.at, center_offset);
        let is_selected_net = viewer.selected_net.as_ref().map_or(false, |net_name| {
            net_name.eq_ignore_ascii_case(&lbl.text)
        });

        painter.text(
            p + Vec2::new(4.0, -4.0),
            egui::Align2::LEFT_BOTTOM,
            &lbl.text,
            egui::FontId::proportional(11.0),
            if is_selected_net { wire_highlight } else { label_color },
        );
    }

    // Handle clicks for selection & cross-probing
    if response.clicked() {
        if let Some(pos) = response.hover_pos() {
            let world_pos = viewer.screen_to_world(pos, center_offset);
            let mut hit_sym: Option<String> = None;
            let mut hit_net: Option<String> = None;

            // Check symbols (within 5mm)
            for sym in &sheet.symbols {
                if sym.at.distance_to(&world_pos) < 6.0 {
                    hit_sym = Some(sym.ref_default.clone());
                    break;
                }
            }

            // Check labels (within 4mm)
            if hit_sym.is_none() {
                for lbl in &sheet.labels {
                    if lbl.at.distance_to(&world_pos) < 4.0 {
                        hit_net = Some(lbl.text.clone());
                        break;
                    }
                }
            }

            if let Some(sym_ref) = hit_sym {
                viewer.selected_symbol = Some(sym_ref.clone());
                viewer.selected_net = None;
                out.clicked_token = Some(sym_ref.clone());
                out.navigate_to_pcb = Some(sym_ref);
            } else if let Some(net_name) = hit_net {
                viewer.selected_net = Some(net_name.clone());
                viewer.selected_symbol = None;
                out.clicked_token = Some(net_name.clone());
                out.navigate_to_pcb = Some(net_name);
            }
        }
    }

    out
}
