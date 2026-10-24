//! Side inspection panel for BoardViewCraft.

use crate::i18n::{self, Language};
use crate::theme::Tokens;
use bvc_core::{Board, Side};
use egui::{RichText, ScrollArea, TextEdit, Ui};

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
    selected_pin_idx: &mut Option<usize>,
    selected_via_idx: &mut Option<usize>,
    selected_part_idx: &mut Option<usize>,
    selected_net: &mut Option<String>,
    current_side: Side,
    lang: Language,
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
    });

    ui.separator();

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
    }
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



