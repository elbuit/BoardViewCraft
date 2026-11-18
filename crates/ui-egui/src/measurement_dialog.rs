//! Modal dialog for entering and editing pin measurements (D, R, V, comments).

use eframe::egui::{self, Button, Color32, RichText, TextEdit, Window};
use bvc_core::{Board, BoardNotesStore, NoteTarget, TechnicalMeasurements};
use crate::i18n::{self, Language};
use crate::theme::Tokens;

#[derive(Debug, Clone, Default)]
pub struct PinMeasurementDialog {
    pub is_open: bool,
    pub target: NoteTarget,
    pub net_name: String,
    pub available_pins: Vec<String>,
    pub d_val: String,
    pub r_val: String,
    pub v_val: String,
    pub comments: String,
}

impl PinMeasurementDialog {
    pub fn open(&mut self, target: NoteTarget, board: Option<&Board>, notes_store: &BoardNotesStore) {
        self.is_open = true;
        self.target = target.clone();
        self.available_pins.clear();
        self.net_name.clear();

        if let Some(b) = board {
            if let Some(part) = b.parts.iter().find(|p| p.name.eq_ignore_ascii_case(&target.component_name)) {
                for &idx in &part.pin_indices {
                    if let Some(pin) = b.pins.get(idx) {
                        self.available_pins.push(pin.name.clone());
                        if target.pin_name.as_deref().is_some_and(|p| p.eq_ignore_ascii_case(&pin.name)) {
                            self.net_name = pin.net_name.clone();
                        }
                    }
                }
            }
        }

        // If target has no pin specified or pin not in list, default to first available
        if self.target.pin_name.is_none() || self.target.pin_name.as_deref().unwrap_or("").is_empty() {
            if let Some(first) = self.available_pins.first().cloned() {
                self.target.pin_name = Some(first.clone());
                if let Some(b) = board {
                    if let Some(part) = b.parts.iter().find(|p| p.name.eq_ignore_ascii_case(&self.target.component_name)) {
                        for &idx in &part.pin_indices {
                            if let Some(pin) = b.pins.get(idx) {
                                if pin.name.eq_ignore_ascii_case(&first) {
                                    self.net_name = pin.net_name.clone();
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }

        // Load existing note if present
        let pin_name = self.target.pin_name.as_deref().unwrap_or("");
        if let Some(existing) = notes_store.find_for_pin(&self.target.component_name, pin_name) {
            self.d_val = existing.measurements.diode_drop.clone().unwrap_or_default();
            self.r_val = existing.measurements.resistance.clone().unwrap_or_default();
            self.v_val = existing.measurements.voltage.clone().unwrap_or_default();
            self.comments = existing.text.clone();
        } else {
            self.d_val.clear();
            self.r_val.clear();
            self.v_val.clear();
            self.comments.clear();
        }
    }

    /// Changes current pin within the same component
    pub fn switch_pin(&mut self, pin_name: &str, board: Option<&Board>, notes_store: &BoardNotesStore) {
        self.target.pin_name = Some(pin_name.to_string());
        self.net_name.clear();
        if let Some(b) = board {
            if let Some(part) = b.parts.iter().find(|p| p.name.eq_ignore_ascii_case(&self.target.component_name)) {
                for &idx in &part.pin_indices {
                    if let Some(pin) = b.pins.get(idx) {
                        if pin.name.eq_ignore_ascii_case(pin_name) {
                            self.net_name = pin.net_name.clone();
                            break;
                        }
                    }
                }
            }
        }
        if let Some(existing) = notes_store.find_for_pin(&self.target.component_name, pin_name) {
            self.d_val = existing.measurements.diode_drop.clone().unwrap_or_default();
            self.r_val = existing.measurements.resistance.clone().unwrap_or_default();
            self.v_val = existing.measurements.voltage.clone().unwrap_or_default();
            self.comments = existing.text.clone();
        } else {
            self.d_val.clear();
            self.r_val.clear();
            self.v_val.clear();
            self.comments.clear();
        }
    }

    pub fn show(
        &mut self,
        ctx: &egui::Context,
        board: Option<&Board>,
        notes_store: &mut BoardNotesStore,
        tokens: &Tokens,
        lang: Language,
        tolerance_pct: f32,
    ) -> bool {
        if !self.is_open {
            return false;
        }

        let mut mutated = false;
        let mut close_dialog = false;

        let pin_label = self.target.display_label();
        let title = format!("📏 {}: {pin_label}", i18n::tr(lang, "measurement.dialog_title"));

        let mut is_open = self.is_open;
        Window::new(RichText::new(title).strong().color(tokens.accent()))
            .open(&mut is_open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                ui.set_min_width(320.0);
                ui.set_max_width(420.0);

                // Pin selector / info line
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("{}: {}", i18n::tr(lang, "context_menu.part_title"), self.target.component_name)).strong());
                    if self.available_pins.len() > 1 {
                        let current_pin = self.target.pin_name.as_deref().unwrap_or("").to_string();
                        let mut switch_to = None;
                        egui::ComboBox::from_id_salt("meas_pin_select")
                            .selected_text(format!("Pin {current_pin}"))
                            .show_ui(ui, |ui| {
                                for pin in &self.available_pins {
                                    if ui.selectable_label(pin == &current_pin, format!("Pin {pin}")).clicked() {
                                        switch_to = Some(pin.clone());
                                    }
                                }
                            });
                        if let Some(pin) = switch_to {
                            self.switch_pin(&pin, board, notes_store);
                        }
                    } else if let Some(ref pin) = self.target.pin_name {
                        ui.label(RichText::new(format!("Pin: {pin}")).strong());
                    }
                });

                let net_display = if self.net_name.is_empty() {
                    "UNCONNECTED"
                } else {
                    &self.net_name
                };
                ui.label(RichText::new(format!("{}: {net_display}", i18n::tr(lang, "context_menu.net"))).color(tokens.text_dim()));

                ui.add_space(6.0);
                ui.separator();
                ui.add_space(6.0);

                // Look up reference metadata for comparison
                let pin_name = self.target.pin_name.as_deref().unwrap_or("");
                let (ref_diode, ref_resistance, ref_voltage) = if let Some(b) = board {
                    let net = (!self.net_name.is_empty()).then_some(self.net_name.as_str());
                    let d = b.metadata.pin_reading(&self.target.component_name, pin_name)
                        .or_else(|| net.and_then(|n| b.metadata.net_reading(n)));
                    let r = net.and_then(|n| b.metadata.net_resistance(n));
                    let v = net.and_then(|n| b.metadata.net_voltage(n));
                    (d, r, v)
                } else {
                    (None, None, None)
                };

                // Measurement inputs: D, R, V (standard diode, resistance, voltage + comments)
                egui::Grid::new("meas_input_grid")
                    .num_columns(2)
                    .spacing([8.0, 8.0])
                    .show(ui, |ui| {
                        // Diode Drop
                        ui.label(RichText::new(i18n::tr(lang, "measurement.d_label")).strong());
                        ui.vertical(|ui| {
                            ui.horizontal(|ui| {
                                ui.add(TextEdit::singleline(&mut self.d_val).hint_text("e.g. 0.450V, 450mV").desired_width(140.0));
                                if let (Some(r), false) = (ref_diode, self.d_val.trim().is_empty()) {
                                    if let Some(comp) = bvc_core::compare_measurement(&self.d_val, r, tolerance_pct) {
                                        let badge_color = if comp.is_pass {
                                            Color32::from_rgb(0x40, 0xc0, 0x57)
                                        } else {
                                            Color32::from_rgb(0xfa, 0x52, 0x52)
                                        };
                                        ui.label(RichText::new(comp.badge_text()).color(badge_color).strong().small());
                                    }
                                }
                            });
                            if let Some(r) = ref_diode {
                                ui.label(RichText::new(format!("Ref: {r}")).color(tokens.text_dim()).small());
                            }
                        });
                        ui.end_row();

                        // Resistance
                        ui.label(RichText::new(i18n::tr(lang, "measurement.r_label")).strong());
                        ui.vertical(|ui| {
                            ui.horizontal(|ui| {
                                ui.add(TextEdit::singleline(&mut self.r_val).hint_text("e.g. 10k, OL, 0Ω").desired_width(140.0));
                                if let (Some(r), false) = (ref_resistance, self.r_val.trim().is_empty()) {
                                    if let Some(comp) = bvc_core::compare_measurement(&self.r_val, r, tolerance_pct) {
                                        let badge_color = if comp.is_pass {
                                            Color32::from_rgb(0x40, 0xc0, 0x57)
                                        } else {
                                            Color32::from_rgb(0xfa, 0x52, 0x52)
                                        };
                                        ui.label(RichText::new(comp.badge_text()).color(badge_color).strong().small());
                                    }
                                }
                            });
                            if let Some(r) = ref_resistance {
                                ui.label(RichText::new(format!("Ref: {r}")).color(tokens.text_dim()).small());
                            }
                        });
                        ui.end_row();

                        // Voltage
                        ui.label(RichText::new(i18n::tr(lang, "measurement.v_label")).strong());
                        ui.vertical(|ui| {
                            ui.horizontal(|ui| {
                                ui.add(TextEdit::singleline(&mut self.v_val).hint_text("e.g. 3.3V, 1.8V").desired_width(140.0));
                                if let (Some(r), false) = (ref_voltage, self.v_val.trim().is_empty()) {
                                    if let Some(comp) = bvc_core::compare_measurement(&self.v_val, r, tolerance_pct) {
                                        let badge_color = if comp.is_pass {
                                            Color32::from_rgb(0x40, 0xc0, 0x57)
                                        } else {
                                            Color32::from_rgb(0xfa, 0x52, 0x52)
                                        };
                                        ui.label(RichText::new(comp.badge_text()).color(badge_color).strong().small());
                                    }
                                }
                            });
                            if let Some(r) = ref_voltage {
                                ui.label(RichText::new(format!("Ref: {r}")).color(tokens.text_dim()).small());
                            }
                        });
                        ui.end_row();
                    });

                ui.add_space(6.0);
                ui.label(RichText::new(i18n::tr(lang, "measurement.comments")).strong());
                ui.add(TextEdit::multiline(&mut self.comments).desired_rows(3).desired_width(ui.available_width()).hint_text("e.g. OK, Shorted, No clock..."));

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(4.0);

                let pin_name = self.target.pin_name.as_deref().unwrap_or("");
                let has_existing = notes_store.find_for_pin(&self.target.component_name, pin_name).is_some();

                ui.horizontal(|ui| {
                    let save_btn = ui.add(Button::new(RichText::new(format!("💾 {}", i18n::tr(lang, "notes.save"))).strong().color(tokens.accent())));
                    if save_btn.clicked() || (ui.input(|i| i.modifiers.command && i.key_pressed(egui::Key::Enter))) {
                        let measurements = TechnicalMeasurements {
                            voltage: (!self.v_val.trim().is_empty()).then(|| self.v_val.trim().to_string()),
                            resistance: (!self.r_val.trim().is_empty()).then(|| self.r_val.trim().to_string()),
                            diode_drop: (!self.d_val.trim().is_empty()).then(|| self.d_val.trim().to_string()),
                            capacitance: None,
                            other: None,
                        };
                        let text = self.comments.trim().to_string();
                        if text.is_empty() && measurements.is_empty() {
                            notes_store.remove(&self.target);
                        } else {
                            notes_store.upsert(self.target.clone(), text, measurements);
                        }
                        mutated = true;
                        close_dialog = true;
                    }

                    if has_existing {
                        if ui.button(RichText::new(format!("🗑 {}", i18n::tr(lang, "notes.delete"))).color(Color32::from_rgb(0xe0, 0x55, 0x55))).clicked() {
                            notes_store.remove(&self.target);
                            mutated = true;
                            close_dialog = true;
                        }
                    }

                    if ui.button(i18n::tr(lang, "measurement.cancel")).clicked() {
                        close_dialog = true;
                    }
                });

                if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    close_dialog = true;
                }
            });

        if !is_open || close_dialog {
            self.is_open = false;
        }

        mutated
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bvc_core::{Part, PartType, Pin, Point, Side};

    fn make_test_board() -> Board {
        let mut b = Board::new();
        let part = Part {
            id: 0,
            name: "U1".to_string(),
            part_type: PartType::Smd,
            side: Side::Top,
            mfg_code: "SOIC-8".to_string(),
            pin_indices: vec![0, 1],
            ..Default::default()
        };
        b.parts.push(part);
        b.pins.push(Pin {
            id: 0,
            name: "1".to_string(),
            part_id: 0,
            pos: Point::new(10.0, 10.0),
            radius: 2.0,
            side: Side::Top,
            net_name: "VCC".to_string(),
            ..Default::default()
        });
        b.pins.push(Pin {
            id: 1,
            name: "2".to_string(),
            part_id: 0,
            pos: Point::new(20.0, 10.0),
            radius: 2.0,
            side: Side::Top,
            net_name: "GND".to_string(),
            ..Default::default()
        });
        b
    }

    #[test]
    fn test_measurement_dialog_open_and_load_existing() {
        let board = make_test_board();
        let mut notes_store = BoardNotesStore::default();
        let measurements = TechnicalMeasurements {
            voltage: Some("3.3V".to_string()),
            resistance: Some("10k".to_string()),
            diode_drop: Some("0.450V".to_string()),
            capacitance: None,
            other: None,
        };
        notes_store.upsert(NoteTarget::pin("U1", "1"), "Normal operation".to_string(), measurements);

        let mut dialog = PinMeasurementDialog::default();
        dialog.open(NoteTarget::pin("U1", "1"), Some(&board), &notes_store);

        assert!(dialog.is_open);
        assert_eq!(dialog.target.component_name, "U1");
        assert_eq!(dialog.target.pin_name.as_deref(), Some("1"));
        assert_eq!(dialog.net_name, "VCC");
        assert_eq!(dialog.available_pins, vec!["1", "2"]);
        assert_eq!(dialog.d_val, "0.450V");
        assert_eq!(dialog.r_val, "10k");
        assert_eq!(dialog.v_val, "3.3V");
        assert_eq!(dialog.comments, "Normal operation");
    }

    #[test]
    fn test_measurement_dialog_switch_pin() {
        let board = make_test_board();
        let notes_store = BoardNotesStore::default();

        let mut dialog = PinMeasurementDialog::default();
        dialog.open(NoteTarget::pin("U1", "1"), Some(&board), &notes_store);
        assert_eq!(dialog.net_name, "VCC");

        dialog.switch_pin("2", Some(&board), &notes_store);
        assert_eq!(dialog.target.pin_name.as_deref(), Some("2"));
        assert_eq!(dialog.net_name, "GND");
        assert!(dialog.d_val.is_empty());
        assert!(dialog.r_val.is_empty());
        assert!(dialog.v_val.is_empty());
    }

    #[test]
    fn test_measurement_dialog_defaults_to_first_pin_if_unspecified() {
        let board = make_test_board();
        let notes_store = BoardNotesStore::default();

        let mut dialog = PinMeasurementDialog::default();
        dialog.open(NoteTarget::component("U1"), Some(&board), &notes_store);

        assert_eq!(dialog.target.pin_name.as_deref(), Some("1"));
        assert_eq!(dialog.net_name, "VCC");
    }
}
