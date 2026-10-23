//! Main application entry point and window runner for BoardViewCraft.

mod canvas;
mod preferences;
mod render;
mod side_panel;
mod theme;

use bvc_core::{Board, ParserRegistry, Side};
use canvas::{CanvasState, show_canvas};
use preferences::{Preferences, show_preferences_window};
use render::Renderer;
use side_panel::{SidePanelState, show_side_panel};
use theme::{ThemePreset, Tokens};

use egui::{CentralPanel, Color32, Panel, RichText, Ui, Vec2};
use std::sync::Arc;

pub struct BoardViewApp {
    board: Option<Board>,
    parser_registry: Arc<ParserRegistry>,
    tokens: Tokens,
    renderer: Renderer,
    canvas_state: CanvasState,
    side_panel_state: SidePanelState,
    preferences: Preferences,
    preferences_open: bool,

    // Selection state
    selected_pin_idx: Option<usize>,
    selected_part_idx: Option<usize>,
    selected_net: Option<String>,

    // Status message
    status_msg: String,
    filename: Option<String>,
}

impl Default for BoardViewApp {
    fn default() -> Self {
        Self {
            board: None,
            parser_registry: Arc::new(ParserRegistry::default()),
            tokens: Tokens::DARK,
            renderer: Renderer::new(),
            canvas_state: CanvasState::default(),
            side_panel_state: SidePanelState::default(),
            preferences: Preferences::default(),
            preferences_open: false,
            selected_pin_idx: None,
            selected_part_idx: None,
            selected_net: None,
            status_msg: "Ready. Open a .cad file to inspect.".to_string(),
            filename: None,
        }
    }
}

impl BoardViewApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self::default()
    }

    pub fn load_file(&mut self, filename: &str, data: &[u8]) {
        match self.parser_registry.parse_file(filename, data) {
            Ok(board) => {
                let parts_cnt = board.parts.len();
                let pins_cnt = board.pins.len();
                let vias_cnt = board.vias.len();
                let nets_cnt = board.nets.len();

                self.status_msg = format!(
                    "Loaded {filename}: {parts_cnt} parts, {pins_cnt} pins, {vias_cnt} vias, {nets_cnt} nets"
                );
                self.filename = Some(filename.to_string());
                self.selected_pin_idx = None;
                self.selected_part_idx = None;
                self.selected_net = None;

                // Center view
                if let Some(bbox) = board.bounding_box {
                    self.canvas_state.viewport.center = bbox.center();
                }

                self.board = Some(board);
            }
            Err(err) => {
                self.status_msg = format!("Failed to open {filename}: {err}");
            }
        }
    }

    pub fn open_file_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("CAD files (*.cad)", &["cad"])
            .add_filter("All files (*.*)", &["*"])
            .pick_file()
        {
            if let Ok(bytes) = std::fs::read(&path) {
                let name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("board.cad");
                self.load_file(name, &bytes);
            }
        }
    }
}

impl eframe::App for BoardViewApp {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        // Handle global hotkeys
        if ui.input(|i| (i.modifiers.ctrl || i.modifiers.command) && i.key_pressed(egui::Key::Comma)) {
            self.preferences_open = !self.preferences_open;
        }

        // Actions shortcuts: R (Rotate CW), Shift+R (Rotate CCW), Space (Flip Side), M (Mirror X), Shift+M (Mirror Y), F (Fit)
        if !self.preferences_open {
            let (r_pressed, shift_down, space_pressed, m_pressed) = ui.input(|i| {
                (
                    i.key_pressed(egui::Key::R),
                    i.modifiers.shift,
                    i.key_pressed(egui::Key::Space),
                    i.key_pressed(egui::Key::M),
                )
            });

            if r_pressed {
                self.canvas_state.viewport.rotation = if shift_down {
                    self.canvas_state.viewport.rotation.rotate_ccw()
                } else {
                    self.canvas_state.viewport.rotation.rotate_cw()
                };
            }

            if space_pressed {
                self.canvas_state.viewport.side = if self.canvas_state.viewport.side == Side::Top {
                    Side::Bottom
                } else {
                    Side::Top
                };
            }

            if m_pressed {
                if shift_down {
                    self.canvas_state.viewport.mirror_y = !self.canvas_state.viewport.mirror_y;
                } else {
                    self.canvas_state.viewport.mirror_x = !self.canvas_state.viewport.mirror_x;
                }
            }
        }

