//! Main application entry point and window runner for BoardViewCraft.

mod canvas;
mod i18n;
mod overlay;
pub mod overlay_align;
pub mod pdf;
mod preferences;
mod render;
mod side_panel;
mod theme;

use bvc_core::{Board, ParserRegistry, Side};
use canvas::{CanvasState, show_canvas};
use overlay::OverlayManager;
use overlay_align::AlignmentSession;
use preferences::{Preferences, show_preferences_window};
use render::Renderer;
use side_panel::{SidePanelState, show_side_panel};
use theme::{AppTheme, Tokens};

use egui::{CentralPanel, Color32, Panel, Pos2, Rect, RichText, Ui, Vec2};
use std::sync::Arc;

pub struct BoardViewApp {
    board: Option<Board>,
    parser_registry: Arc<ParserRegistry>,
    tokens: Tokens,
    renderer: Renderer,
    canvas_state: CanvasState,
    side_panel_state: SidePanelState,
    overlay_manager: OverlayManager,
    alignment_session: AlignmentSession,
    preferences: Preferences,
    preferences_open: bool,
    pending_auto_fit: bool,

    // Selection state
    selected_pin_idx: Option<usize>,
    selected_via_idx: Option<usize>,
    selected_part_idx: Option<usize>,
    selected_net: Option<String>,

    // Status message
    status_msg: String,
    filename: Option<String>,

    // PDF Schematic integration
    pdf_viewer: pdf::PdfSchematicViewer,
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
            overlay_manager: OverlayManager::default(),
            alignment_session: AlignmentSession::default(),
            preferences: Preferences::default(),
            preferences_open: false,
            pending_auto_fit: false,
            selected_pin_idx: None,
            selected_via_idx: None,
            selected_part_idx: None,
            selected_net: None,
            status_msg: "Ready. Open a .cad file to inspect.".to_string(),
            filename: None,
            pdf_viewer: pdf::PdfSchematicViewer::default(),
        }
    }
}

impl BoardViewApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let preferences = Preferences::load();
        let tokens = Tokens::new_with_custom(
            preferences.app_theme,
            preferences.pcb_theme,
            preferences.custom_pcb_tokens,
        );
        let is_light = preferences.app_theme == AppTheme::Light;
        cc.egui_ctx.set_visuals(tokens.egui_visuals(is_light));

        Self {
            preferences,
            tokens,
            ..Self::default()
        }
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
                self.selected_via_idx = None;
                self.selected_part_idx = None;
                self.selected_net = None;

                // Center view and request auto-fit to window on next render frame
                if let Some(bbox) = board.bounding_box {
                    self.canvas_state.viewport.center = bbox.center();
                    self.pending_auto_fit = true;
                }

                self.board = Some(board);
            }
            Err(err) => {
                self.status_msg = format!("Failed to open {filename}: {err}");
            }
        }
    }

    pub fn open_file_from_path(&mut self, path: &std::path::Path) {
        match std::fs::read(path) {
            Ok(bytes) => {
                let name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("board.cad");
                self.load_file(name, &bytes);
                self.preferences.add_recent_file(path.to_path_buf());
                let _ = self.preferences.save_atomic();
            }
            Err(err) => {
                self.status_msg = format!("Could not open {}: {err}", path.display());
            }
        }
    }

    pub fn open_file_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter(
                "All Supported Boardviews (*.cad, *.tvw, *.brd, *.fz, *.pcb, *.bdv, *.asc, *.bv, *.bvr, *.cst, *.gr, *.f2b)",
                &["cad", "tvw", "brd", "fz", "pcb", "bdv", "asc", "bv", "bvr", "cst", "gr", "f2b"],
            )
            .add_filter("CAD files (*.cad)", &["cad"])
            .add_filter("TestView files (*.tvw)", &["tvw"])
            .add_filter("Eagle / BRD files (*.brd)", &["brd"])
            .add_filter("Foxconn files (*.fz)", &["fz"])
            .add_filter("XZZ PCB files (*.pcb)", &["pcb"])
            .add_filter("BDV files (*.bdv)", &["bdv"])
            .add_filter("ASC files (*.asc)", &["asc"])
            .add_filter("Other Boardviews (*.bv, *.bvr, *.cst, *.gr, *.f2b)", &["bv", "bvr", "cst", "gr", "f2b"])
            .add_filter("All files (*.*)", &["*"])
            .pick_file()
        {
            self.open_file_from_path(&path);
        }
    }

    pub fn open_photo_dialog(&mut self, ctx: &egui::Context, side: Side) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Images (*.png, *.jpg, *.jpeg, *.webp, *.bmp)", &["png", "jpg", "jpeg", "webp", "bmp"])
            .add_filter("All files (*.*)", &["*"])
            .pick_file()
        {
            if let Ok(bytes) = std::fs::read(&path) {
                let bbox = self.board.as_ref().and_then(|b| b.bounding_box);
                let slot = self.overlay_manager.slot_for_side(side);
                if let Err(err) = slot.load_from_bytes(ctx, &path, &bytes, bbox) {
                    self.status_msg = format!("Failed to load photo: {err}");
                } else {
                    self.status_msg = format!("Loaded overlay photo for {side:?}: {}", path.display());
                    self.side_panel_state.active_tab = side_panel::Tab::Overlay;
                }
            }
        }
    }

    pub fn open_pdf_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("PDF Schematics (*.pdf)", &["pdf"])
            .add_filter("All files (*.*)", &["*"])
            .pick_file()
        {
            if let Err(err) = self.pdf_viewer.open_file(&path) {
                self.status_msg = format!("Failed to open schematic PDF: {err}");
            } else {
                self.status_msg = format!("Opened schematic: {}", path.display());
            }
        }
    }
}

