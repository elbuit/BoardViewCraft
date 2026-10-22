//! Side inspection panel for BoardViewCraft.

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
    selected_part_idx: &mut Option<usize>,
    selected_net: &mut Option<String>,
    current_side: Side,
) {
    ui.horizontal(|ui| {
        if ui
            .selectable_label(state.active_tab == Tab::Net, "Net Details")
            .clicked()
        {
            state.active_tab = Tab::Net;
        }
        if ui
            .selectable_label(state.active_tab == Tab::Part, "Part Details")
            .clicked()
        {
            state.active_tab = Tab::Part;
        }
        if ui
            .selectable_label(state.active_tab == Tab::Search, "Search / List")
            .clicked()
        {
            state.active_tab = Tab::Search;
        }
    });

    ui.separator();

    let Some(b) = board else {
        ui.label(RichText::new("No board loaded").color(tokens.text_dim));
        return;
    };

    match state.active_tab {
        Tab::Net => {
            if let Some(net_name) = selected_net {
                ui.heading(RichText::new(format!("Net: {net_name}")).color(tokens.accent));
                if let Some(net) = b.get_net(net_name) {
                    ui.label(
                        RichText::new(format!(
                            "Connected: {} pins, {} vias",
                            net.pin_indices.len(),
                            net.via_indices.len()
                        ))
                        .color(tokens.text_dim),
                    );

                    ui.add_space(8.0);
                    ui.label(RichText::new("Pins on Net:").strong());

                    ScrollArea::vertical()
                        .id_salt("net_pins_scroll")
                        .max_height(280.0)
                        .show(ui, |ui| {
                            for &pin_idx in &net.pin_indices {
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
                                        tokens.pin_highlight_cur_side
                                    } else {
                                        tokens.pin_highlight_opp_side
                                    };

                                    let is_selected = *selected_pin_idx == Some(pin_idx);
                                    if ui
                                        .selectable_label(
                                            is_selected,
                                            RichText::new(label_text).color(color),
                                        )
                                        .clicked()
                                    {
                                        *selected_pin_idx = Some(pin_idx);
                                        *selected_part_idx = Some(pin.part_id);
                                    }
                                }
                            }
                        });

                    if !net.via_indices.is_empty() {
                        ui.add_space(8.0);
                        ui.label(RichText::new("Vias on Net:").strong());
                        ScrollArea::vertical()
                            .id_salt("net_vias_scroll")
                            .max_height(150.0)
                            .show(ui, |ui| {
                                for &via_idx in &net.via_indices {
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
                                            .color(tokens.text_dim),
                                        );
                                    }
                                }
                            });
                    }
                }
            } else {
                ui.label(RichText::new("Click a pin or via to view its net.").color(tokens.text_dim));
            }
        }
        Tab::Part => {
            if let Some(part_id) = selected_part_idx {
                if let Some(part) = b.parts.get(*part_id) {
                    ui.heading(RichText::new(format!("Part: {}", part.name)).color(tokens.accent));
                    let side_str = match part.side {
                        Side::Top => "Top",
                        Side::Bottom => "Bottom",
                        Side::Both => "Both",
                    };
                    ui.label(
                        RichText::new(format!("Side: {side_str} | Pins: {}", part.pin_indices.len()))
                            .color(tokens.text_dim),
                    );

                    ui.add_space(8.0);
                    ui.label(RichText::new("Pins:").strong());

                    ScrollArea::vertical()
                        .id_salt("part_pins_scroll")
                        .show(ui, |ui| {
                        for &pin_idx in &part.pin_indices {
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

                                if ui
                                    .selectable_label(
                                        is_selected,
                                        RichText::new(pin_label).color(tokens.text),
                                    )
                                    .clicked()
                                {
                                    *selected_pin_idx = Some(pin_idx);
                                    *selected_net = if pin.net_name.is_empty() {
                                        None
                                    } else {
                                        Some(pin.net_name.clone())
                                    };
                                }
                            }
                        }
                    });
                }
            } else {
                ui.label(RichText::new("Click a component pin to view part details.").color(tokens.text_dim));
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