        // Cadcraft top toolbar
        Panel::top("top_bar").show(ui, |ui: &mut Ui| {
            ui.add_space(3.0);
            ui.horizontal(|ui: &mut Ui| {
                ui.heading(RichText::new("BoardViewCraft").strong().color(self.tokens.accent));
                ui.separator();

                if ui.button("Open CAD...").clicked() {
                    self.open_file_dialog();
                }

                // Actions Menu dropdown
                ui.menu_button("Actions", |ui| {
                    if ui.button("Rotate 90° CW (R)").clicked() {
                        self.canvas_state.viewport.rotation = self.canvas_state.viewport.rotation.rotate_cw();
                        ui.close();
                    }
                    if ui.button("Rotate 90° CCW (Shift+R)").clicked() {
                        self.canvas_state.viewport.rotation = self.canvas_state.viewport.rotation.rotate_ccw();
                        ui.close();
                    }
                    if ui.button("Rotate 180°").clicked() {
                        self.canvas_state.viewport.rotation = self.canvas_state.viewport.rotation.rotate_180();
                        ui.close();
                    }
                    ui.separator();
                    let is_top = self.canvas_state.viewport.side == Side::Top;
                    if ui.button(if is_top { "Flip to BOTTOM (Space)" } else { "Flip to TOP (Space)" }).clicked() {
                        self.canvas_state.viewport.side = if is_top { Side::Bottom } else { Side::Top };
                        ui.close();
                    }
                    if ui.button(format!("Mirror X [Horizontal] (M) [{}]", if self.canvas_state.viewport.mirror_x { "ON" } else { "OFF" })).clicked() {
                        self.canvas_state.viewport.mirror_x = !self.canvas_state.viewport.mirror_x;
                        ui.close();
                    }
                    if ui.button(format!("Mirror Y [Vertical] (Shift+M) [{}]", if self.canvas_state.viewport.mirror_y { "ON" } else { "OFF" })).clicked() {
                        self.canvas_state.viewport.mirror_y = !self.canvas_state.viewport.mirror_y;
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("Reset Orientation").clicked() {
                        self.canvas_state.viewport.rotation = canvas::Rotation::Deg0;
                        self.canvas_state.viewport.mirror_x = false;
                        self.canvas_state.viewport.mirror_y = false;
                        self.canvas_state.viewport.side = Side::Top;
                        ui.close();
                    }
                });

                if ui.button("Preferences...").clicked() {
                    self.preferences_open = true;
                }

                ui.separator();

                // Quick flip side button
                let is_top = self.canvas_state.viewport.side == Side::Top;
                let toggle_label = if is_top { "View: TOP" } else { "View: BOTTOM" };
                if ui.button(toggle_label).on_hover_text("Click or press Space to flip board side").clicked() {
                    self.canvas_state.viewport.side = if is_top { Side::Bottom } else { Side::Top };
                }

                // Quick rotation button
                let rot_label = format!("Rotate: {}", self.canvas_state.viewport.rotation.display_str());
                if ui.button(rot_label).on_hover_text("Click or press R to rotate 90° CW").clicked() {
                    self.canvas_state.viewport.rotation = self.canvas_state.viewport.rotation.rotate_cw();
                }

                // Fit to window
                if ui.button("Fit Window (F)").clicked() || ui.input(|i| i.key_pressed(egui::Key::F)) {
                    if let Some(board) = &self.board {
                        if let Some(bbox) = board.bounding_box {
                            let rect = ui.clip_rect();
                            self.canvas_state.viewport.fit_to_rect(bbox, rect);
                        }
                    }
                }

                // Zoom reset
                if ui.button("1:1").clicked() {
                    self.canvas_state.viewport.scale = 0.05;
                }

                if let Some(name) = &self.filename {
                    ui.separator();
                    ui.label(RichText::new(name).color(self.tokens.text_dim));
                }
            });
            ui.add_space(3.0);
        });

        // Bottom status bar
        Panel::bottom("status_bar").show(ui, |ui: &mut Ui| {
            ui.add_space(2.0);
            ui.horizontal(|ui: &mut Ui| {
                ui.label(RichText::new(&self.status_msg).color(self.tokens.status_bar_text));

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui: &mut Ui| {
                    let side_str = match self.canvas_state.viewport.side {
                        Side::Top => "TOP",
                        Side::Bottom => "BOTTOM",
                        Side::Both => "BOTH",
                    };
                    let rot_str = self.canvas_state.viewport.rotation.display_str();
                    let mirror_str = match (self.canvas_state.viewport.mirror_x, self.canvas_state.viewport.mirror_y) {
                        (true, true) => " [Mirror XY]",
                        (true, false) => " [Mirror X]",
                        (false, true) => " [Mirror Y]",
                        (false, false) => "",
                    };
                    ui.label(RichText::new(format!("[{side_str} | Rot: {rot_str}{mirror_str}] Scale: {:.4}", self.canvas_state.viewport.scale)).color(self.tokens.text_dim));
                });
            });
            ui.add_space(2.0);
        });