impl eframe::App for BoardViewApp {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        // Handle Drag & Drop files
        let dropped_files = ui.ctx().input_mut(|i| std::mem::take(&mut i.raw.dropped_files));
        if !dropped_files.is_empty() {
            for file in dropped_files {
                let path = file.path();
                if !path.as_os_str().is_empty() {
                    let ext = path
                        .extension()
                        .and_then(|s| s.to_str())
                        .unwrap_or("")
                        .to_ascii_lowercase();
                    match ext.as_str() {
                        "png" | "jpg" | "jpeg" | "webp" | "bmp" => {
                            if let Ok(bytes) = std::fs::read(path) {
                                let cur_side = self.canvas_state.viewport.side;
                                let bbox = self.board.as_ref().and_then(|b| b.bounding_box);
                                let slot = self.overlay_manager.slot_for_side(cur_side);
                                if let Err(err) = slot.load_from_bytes(ui.ctx(), path, &bytes, bbox) {
                                    self.status_msg = format!("Failed to load dropped photo: {err}");
                                } else {
                                    self.status_msg = format!("Loaded dropped overlay photo for {cur_side:?}: {}", path.display());
                                    self.side_panel_state.active_tab = side_panel::Tab::Overlay;
                                }
                            }
                        }
                        "pdf" => {
                            if let Err(err) = self.pdf_viewer.open_file(path) {
                                self.status_msg = format!("Failed to load schematic PDF: {err}");
                            } else {
                                self.status_msg = format!("Loaded schematic PDF: {}", path.display());
                            }
                        }
                        _ => {
                            self.open_file_from_path(path);
                        }
                    }
                } else if let Ok(bytes) = file.bytes() {
                    self.load_file("dropped_board", &bytes);
                }
            }
        }

        // Handle global hotkeys
        if ui.input(|i| (i.modifiers.ctrl || i.modifiers.command) && i.key_pressed(egui::Key::Comma)) {
            self.preferences_open = !self.preferences_open;
        }