        // Right side details / search panel
        Panel::right("side_panel")
            .resizable(true)
            .default_size(320.0)
            .size_range(240.0..=500.0)
            .show(ui, |ui: &mut Ui| {
                show_side_panel(
                    ui,
                    &self.tokens,
                    self.board.as_ref(),
                    &mut self.side_panel_state,
                    &mut self.selected_pin_idx,
                    &mut self.selected_part_idx,
                    &mut self.selected_net,
                    self.canvas_state.viewport.side,
                );
            });

        // Central Canvas Panel
        CentralPanel::default()
            .frame(egui::Frame::NONE.fill(self.tokens.canvas))
            .show(ui, |ui: &mut Ui| {
                show_canvas(
                    ui,
                    &self.tokens,
                    self.board.as_ref(),
                    &mut self.canvas_state,
                    &mut self.selected_pin_idx,
                    &mut self.selected_part_idx,
                    &mut self.selected_net,
                    &mut self.renderer,
                    self.preferences.show_net_web,
                    self.preferences.show_opposite_elements,
                );
            });

        // Preferences modal window
        let mut prefs_changed = false;
        let mut prefs = self.preferences.clone();
        if show_preferences_window(
            ui.ctx(),
            &mut self.preferences_open,
            &mut prefs,
            &self.tokens,
        ) {
            prefs_changed = true;
        }

        if prefs_changed {
            if prefs.theme != self.preferences.theme {
                self.tokens = prefs.theme.tokens();
                let is_light = prefs.theme == ThemePreset::LightMode;
                ui.ctx().set_visuals(self.tokens.egui_visuals(is_light));
            }
            // Invalidate canvas mesh cache
            self.canvas_state.cached_view_key = None;
            self.preferences = prefs;
        }
    }
}

fn main() -> eframe::Result<()> {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size(Vec2::new(1280.0, 800.0))
            .with_title("BoardViewCraft - Pure Rust CAD Board Viewer"),
        ..Default::default()
    };

    eframe::run_native(
        "BoardViewCraft",
        native_options,
        Box::new(|cc| {
            // Apply cadcraft dark styling defaults
            let mut visuals = egui::Visuals::dark();
            visuals.panel_fill = Color32::from_rgb(0x28, 0x2e, 0x38);
            visuals.window_fill = Color32::from_rgb(0x28, 0x2e, 0x38);
            cc.egui_ctx.set_visuals(visuals);

            let mut app = BoardViewApp::new(cc);

            // If a file argument is provided on CLI, load it automatically on start
            if let Some(arg) = std::env::args().nth(1) {
                let path = std::path::Path::new(&arg);
                if let Ok(bytes) = std::fs::read(path) {
                    let name = path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("board.cad");
                    app.load_file(name, &bytes);
                } else {
                    app.status_msg = format!("Failed to read file: {arg}");
                }
            }

            Ok(Box::new(app))
        }),
    )
}