        // Actions shortcuts: R (Rotate CW), Shift+R (Rotate CCW), Space (Flip Side), M (Mirror X), Shift+M (Mirror Y), F (Fit), Esc (Deselect)
        if !self.preferences_open {
            let wants_kb = ui.ctx().egui_wants_keyboard_input() || ui.memory(|m| m.focused().is_some());
            let (r_pressed, shift_down, space_pressed, m_pressed, esc_pressed, enter_pressed, f_pressed) = ui.input(|i| {
                (
                    i.key_pressed(egui::Key::R),
                    i.modifiers.shift,
                    i.key_pressed(egui::Key::Space),
                    i.key_pressed(egui::Key::M),
                    i.key_pressed(egui::Key::Escape),
                    i.key_pressed(egui::Key::Enter),
                    i.key_pressed(egui::Key::F),
                )
            });

            if esc_pressed {
                if self.alignment_session.is_active() {
                    let cur_side = self.canvas_state.viewport.side;
                    let slot = self.overlay_manager.slot_for_side(cur_side);
                    if let Some(snap) = self.alignment_session.cancel() {
                        slot.restore_snapshot(snap);
                    }
                    self.status_msg = "Calibration cancelled and overlay restored.".to_string();
                    ui.ctx().request_repaint();
                } else if self.selected_pin_idx.is_some() || self.selected_net.is_some() || self.selected_part_idx.is_some() {
                    self.selected_pin_idx = None;
                    self.selected_part_idx = None;
                    self.selected_net = None;
                    self.status_msg = "Selection cleared.".to_string();
                    self.canvas_state.cached_view_key = None;
                    ui.ctx().request_repaint();
                }
            }

            if enter_pressed && self.alignment_session.is_active() {
                if let Some(Ok(ref res)) = self.alignment_session.live_result {
                    let cur_side = self.canvas_state.viewport.side;
                    let slot = self.overlay_manager.slot_for_side(cur_side);
                    let cur_rotation_rad = self.canvas_state.viewport.rotation.angle_rad();
                    let (cur_sx, cur_sy) = self.canvas_state.viewport.effective_factors();
                    slot.apply_alignment_result(
                        res,
                        self.alignment_session.auto_lock_on_apply,
                        cur_rotation_rad,
                        cur_sx,
                        cur_sy,
                    );
                    self.alignment_session.cancel();
                    self.status_msg = "Alignment applied to overlay.".to_string();
                    ui.ctx().request_repaint();
                }
            }

            if !wants_kb {
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

                if f_pressed {
                    if let Some(board) = &self.board {
                        if let Some(bbox) = board.bounding_box {
                            let rect = ui.clip_rect();
                            self.canvas_state.viewport.fit_to_rect(bbox, rect);
                            self.canvas_state.cached_view_key = None;
                            ui.ctx().request_repaint();
                            self.status_msg = "Viewport fitted to PCB bounds (F)".to_string();
                        }
                    }
                }
            }
        }

        let lang = self.preferences.language;

        // Cadcraft top toolbar
        Panel::top("top_bar").show(ui, |ui: &mut Ui| {
            ui.add_space(3.0);
            ui.horizontal(|ui: &mut Ui| {
                ui.heading(RichText::new("BoardViewCraft").strong().color(self.tokens.accent()));
                ui.separator();

                if ui.button(i18n::tr(lang, "toolbar.open_board")).clicked() {
                    self.open_file_dialog();
                }

                // Recent files dropdown
                let mut file_to_open = None;
                ui.menu_button(i18n::tr(lang, "toolbar.open_recent"), |ui| {
                    if self.preferences.recent_files.is_empty() {
                        ui.label(RichText::new(i18n::tr(lang, "toolbar.no_recent_files")).color(self.tokens.text_dim()));
                    } else {
                        for path in &self.preferences.recent_files {
                            let label = path.file_name().and_then(|n| n.to_str()).unwrap_or("board");
                            if ui.button(label).on_hover_text(path.display().to_string()).clicked() {
                                file_to_open = Some(path.clone());
                                ui.close();
                            }
                        }
                    }
                });

                if let Some(path) = file_to_open {
                    self.open_file_from_path(&path);
                }

                // Actions Menu dropdown
                ui.menu_button(i18n::tr(lang, "toolbar.actions"), |ui| {
                    if ui.button(i18n::tr(lang, "toolbar.load_photo")).clicked() {
                        let cur_side = self.canvas_state.viewport.side;
                        self.open_photo_dialog(ui.ctx(), cur_side);
                        ui.close();
                    }
                    if ui.button(if self.pdf_viewer.is_open { "Hide Schematic PDF" } else { "Open Schematic PDF..." }).clicked() {
                        if !self.pdf_viewer.is_open && self.pdf_viewer.path.is_none() {
                            self.open_pdf_dialog();
                        } else {
                            self.pdf_viewer.is_open = !self.pdf_viewer.is_open;
                        }
                        ui.close();
                    }
                    ui.separator();
                    if ui.button(i18n::tr(lang, "actions.rotate_cw")).clicked() {
                        self.canvas_state.viewport.rotation = self.canvas_state.viewport.rotation.rotate_cw();
                        ui.close();
                    }
                    if ui.button(i18n::tr(lang, "actions.rotate_ccw")).clicked() {
                        self.canvas_state.viewport.rotation = self.canvas_state.viewport.rotation.rotate_ccw();
                        ui.close();
                    }
                    if ui.button(i18n::tr(lang, "actions.rotate_180")).clicked() {
                        self.canvas_state.viewport.rotation = self.canvas_state.viewport.rotation.rotate_180();
                        ui.close();
                    }
                    ui.separator();
                    let is_top = self.canvas_state.viewport.side == Side::Top;
                    if ui.button(if is_top { i18n::tr(lang, "actions.flip_bottom") } else { i18n::tr(lang, "actions.flip_top") }).clicked() {
                        self.canvas_state.viewport.side = if is_top { Side::Bottom } else { Side::Top };
                        ui.close();
                    }
                    if ui.button(format!("{} [{}]", i18n::tr(lang, "actions.mirror_x"), if self.canvas_state.viewport.mirror_x { "ON" } else { "OFF" })).clicked() {
                        self.canvas_state.viewport.mirror_x = !self.canvas_state.viewport.mirror_x;
                        ui.close();
                    }
                    if ui.button(format!("{} [{}]", i18n::tr(lang, "actions.mirror_y"), if self.canvas_state.viewport.mirror_y { "ON" } else { "OFF" })).clicked() {
                        self.canvas_state.viewport.mirror_y = !self.canvas_state.viewport.mirror_y;
                        ui.close();
                    }
                    ui.separator();
                    if ui.button(i18n::tr(lang, "actions.reset")).clicked() {
                        self.canvas_state.viewport.rotation = canvas::Rotation::Deg0;
                        self.canvas_state.viewport.mirror_x = false;
                        self.canvas_state.viewport.mirror_y = false;
                        self.canvas_state.viewport.side = Side::Top;
                        ui.close();
                    }
                });

                if ui.button(i18n::tr(lang, "toolbar.preferences")).clicked() {
                    self.preferences_open = true;
                }

                ui.separator();

                // Quick flip side button
                let is_top = self.canvas_state.viewport.side == Side::Top;
                let toggle_label = if is_top { i18n::tr(lang, "toolbar.view_top") } else { i18n::tr(lang, "toolbar.view_bottom") };
                if ui.button(toggle_label).on_hover_text("Click or press Space to flip board side").clicked() {
                    self.canvas_state.viewport.side = if is_top { Side::Bottom } else { Side::Top };
                }

                // Quick rotation button
                let rot_label = format!("{}: {}", i18n::tr(lang, "toolbar.rotate"), self.canvas_state.viewport.rotation.display_str());
                if ui.button(rot_label).on_hover_text("Click or press R to rotate 90° CW").clicked() {
                    self.canvas_state.viewport.rotation = self.canvas_state.viewport.rotation.rotate_cw();
                }

                // Quick pin visibility toggle button
                let pins_status = if self.preferences.show_pins { "ON" } else { "OFF" };
                let pins_label = format!("{}: {}", i18n::tr(lang, "toolbar.pins"), pins_status);
                if ui.button(pins_label).on_hover_text("Toggle component pin/pad visibility").clicked() {
                    self.preferences.show_pins = !self.preferences.show_pins;
                    self.canvas_state.cached_view_key = None;
                    ui.ctx().request_repaint();
                }

                // Quick via visibility toggle button
                let vias_status = if self.preferences.show_vias { "ON" } else { "OFF" };
                let vias_label = format!("{}: {}", i18n::tr(lang, "toolbar.vias"), vias_status);
                if ui.button(vias_label).on_hover_text("Toggle PCB via visibility").clicked() {
                    self.preferences.show_vias = !self.preferences.show_vias;
                    self.canvas_state.cached_view_key = None;
                    ui.ctx().request_repaint();
                }

                // Zoom dropdown / presets
                ui.menu_button(i18n::tr(lang, "toolbar.zoom"), |ui| {
                    let rect = ui.clip_rect();
                    if ui.button(i18n::tr(lang, "toolbar.zoom_fit_window")).clicked() {
                        if let Some(board) = &self.board {
                            if let Some(bbox) = board.bounding_box {
                                self.canvas_state.viewport.fit_to_rect(bbox, rect);
                                self.canvas_state.cached_view_key = None;
                                ui.ctx().request_repaint();
                            }
                        }
                        ui.close();
                    }
                    ui.separator();
                    if ui.button(i18n::tr(lang, "toolbar.zoom_preset_25")).clicked() {
                        self.canvas_state.viewport.zoom_preset(0.25, rect);
                        ui.close();
                    }
                    if ui.button(i18n::tr(lang, "toolbar.zoom_preset_50")).clicked() {
                        self.canvas_state.viewport.zoom_preset(0.50, rect);
                        ui.close();
                    }
                    if ui.button(i18n::tr(lang, "toolbar.zoom_preset_100")).clicked() {
                        self.canvas_state.viewport.zoom_preset(1.0, rect);
                        ui.close();
                    }
                    if ui.button(i18n::tr(lang, "toolbar.zoom_preset_150")).clicked() {
                        self.canvas_state.viewport.zoom_preset(1.50, rect);
                        ui.close();
                    }
                    if ui.button(i18n::tr(lang, "toolbar.zoom_preset_200")).clicked() {
                        self.canvas_state.viewport.zoom_preset(2.0, rect);
                        ui.close();
                    }
                });

                if let Some(name) = &self.filename {
                    ui.separator();
                    ui.label(RichText::new(name).color(self.tokens.text_dim()));
                }
            });
            ui.add_space(3.0);
        });

        // Bottom status bar
        Panel::bottom("status_bar").show(ui, |ui: &mut Ui| {
            ui.add_space(2.0);
            ui.horizontal(|ui: &mut Ui| {
                ui.label(RichText::new(&self.status_msg).color(self.tokens.status_bar_text()));

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
                    ui.label(RichText::new(format!("[{side_str} | Rot: {rot_str}{mirror_str}] Scale: {:.4}", self.canvas_state.viewport.scale)).color(self.tokens.text_dim()));
                });
            });
            ui.add_space(2.0);
        });

        // Right side details / search panel
        let mut on_load_photo: Option<Side> = None;
        let cur_rotation_rad = self.canvas_state.viewport.rotation.angle_rad();
        let (cur_sx, cur_sy) = self.canvas_state.viewport.effective_factors();
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
                    &mut self.overlay_manager,
                    &mut self.alignment_session,
                    &mut self.selected_pin_idx,
                    &mut self.selected_via_idx,
                    &mut self.selected_part_idx,
                    &mut self.selected_net,
                    self.canvas_state.viewport.side,
                    cur_rotation_rad,
                    cur_sx,
                    cur_sy,
                    self.preferences.language,
                    &mut on_load_photo,
                );
            });

        if let Some(side) = on_load_photo {
            self.open_photo_dialog(ui.ctx(), side);
        }

        // Automatic cross-probing: sync with PDF schematic ONLY when a component is explicitly selected
        if self.pdf_viewer.is_open && self.selected_pin_idx.is_none() {
            if let Some(part_idx) = self.selected_part_idx {
                if let Some(board) = &self.board {
                    if let Some(part) = board.parts.get(part_idx) {
                        if self.pdf_viewer.active_search != part.name {
                            self.pdf_viewer.cross_probe(&part.name);
                        }
                    }
                }
            }
        }

        // Schematic PDF split panel (if open)
        if self.pdf_viewer.is_open {
            Panel::left("pdf_schematic_panel")
                .resizable(true)
                .default_size(ui.available_width() * 0.45)
                .size_range(280.0..=ui.available_width() * 0.75)
                .show(ui, |ui: &mut Ui| {
                    ui.horizontal(|ui| {
                        ui.heading(RichText::new("Schematic PDF").strong().color(self.tokens.accent()));
                        if let Some(fn_str) = &self.pdf_viewer.filename {
                            ui.label(RichText::new(fn_str).small().color(self.tokens.text_dim()));
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("✖").on_hover_text("Close Schematic PDF").clicked() {
                                self.pdf_viewer.is_open = false;
                            }
                        });
                    });
                    ui.separator();

                    // Navigation bar
                    ui.horizontal(|ui| {
                        if ui.button("⏴").on_hover_text("Previous page").clicked() && self.pdf_viewer.current_page > 1 {
                            self.pdf_viewer.current_page -= 1;
                        }
                        ui.label(format!("Page {} of {}", self.pdf_viewer.current_page, self.pdf_viewer.total_pages));
                        if ui.button("⏵").on_hover_text("Next page").clicked() && self.pdf_viewer.current_page < self.pdf_viewer.total_pages {
                            self.pdf_viewer.current_page += 1;
                        }

                        if !self.pdf_viewer.search_results.is_empty() {
                            ui.separator();
                            let count = self.pdf_viewer.search_results.len();
                            let current = self.pdf_viewer.selected_result_idx + 1;
                            ui.label(RichText::new(format!("{current}/{count} hits")).color(self.tokens.accent()));
                            if ui.button("Prev").clicked() {
                                self.pdf_viewer.prev_match();
                            }
                            if ui.button("Next").clicked() {
                                self.pdf_viewer.next_match();
                            }
                        }
                    });
                    ui.separator();

                    // PDF Page viewer canvas area with interactive pan and zoom
                    let (rect, resp) = ui.allocate_exact_size(ui.available_size(), egui::Sense::click_and_drag());
                    ui.painter().rect_filled(rect, 4.0, Color32::from_rgb(255, 255, 255));

                    if resp.dragged() {
                        self.pdf_viewer.pan_offset += resp.drag_delta();
                    }

                    // Mouse wheel zoom when hovering over the PDF schematic canvas
                    if resp.hovered() {
                        let scroll_y = ui.input(|i| i.smooth_scroll_delta.y);
                        if scroll_y != 0.0 {
                            let zoom_factor = if scroll_y > 0.0 { 1.15 } else { 1.0 / 1.15 };
                            self.pdf_viewer.zoom = (self.pdf_viewer.zoom * zoom_factor).clamp(0.2, 20.0);
                            ui.ctx().request_repaint();
                        }

                        // Keyboard navigation inside PDF panel when hovered or focused
                        let (page_up, page_down, arrow_left, arrow_right, plus_key, minus_key) = ui.input(|i| {
                            (
                                i.key_pressed(egui::Key::PageUp),
                                i.key_pressed(egui::Key::PageDown),
                                i.key_pressed(egui::Key::ArrowLeft),
                                i.key_pressed(egui::Key::ArrowRight),
                                i.key_pressed(egui::Key::Plus) || i.key_pressed(egui::Key::Equals),
                                i.key_pressed(egui::Key::Minus),
                            )
                        });

                        if (page_up || arrow_left) && self.pdf_viewer.current_page > 1 {
                            self.pdf_viewer.current_page -= 1;
                            ui.ctx().request_repaint();
                        } else if (page_down || arrow_right) && self.pdf_viewer.current_page < self.pdf_viewer.total_pages {
                            self.pdf_viewer.current_page += 1;
                            ui.ctx().request_repaint();
                        }

                        if plus_key {
                            self.pdf_viewer.zoom = (self.pdf_viewer.zoom * 1.25).clamp(0.2, 20.0);
                            ui.ctx().request_repaint();
                        } else if minus_key {
                            self.pdf_viewer.zoom = (self.pdf_viewer.zoom / 1.25).clamp(0.2, 20.0);
                            ui.ctx().request_repaint();
                        }
                    }

                    // Render crisp rasterized PDF page texture from engine
                    if let Some((texture, orig_dim)) = self.pdf_viewer.ensure_current_page_texture(ui.ctx()) {
                        let scale_x = (rect.width() * 0.95) / orig_dim.x.max(1.0);
                        let scale_y = (rect.height() * 0.95) / orig_dim.y.max(1.0);
                        let auto_scale = scale_x.min(scale_y);
                        let effective_scale = auto_scale * self.pdf_viewer.zoom;
                        let render_size = orig_dim * effective_scale;

                        let render_pos = rect.min
                            + (rect.size() - render_size) * 0.5
                            + self.pdf_viewer.pan_offset;

                        let img_rect = Rect::from_min_size(render_pos, render_size);
                        ui.painter().image(
                            texture.id(),
                            img_rect,
                            Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                            Color32::WHITE,
                        );

                        // PDF coordinate mapping function (PDF points -> screen points on raster)
                        let page_size_pts = self.pdf_viewer.index.page_sizes.get(&self.pdf_viewer.current_page).copied().unwrap_or(egui::vec2(1224.0, 792.0));
                        let to_screen = |pdf_pt: Pos2| -> Pos2 {
                            let scale_x = orig_dim.x / page_size_pts.x.max(1.0);
                            let scale_y = orig_dim.y / page_size_pts.y.max(1.0);
                            let rx = pdf_pt.x * scale_x * effective_scale;
                            let ry = pdf_pt.y * scale_y * effective_scale;
                            render_pos + Vec2::new(rx, ry)
                        };

                        // Cross-probing highlights
                        for (i, hit) in self.pdf_viewer.search_results.iter().enumerate() {
                            if hit.page_number == self.pdf_viewer.current_page {
                                let top_left = to_screen(hit.bbox.min);
                                let btm_right = to_screen(hit.bbox.max);
                                let marker_rect = Rect::from_min_max(top_left, btm_right);

                                let is_selected = i == self.pdf_viewer.selected_result_idx;
                                let bg_color = if is_selected {
                                    Color32::from_rgba_unmultiplied(255, 230, 0, 160)
                                } else {
                                    Color32::from_rgba_unmultiplied(100, 180, 255, 120)
                                };

                                ui.painter().rect_filled(marker_rect, 2.0, bg_color);
                                ui.painter().rect_stroke(
                                    marker_rect,
                                    2.0,
                                    egui::Stroke::new(2.0, Color32::from_rgb(220, 20, 40)),
                                    egui::StrokeKind::Middle,
                                );
                            }
                        }
                    } else {
                        let page_label = format!("Loading Schematic Page {}...", self.pdf_viewer.current_page);
                        ui.painter().text(
                            rect.center(),
                            egui::Align2::CENTER_CENTER,
                            page_label,
                            egui::FontId::proportional(16.0),
                            Color32::from_rgb(100, 100, 110),
                        );
                    }
                });
        }

        // Central Canvas Panel
        CentralPanel::default()
            .frame(egui::Frame::NONE.fill(self.tokens.canvas()))
            .show(ui, |ui: &mut Ui| {
                let overlay_mode_active = self.side_panel_state.active_tab == side_panel::Tab::Overlay;
                let response = show_canvas(
                    ui,
                    &self.tokens,
                    self.board.as_ref(),
                    &mut self.canvas_state,
                    &mut self.overlay_manager,
                    &mut self.alignment_session,
                    overlay_mode_active,
                    &mut self.selected_pin_idx,
                    &mut self.selected_via_idx,
                    &mut self.selected_part_idx,
                    &mut self.selected_net,
                    &mut self.renderer,
                    &self.preferences,
                );

                if self.pending_auto_fit {
                    let rect = response.rect;
                    if rect.width() > 10.0 && rect.height() > 10.0 {
                        if let Some(board) = &self.board {
                            if let Some(bbox) = board.bounding_box {
                                self.canvas_state.viewport.fit_to_rect(bbox, rect);
                                self.canvas_state.cached_view_key = None;
                                ui.ctx().request_repaint();
                            }
                        }
                        self.pending_auto_fit = false;
                    }
                }
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
            if prefs.app_theme != self.preferences.app_theme
                || prefs.pcb_theme != self.preferences.pcb_theme
                || prefs.custom_pcb_tokens != self.preferences.custom_pcb_tokens
            {
                self.tokens = Tokens::new_with_custom(
                    prefs.app_theme,
                    prefs.pcb_theme,
                    prefs.custom_pcb_tokens,
                );
                let is_light = prefs.app_theme == AppTheme::Light;
                ui.ctx().set_visuals(self.tokens.egui_visuals(is_light));
            }
            // Invalidate canvas mesh cache
            self.canvas_state.cached_view_key = None;
            self.preferences = prefs;
            let _ = self.preferences.save_atomic();
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
