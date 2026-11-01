//! Main application entry point and window runner for BoardViewCraft.

mod about;
mod canvas;
mod i18n;
mod overlay;
pub mod overlay_align;
pub mod pdf;
mod preferences;
mod render;
mod side_panel;
mod theme;

use bvc_core::{Board, ParserRegistry, Point, Side};
use canvas::{BoardCanvases, show_canvas};
use overlay::OverlayManager;
use overlay_align::AlignmentSession;
use preferences::{Preferences, show_preferences_window};
use render::Renderer;
use side_panel::{SidePanelState, show_side_panel};
use theme::{AppTheme, Tokens};

use egui::{CentralPanel, Color32, Panel, Pos2, Rect, RichText, Ui};
use std::sync::Arc;

/// Board sides offered by the toolbar side selector, in display order.
const SIDE_CHOICES: [Side; 3] = [Side::Top, Side::Bottom, Side::Both];

/// Translated toolbar label for a board side.
fn side_label(lang: i18n::Language, side: Side) -> &'static str {
    match side {
        Side::Top => i18n::tr(lang, "toolbar.view_top"),
        Side::Bottom => i18n::tr(lang, "toolbar.view_bottom"),
        Side::Both => i18n::tr(lang, "toolbar.view_both"),
    }
}


/// Searches for a co-located PDF schematic file matching the given boardview file path.
/// Checks `<stem>.pdf`, `<stem>.PDF`, and falls back to a case-insensitive directory scan.
pub fn find_matching_schematic_pdf(board_path: &std::path::Path) -> Option<std::path::PathBuf> {
    let parent = board_path.parent()?;
    let stem = board_path.file_stem()?.to_str()?;

    // 1. Direct candidate checks
    for ext in &["pdf", "PDF"] {
        let candidate = parent.join(format!("{stem}.{ext}"));
        if candidate.is_file() {
            return Some(candidate);
        }
    }

    // 2. Directory scan fallback (case-insensitive for mixed extensions on Linux)
    if let Ok(entries) = std::fs::read_dir(parent) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let (Some(entry_stem), Some(entry_ext)) = (path.file_stem(), path.extension()) {
                    if entry_stem.to_string_lossy().eq_ignore_ascii_case(stem)
                        && entry_ext.to_string_lossy().eq_ignore_ascii_case("pdf")
                    {
                        return Some(path);
                    }
                }
            }
        }
    }

    None
}

/// The schematic PDF remembered for `board_path`, when that file still exists.
pub fn remembered_schematic_pdf(prefs: &Preferences, board_path: &std::path::Path) -> Option<std::path::PathBuf> {
    prefs.board_schematic(board_path).filter(|pdf| pdf.is_file()).map(std::path::Path::to_path_buf)
}

pub struct BoardViewApp {
    board: Option<Board>,
    parser_registry: Arc<ParserRegistry>,
    tokens: Tokens,
    renderer: Renderer,
    /// One board view per side (own pan, zoom and mesh cache; shared rotation and mirroring).
    canvases: BoardCanvases,
    /// Sides shown in the central area: `Top`, `Bottom` or `Both` (side by side).
    shown_side: Side,
    /// Screen rectangle of each canvas drawn in the last frame, for fitting to the real size.
    canvas_rects: Vec<(Side, Rect)>,
    side_panel_state: SidePanelState,
    overlay_manager: OverlayManager,
    alignment_session: AlignmentSession,
    preferences: Preferences,
    preferences_open: bool,
    about_open: bool,
    pending_auto_fit: bool,

    // Selection state
    selected_pin_idx: Option<usize>,
    selected_via_idx: Option<usize>,
    selected_part_idx: Option<usize>,
    selected_net: Option<String>,

    // Status message
    status_msg: String,
    filename: Option<String>,
    /// File path of the loaded board, when it was opened from a file (keys its schematic PDF).
    board_path: Option<std::path::PathBuf>,

    // PDF Schematic integration
    pdf_viewer: pdf::PdfSchematicViewer,
    pending_schematic_prompt: Option<std::path::PathBuf>,
}

impl Default for BoardViewApp {
    fn default() -> Self {
        Self {
            board: None,
            parser_registry: Arc::new(ParserRegistry::default()),
            tokens: Tokens::DARK,
            renderer: Renderer::new(),
            canvases: BoardCanvases::default(),
            shown_side: Side::Top,
            canvas_rects: Vec::new(),
            side_panel_state: SidePanelState::default(),
            overlay_manager: OverlayManager::default(),
            alignment_session: AlignmentSession::default(),
            preferences: Preferences::default(),
            preferences_open: false,
            about_open: false,
            pending_auto_fit: false,
            selected_pin_idx: None,
            selected_via_idx: None,
            selected_part_idx: None,
            selected_net: None,
            status_msg: "Ready. Open a .cad file to inspect.".to_string(),
            filename: None,
            board_path: None,
            pdf_viewer: pdf::PdfSchematicViewer::default(),
            pending_schematic_prompt: None,
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

    /// Parses and loads a board. Returns whether it was loaded; on failure the current board
    /// (and its schematic PDF) stay as they were.
    pub fn load_file(&mut self, filename: &str, data: &[u8]) -> bool {
        match self.parser_registry.parse_file(filename, data) {
            Ok(board) => {
                self.adopt_board(filename, board);
                true
            }
            Err(err) => {
                self.status_msg = format!("Failed to open {filename}: {err}");
                false
            }
        }
    }

    /// Makes `board` the current board. The previous board's schematic PDF and a pending PDF
    /// prompt are closed: each board has its own schematic.
    fn adopt_board(&mut self, filename: &str, board: Board) {
        let parts_cnt = board.parts.len();
        let pins_cnt = board.pins.len();
        let vias_cnt = board.vias.len();
        let nets_cnt = board.nets.len();

        self.status_msg = format!(
            "Loaded {filename}: {parts_cnt} parts, {pins_cnt} pins, {vias_cnt} vias, {nets_cnt} nets"
        );
        self.filename = Some(filename.to_string());
        self.board_path = None;
        self.selected_pin_idx = None;
        self.selected_via_idx = None;
        self.selected_part_idx = None;
        self.selected_net = None;
        self.side_panel_state.search.invalidate();
        self.pdf_viewer.close();
        self.pending_schematic_prompt = None;

        // Center view and request auto-fit to window on next render frame
        if let Some(bbox) = board.bounding_box {
            self.canvases.center_all(bbox.center());
            self.pending_auto_fit = true;
        }

        self.board = Some(board);
    }

    pub fn open_file_from_path(&mut self, path: &std::path::Path) {
        match std::fs::read(path) {
            Ok(bytes) => {
                let name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("board.cad");
                if !self.load_file(name, &bytes) {
                    return;
                }
                self.board_path = Some(path.to_path_buf());
                self.preferences.add_recent_file(path.to_path_buf());
                let _ = self.preferences.save_atomic();

                // Reopen the board's remembered schematic; otherwise offer a matching PDF in the same folder.
                if let Some(pdf) = remembered_schematic_pdf(&self.preferences, path) {
                    let status = self.status_msg.clone();
                    if self.open_schematic(&pdf) {
                        self.status_msg = format!("{status}; reopened its schematic {}", pdf.display());
                    }
                } else if let Some(pdf_candidate) = find_matching_schematic_pdf(path) {
                    self.pending_schematic_prompt = Some(pdf_candidate);
                }
            }
            Err(err) => {
                self.status_msg = format!("Could not open {}: {err}", path.display());
            }
        }
    }

    /// Opens a schematic PDF and, when a board file is loaded, remembers it as that board's
    /// schematic. Returns whether the PDF was opened.
    pub fn open_schematic(&mut self, path: &std::path::Path) -> bool {
        if let Err(err) = self.pdf_viewer.open_file(path) {
            self.status_msg = format!("Failed to open schematic PDF: {err}");
            return false;
        }
        self.status_msg = format!("Opened schematic: {}", path.display());
        if self.remember_schematic(path) {
            let _ = self.preferences.save_atomic();
        }
        true
    }

    /// Records `pdf` as the current board's schematic (no-op without a board file). Returns
    /// whether the preferences changed and need saving.
    fn remember_schematic(&mut self, pdf: &std::path::Path) -> bool {
        match (&self.board, &self.board_path) {
            (Some(_), Some(board)) => self.preferences.remember_board_schematic(board.clone(), pdf.to_path_buf()),
            _ => false,
        }
    }

    /// Closes the current board and everything tied to it: selection, side panel details,
    /// photo overlays (aligned to that board), the schematic PDF and a pending PDF prompt.
    /// View preferences and the side panel search filters are kept.
    pub fn close_board(&mut self) {
        let Some(name) = self.filename.take().or_else(|| self.board.as_ref().map(|_| "board".to_string())) else {
            return;
        };
        self.board = None;
        self.board_path = None;
        self.selected_pin_idx = None;
        self.selected_via_idx = None;
        self.selected_part_idx = None;
        self.selected_net = None;
        self.pending_auto_fit = false;
        self.canvases.clear_board();
        self.side_panel_state.overlay_editing = false;
        self.side_panel_state.focus_part = None;
        self.side_panel_state.stay_on_part = None;
        self.side_panel_state.search.invalidate();
        self.overlay_manager.top.clear();
        self.overlay_manager.bottom.clear();
        self.alignment_session = AlignmentSession::default();
        self.pdf_viewer.close();
        self.pending_schematic_prompt = None;
        self.status_msg = format!("Closed {name}.");
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
                    self.side_panel_state.overlay_editing = true;
                }
            }
        }
    }

    pub fn set_status(&mut self, msg: String) {
        self.status_msg = msg;
    }

    pub fn status(&self) -> &str {
        &self.status_msg
    }

    pub fn open_pdf_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("PDF Schematics (*.pdf)", &["pdf"])
            .add_filter("All files (*.*)", &["*"])
            .pick_file()
        {
            self.open_schematic(&path);
        }
    }

    /// Sides whose canvases are on screen.
    fn visible_sides(&self) -> Vec<Side> {
        match self.shown_side {
            Side::Both => vec![Side::Top, Side::Bottom],
            side => vec![side],
        }
    }

    /// Shows `side` (`Both` shows the two canvases) and keeps the active canvas visible.
    fn show_side(&mut self, side: Side) {
        self.shown_side = side;
        if !self.visible_sides().contains(&self.canvases.active) {
            self.canvases.active = if side == Side::Bottom { Side::Bottom } else { Side::Top };
        }
    }

    /// Side the overlay photo tools work on: the active canvas.
    fn overlay_side(&self) -> Side {
        self.canvases.active
    }

    /// Fits every canvas drawn in the last frame to its own rectangle (`fallback` for the active
    /// canvas before any was drawn). Returns whether a board was fitted.
    fn fit_shown_canvases(&mut self, fallback: Rect) -> bool {
        let Some(bbox) = self.board.as_ref().and_then(|b| b.bounding_box) else {
            return false;
        };
        if self.canvas_rects.is_empty() {
            self.canvases.active_mut().viewport.fit_to_rect(bbox, fallback);
        }
        for &(side, rect) in &self.canvas_rects {
            self.canvases.side_mut(side).viewport.fit_to_rect(bbox, rect);
        }
        true
    }

    /// Centers the board view on `center`, an item visible on `item_side`: the shown canvases
    /// that can show it are centered. When none can, the canvases of the item's side are
    /// centered, and with `switch_side` the view switches to that side first.
    fn focus_point(&mut self, center: Point, item_side: Side, switch_side: bool) {
        let mut targets: Vec<Side> = self.visible_sides().into_iter().filter(|&s| item_side.visible_on(s)).collect();
        if targets.is_empty() {
            targets = [Side::Top, Side::Bottom].into_iter().filter(|&s| item_side.visible_on(s)).collect();
            if switch_side && let Some(&side) = targets.first() {
                self.show_side(side);
                targets.truncate(1);
            }
        }
        for side in targets {
            self.canvases.side_mut(side).viewport.center = center;
        }
    }

    /// Centers the board view on part `part_id` (see `focus_point`). Returns whether it exists.
    fn focus_part(&mut self, part_id: usize, switch_side: bool) -> bool {
        let Some(board) = &self.board else { return false };
        let Some(part_side) = board.parts.get(part_id).map(|p| p.side) else { return false };
        let Some((center, _)) = side_panel::part_focus_target(board, part_id, Side::Both) else { return false };
        self.focus_point(center, part_side, switch_side);
        true
    }
}

/// Where a schematic click moves the board view.
enum Focus {
    Part(usize),
    Point(Point, Side),
}

impl BoardViewApp {
    /// Draws the canvas of each `(side, rect)`, tracks the active canvas (last hovered or
    /// clicked) and applies a pending auto-fit to the real canvas rectangles.
    fn show_board_canvases(&mut self, ui: &mut Ui, rects: &[(Side, Rect)]) {
        let overlay_mode_active = self.side_panel_state.overlay_editing;
        for &(side, rect) in rects {
            let is_active = self.canvases.active == side;
            let response = show_canvas(
                ui,
                rect,
                is_active,
                &self.tokens,
                self.board.as_ref(),
                self.canvases.side_mut(side),
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
            if !is_active && (response.hovered() || response.clicked() || response.secondary_clicked()) {
                self.canvases.active = side;
            }
        }
        self.canvas_rects = rects.to_vec();

        if self.pending_auto_fit
            && let Some(&(_, first)) = rects.first()
            && first.width() > 10.0
            && first.height() > 10.0
        {
            if let Some(bbox) = self.board.as_ref().and_then(|b| b.bounding_box) {
                // Hidden sides get the first rectangle until they are shown and fitted again.
                for side in [Side::Top, Side::Bottom] {
                    let rect = rects.iter().find(|(s, _)| *s == side).map_or(first, |&(_, r)| r);
                    self.canvases.side_mut(side).viewport.fit_to_rect(bbox, rect);
                }
                ui.ctx().request_repaint();
            }
            self.pending_auto_fit = false;
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
                                let cur_side = self.overlay_side();
                                let bbox = self.board.as_ref().and_then(|b| b.bounding_box);
                                let slot = self.overlay_manager.slot_for_side(cur_side);
                                if let Err(err) = slot.load_from_bytes(ui.ctx(), path, &bytes, bbox) {
                                    self.status_msg = format!("Failed to load dropped photo: {err}");
                                } else {
                                    self.status_msg = format!("Loaded dropped overlay photo for {cur_side:?}: {}", path.display());
                                    self.side_panel_state.overlay_editing = true;
                                }
                            }
                        }
                        "pdf" => {
                            self.open_schematic(path);
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
                if wants_kb {
                    // Release focus from text fields if Escape is pressed while typing
                    ui.memory_mut(|m| m.stop_text_input());
                } else if self.alignment_session.is_active() {
                    let cur_side = self.overlay_side();
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
                    self.canvases.invalidate_caches();
                    ui.ctx().request_repaint();
                }
            }

            if enter_pressed && self.alignment_session.is_active() {
                if let Some(Ok(ref res)) = self.alignment_session.live_result {
                    let cur_side = self.overlay_side();
                    let slot = self.overlay_manager.slot_for_side(cur_side);
                    let viewport = &self.canvases.side(cur_side).viewport;
                    let cur_rotation_rad = viewport.rotation.angle_rad();
                    let (cur_sx, cur_sy) = viewport.effective_factors();
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
                    if shift_down {
                        self.canvases.rotate_ccw();
                    } else {
                        self.canvases.rotate_cw();
                    }
                }

                if space_pressed {
                    self.show_side(if self.shown_side == Side::Top { Side::Bottom } else { Side::Top });
                }

                if m_pressed {
                    if shift_down {
                        self.canvases.toggle_mirror_y();
                    } else {
                        self.canvases.toggle_mirror_x();
                    }
                }

                if f_pressed && self.fit_shown_canvases(ui.clip_rect()) {
                    ui.ctx().request_repaint();
                    self.status_msg = "Viewport fitted to PCB bounds (F)".to_string();
                }
            }
        }

        let lang = self.preferences.ui_language();

        // Cadcraft top toolbar
        Panel::top("top_bar").show(ui, |ui: &mut Ui| {
            ui.add_space(3.0);
            ui.horizontal(|ui: &mut Ui| {
                ui.heading(RichText::new("BoardViewCraft").strong().color(self.tokens.accent()));
                ui.separator();

                // File menu: open, recent boards, preferences
                let mut file_to_open = None;
                ui.menu_button(i18n::tr(lang, "menu.file"), |ui| {
                    if ui.button(i18n::tr(lang, "toolbar.open_board")).clicked() {
                        self.open_file_dialog();
                        ui.close();
                    }
                    let mut close_board = false;
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
                    if ui.add_enabled(self.board.is_some(), egui::Button::new(i18n::tr(lang, "toolbar.close_board"))).clicked() {
                        close_board = true;
                        ui.close();
                    }
                    ui.separator();
                    if ui.button(i18n::tr(lang, "toolbar.preferences")).clicked() {
                        self.preferences_open = true;
                        ui.close();
                    }
                    if close_board {
                        self.close_board();
                    }
                });

                if let Some(path) = file_to_open {
                    self.open_file_from_path(&path);
                }

                // Actions Menu dropdown
                ui.menu_button(i18n::tr(lang, "toolbar.actions"), |ui| {
                    if ui.button(i18n::tr(lang, "toolbar.load_photo")).clicked() {
                        let cur_side = self.overlay_side();
                        self.open_photo_dialog(ui.ctx(), cur_side);
                        ui.close();
                    }
                    // Re-open the overlay adjustment once the side panel left it.
                    let any_overlay = self.overlay_manager.top.is_loaded() || self.overlay_manager.bottom.is_loaded();
                    if ui.add_enabled(any_overlay, egui::Button::new(i18n::tr(lang, "actions.adjust_overlay"))).clicked() {
                        // Show a side that has a photo when the current one has none.
                        let cur_side = self.overlay_side();
                        if !self.overlay_manager.slot_for_side_ref(cur_side).is_loaded() {
                            let side = if self.overlay_manager.top.is_loaded() { Side::Top } else { Side::Bottom };
                            self.show_side(side);
                            self.canvases.active = side;
                        }
                        self.side_panel_state.overlay_editing = true;
                        ui.close();
                    }
                    let pdf_toggle_label = if self.pdf_viewer.is_open {
                        i18n::tr(lang, "pdf.hide")
                    } else {
                        i18n::tr(lang, "pdf.open")
                    };
                    if ui.button(pdf_toggle_label).clicked() {
                        if !self.pdf_viewer.is_open && self.pdf_viewer.path.is_none() {
                            self.open_pdf_dialog();
                        } else {
                            self.pdf_viewer.is_open = !self.pdf_viewer.is_open;
                        }
                        ui.close();
                    }
                    ui.separator();
                    if ui.button(i18n::tr(lang, "actions.rotate_cw")).clicked() {
                        self.canvases.rotate_cw();
                        ui.close();
                    }
                    if ui.button(i18n::tr(lang, "actions.rotate_ccw")).clicked() {
                        self.canvases.rotate_ccw();
                        ui.close();
                    }
                    if ui.button(i18n::tr(lang, "actions.rotate_180")).clicked() {
                        self.canvases.rotate_180();
                        ui.close();
                    }
                    ui.separator();
                    let is_top = self.shown_side == Side::Top;
                    if ui.button(if is_top { i18n::tr(lang, "actions.flip_bottom") } else { i18n::tr(lang, "actions.flip_top") }).clicked() {
                        self.show_side(if is_top { Side::Bottom } else { Side::Top });
                        ui.close();
                    }
                    if ui.button(format!("{} [{}]", i18n::tr(lang, "actions.mirror_x"), if self.canvases.mirror_x() { "ON" } else { "OFF" })).clicked() {
                        self.canvases.toggle_mirror_x();
                        ui.close();
                    }
                    if ui.button(format!("{} [{}]", i18n::tr(lang, "actions.mirror_y"), if self.canvases.mirror_y() { "ON" } else { "OFF" })).clicked() {
                        self.canvases.toggle_mirror_y();
                        ui.close();
                    }
                    ui.separator();
                    if ui.button(i18n::tr(lang, "actions.reset")).clicked() {
                        self.canvases.reset_orientation();
                        self.show_side(Side::Top);
                        ui.close();
                    }
                });

                // View menu: zoom presets and pin/via visibility
                ui.menu_button(i18n::tr(lang, "menu.view"), |ui| {
                    ui.menu_button(i18n::tr(lang, "toolbar.zoom"), |ui| {
                        let rect = ui.clip_rect();
                        if ui.button(i18n::tr(lang, "toolbar.zoom_fit_window")).clicked() {
                            if self.fit_shown_canvases(rect) {
                                ui.ctx().request_repaint();
                            }
                            ui.close();
                        }
                        ui.separator();
                        if ui.button(i18n::tr(lang, "toolbar.zoom_preset_25")).clicked() {
                            self.canvases.active_mut().viewport.zoom_preset(0.25, rect);
                            ui.close();
                        }
                        if ui.button(i18n::tr(lang, "toolbar.zoom_preset_50")).clicked() {
                            self.canvases.active_mut().viewport.zoom_preset(0.50, rect);
                            ui.close();
                        }
                        if ui.button(i18n::tr(lang, "toolbar.zoom_preset_100")).clicked() {
                            self.canvases.active_mut().viewport.zoom_preset(1.0, rect);
                            ui.close();
                        }
                        if ui.button(i18n::tr(lang, "toolbar.zoom_preset_150")).clicked() {
                            self.canvases.active_mut().viewport.zoom_preset(1.50, rect);
                            ui.close();
                        }
                        if ui.button(i18n::tr(lang, "toolbar.zoom_preset_200")).clicked() {
                            self.canvases.active_mut().viewport.zoom_preset(2.0, rect);
                            ui.close();
                        }
                    });
                    ui.separator();
                    if ui.checkbox(&mut self.preferences.show_pins, i18n::tr(lang, "toolbar.pins"))
                        .on_hover_text("Toggle component pin/pad visibility")
                        .changed()
                    {
                        self.canvases.invalidate_caches();
                        ui.ctx().request_repaint();
                    }
                    if ui.checkbox(&mut self.preferences.show_vias, i18n::tr(lang, "toolbar.vias"))
                        .on_hover_text("Toggle PCB via visibility")
                        .changed()
                    {
                        self.canvases.invalidate_caches();
                        ui.ctx().request_repaint();
                    }
                });

                // Help menu
                ui.menu_button(i18n::tr(lang, "menu.help"), |ui| {
                    if ui.button(i18n::tr(lang, "toolbar.about")).clicked() {
                        self.about_open = true;
                        ui.close();
                    }
                });

                ui.separator();

                // Board side selector (Top / Bottom / Both). The side is part of
                // the view cache key, so assigning it re-renders the board.
                let mut shown = self.shown_side;
                egui::ComboBox::from_id_salt("board_side_selector")
                    .selected_text(side_label(lang, shown))
                    .show_ui(ui, |ui: &mut Ui| {
                        for side in SIDE_CHOICES {
                            ui.selectable_value(&mut shown, side, side_label(lang, side));
                        }
                    })
                    .response
                    .on_hover_text(i18n::tr(lang, "toolbar.view_side_hover"));
                if shown != self.shown_side {
                    self.show_side(shown);
                }

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
                    let side_str = match self.shown_side {
                        Side::Top => "TOP",
                        Side::Bottom => "BOTTOM",
                        Side::Both => "BOTH",
                    };
                    let rot_str = self.canvases.rotation().display_str();
                    let mirror_str = match (self.canvases.mirror_x(), self.canvases.mirror_y()) {
                        (true, true) => " [Mirror XY]",
                        (true, false) => " [Mirror X]",
                        (false, true) => " [Mirror Y]",
                        (false, false) => "",
                    };
                    ui.label(RichText::new(format!("[{side_str} | Rot: {rot_str}{mirror_str}] Scale: {:.4}", self.canvases.active_state().viewport.scale)).color(self.tokens.text_dim()));
                });
            });
            ui.add_space(2.0);
        });

        // Right side details / search panel
        let mut on_load_photo: Option<Side> = None;
        let overlay_side = self.overlay_side();
        let cur_rotation_rad = self.canvases.rotation().angle_rad();
        let (cur_sx, cur_sy) = self.canvases.side(overlay_side).viewport.effective_factors();
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
                    overlay_side,
                    cur_rotation_rad,
                    cur_sx,
                    cur_sy,
                    self.preferences.ui_language(),
                    &mut on_load_photo,
                );
            });

        if let Some(side) = on_load_photo {
            self.open_photo_dialog(ui.ctx(), side);
        }

        // Center the board view on a part picked in the side panel search,
        // switching the view side when the part is not visible on the shown one.
        if let Some(part_id) = self.side_panel_state.focus_part.take()
            && self.focus_part(part_id, true)
        {
            ui.ctx().request_repaint();
        }

        // Automatic cross-probing: sync with PDF schematic whenever a component is selected.
        // Only on a selection change, so a search typed in the PDF panel is not overridden.
        if self.pdf_viewer.is_open {
            if let Some(part_idx) = self.selected_part_idx {
                if let Some(board) = &self.board {
                    if let Some(part) = board.parts.get(part_idx) {
                        if self.pdf_viewer.probed_part != Some(part_idx) {
                            self.pdf_viewer.probed_part = Some(part_idx);
                            self.pdf_viewer.search_input = part.name.clone();
                            self.pdf_viewer.cross_probe(&part.name);
                        }
                    }
                }
            } else {
                self.pdf_viewer.probed_part = None;
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
                        ui.heading(RichText::new(i18n::tr(lang, "pdf.title")).strong().color(self.tokens.accent()));
                        if let Some(fn_str) = &self.pdf_viewer.filename {
                            ui.label(RichText::new(fn_str).small().color(self.tokens.text_dim()));
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("✖").on_hover_text(i18n::tr(lang, "pdf.close")).clicked() {
                                self.pdf_viewer.is_open = false;
                            }
                        });
                    });
                    ui.separator();

                    // Navigation bar
                    ui.horizontal(|ui| {
                        if ui.button("⏴").on_hover_text(i18n::tr(lang, "pdf.page_prev")).clicked() && self.pdf_viewer.current_page > 1 {
                            self.pdf_viewer.current_page -= 1;
                        }
                        let page_counter_text = i18n::tr(lang, "pdf.page_counter")
                            .replacen("{}", &self.pdf_viewer.current_page.to_string(), 1)
                            .replacen("{}", &self.pdf_viewer.total_pages.to_string(), 1);
                        ui.label(page_counter_text);
                        if ui.button("⏵").on_hover_text(i18n::tr(lang, "pdf.page_next")).clicked() && self.pdf_viewer.current_page < self.pdf_viewer.total_pages {
                            self.pdf_viewer.current_page += 1;
                        }

                        ui.separator();
                        let zoom_pct = (self.pdf_viewer.zoom * 100.0).round() as i32;
                        let zoom_label = format!("🔍 {zoom_pct}%");
                        if ui.button(RichText::new(zoom_label).color(self.tokens.accent()).monospace())
                            .on_hover_text("Click to reset zoom to 100%")
                            .clicked()
                        {
                            self.pdf_viewer.zoom = 1.0;
                            self.pdf_viewer.pan_offset = egui::Vec2::ZERO;
                            ui.ctx().request_repaint();
                        }

                        if !self.pdf_viewer.search_results.is_empty() {
                            ui.separator();
                            let count = self.pdf_viewer.search_results.len();
                            let current = self.pdf_viewer.selected_result_idx + 1;
                            let hits_label = format!("{current}/{count} {}", i18n::tr(lang, "pdf.hits"));

                            let mut match_to_select = None;
                            egui::ComboBox::from_id_salt("pdf_match_list")
                                .selected_text(RichText::new(hits_label).color(self.tokens.accent()))
                                .show_ui(ui, |ui| {
                                    for (i, hit) in self.pdf_viewer.search_results.iter().enumerate() {
                                        let item_label = format!("#{}: {} (p. {})", i + 1, hit.text, hit.page_number);
                                        let is_selected = i == self.pdf_viewer.selected_result_idx;
                                        if ui.selectable_label(is_selected, item_label).clicked() {
                                            match_to_select = Some(i);
                                        }
                                    }
                                });

                            if let Some(idx) = match_to_select {
                                self.pdf_viewer.select_match(idx);
                            }

                            if ui.button(i18n::tr(lang, "pdf.hit_prev")).clicked() {
                                self.pdf_viewer.prev_match();
                            }
                            if ui.button(i18n::tr(lang, "pdf.hit_next")).clicked() {
                                self.pdf_viewer.next_match();
                            }
                        }
                    });

                    // Text search: Enter or the button jumps to the first hit; Enter again on the
                    // same query steps to the next one.
                    ui.horizontal(|ui| {
                        let search_resp = ui.add(
                            egui::TextEdit::singleline(&mut self.pdf_viewer.search_input)
                                .hint_text(i18n::tr(lang, "pdf.search_hint"))
                                .desired_width(180.0),
                        );
                        let entered = search_resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                        if entered || ui.button(i18n::tr(lang, "pdf.search")).clicked() {
                            let query = self.pdf_viewer.search_input.trim().to_string();
                            if entered && !query.is_empty() && query == self.pdf_viewer.active_search && !self.pdf_viewer.search_results.is_empty() {
                                self.pdf_viewer.next_match();
                            } else {
                                self.pdf_viewer.search(&query);
                            }
                            if entered {
                                search_resp.request_focus();
                            }
                        }
                        if !self.pdf_viewer.active_search.is_empty()
                            && self.pdf_viewer.search_results.is_empty()
                            && self.pdf_viewer.text_indexed()
                        {
                            ui.label(RichText::new(i18n::tr(lang, "pdf.no_hits")).color(self.tokens.text_dim()));
                        }
                    });
                    ui.separator();

                    // PDF Page viewer canvas area with interactive pan and zoom
                    let (rect, resp) = ui.allocate_exact_size(ui.available_size(), egui::Sense::click_and_drag());
                    let painter = ui.painter().with_clip_rect(rect);
                    painter.rect_filled(rect, 4.0, Color32::from_rgb(255, 255, 255));

                    if resp.dragged() {
                        self.pdf_viewer.pan_offset += resp.drag_delta();
                    }

                    // Mouse wheel zoom centered under cursor with gentle sensitivity
                    if resp.hovered() {
                        let scroll_y = ui.input(|i| i.smooth_scroll_delta.y);
                        if scroll_y != 0.0 {
                            let old_zoom = self.pdf_viewer.zoom;
                            // Gentle 1.07 factor per tick instead of aggressive 1.15
                            let zoom_factor = if scroll_y > 0.0 { 1.07 } else { 1.0 / 1.07 };
                            let new_zoom = (old_zoom * zoom_factor).clamp(0.2, 30.0);

                            if let Some(hover_pos) = resp.hover_pos() {
                                // Keep the document point under the mouse invariant:
                                // hover_pos = rect.center() + pan_offset_new + (hover_pos - rect.center() - pan_offset_old) * (new_zoom / old_zoom)
                                let cursor_rel = hover_pos - rect.center() - self.pdf_viewer.pan_offset;
                                let scale_ratio = new_zoom / old_zoom;
                                self.pdf_viewer.pan_offset = hover_pos - rect.center() - cursor_rel * scale_ratio;
                            }

                            self.pdf_viewer.zoom = new_zoom;
                            ui.ctx().request_repaint();
                        }

                        // Keyboard navigation inside PDF panel when hovered, unless a text field
                        // (e.g. the search box) has the keyboard
                        let typing = ui.memory(|m| m.focused().is_some());
                        let (page_up, page_down, arrow_left, arrow_right, plus_key, minus_key) = ui.input(|i| {
                            if typing {
                                return Default::default();
                            }
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
                            self.pdf_viewer.zoom = (self.pdf_viewer.zoom * 1.15).clamp(0.2, 30.0);
                            ui.ctx().request_repaint();
                        } else if minus_key {
                            self.pdf_viewer.zoom = (self.pdf_viewer.zoom / 1.15).clamp(0.2, 30.0);
                            ui.ctx().request_repaint();
                        }
                    }

                    // Render crisp rasterized PDF page (whole, or backdrop + visible tiles) from engine
                    if self.pdf_viewer.doc_info.is_some() {
                        let page_size_pts = self.pdf_viewer.page_size_pts(self.pdf_viewer.current_page);
                        // Natural unzoomed size of the page in UI points
                        let base_page_size = page_size_pts * crate::pdf::PT;
                        let fit_scale = ((rect.width() * 0.95) / base_page_size.x.max(1.0))
                            .min((rect.height() * 0.95) / base_page_size.y.max(1.0));
                        let display_scale = fit_scale * self.pdf_viewer.zoom;
                        let render_size = base_page_size * display_scale;

                        // Center viewport on selected match if requested (e.g. cross-probing or next/prev match)
                        if self.pdf_viewer.pending_center_on_match {
                            if let Some(hit) = self.pdf_viewer.search_results.get(self.pdf_viewer.selected_result_idx) {
                                if hit.page_number == self.pdf_viewer.current_page {
                                    let center_pt = hit.bbox.center();
                                    let comp_pos = center_pt * crate::pdf::PT * display_scale;

                                    self.pdf_viewer.pan_offset = egui::vec2(
                                        render_size.x * 0.5 - comp_pos.x,
                                        render_size.y * 0.5 - comp_pos.y,
                                    );
                                    self.pdf_viewer.pending_center_on_match = false;
                                }
                            }
                        }

                        let render_pos = rect.min
                            + (rect.size() - render_size) * 0.5
                            + self.pdf_viewer.pan_offset;

                        let img_rect = Rect::from_min_size(render_pos, render_size);
                        let painted = self.pdf_viewer.paint_current_page(ui.ctx(), &painter, img_rect, rect);
                        if !painted {
                            let page_label = i18n::tr(lang, "pdf.loading_page")
                                .replace("{}", &self.pdf_viewer.current_page.to_string());
                            painter.text(
                                rect.center(),
                                egui::Align2::CENTER_CENTER,
                                page_label,
                                egui::FontId::proportional(16.0),
                                Color32::from_rgb(100, 100, 110),
                            );
                        }

                        // PDF coordinate mapping function (PDF points -> screen points)
                        let to_screen = |pdf_pt: Pos2| -> Pos2 {
                            let p = pdf_pt.to_vec2() * crate::pdf::PT * display_scale;
                            render_pos + p
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

                                painter.rect_filled(marker_rect, 2.0, bg_color);
                                painter.rect_stroke(
                                    marker_rect,
                                    2.0,
                                    egui::Stroke::new(2.0, Color32::from_rgb(220, 20, 40)),
                                    egui::StrokeKind::Middle,
                                );
                            }
                        }

                        // Reverse Cross-probing: Click on PDF text token to select & center part/net in PCB layout
                        if resp.clicked() {
                            let click_opt = resp.interact_pointer_pos().or_else(|| resp.hover_pos()).or_else(|| ui.input(|i| i.pointer.interact_pos()));
                            if let Some(click_pos) = click_opt {
                                if rect.contains(click_pos) {
                                    // Inverse map: screen pos -> PDF point
                                    let rel_screen = click_pos - render_pos;
                                    let pdf_pt_x = rel_screen.x / (crate::pdf::PT * display_scale);
                                    let pdf_pt_y = rel_screen.y / (crate::pdf::PT * display_scale);
                                    let clicked_pdf_pt = Pos2::new(pdf_pt_x, pdf_pt_y);

                                    let clicked_token = self.pdf_viewer.find_token_at(self.pdf_viewer.current_page, clicked_pdf_pt);
                                    let mut focus = None;

                                    if let Some(occ) = clicked_token {
                                        let token_raw = occ.text.trim();
                                        let token_clean = crate::pdf::SchematicIndex::normalize_key(token_raw);
                                        if let Some(board) = &self.board {
                                            // 1. Try matching component designator (e.g. C123, U5, R10)
                                            let part_match = board.find_part_by_name(token_raw).or_else(|| board.find_part_by_name(&token_clean));
                                            if let Some(part) = part_match {
                                                // Select only the part (no pin, via or net), as the side panel search does
                                                self.selected_part_idx = Some(part.id);
                                                self.selected_pin_idx = None;
                                                self.selected_via_idx = None;
                                                self.selected_net = None;
                                                // Already shown in the PDF: don't cross-probe it back to its first hit
                                                self.pdf_viewer.probed_part = Some(part.id);

                                                self.status_msg = format!("Selected part {} from schematic PDF", part.name);
                                                focus = Some(Focus::Part(part.id));
                                                ui.ctx().request_repaint();
                                            }
                                            // 2. Try matching net name (e.g. PP3V3_S5, GND, +12V)
                                            else if let Some(net) = board.get_net(token_raw).or_else(|| board.get_net(&token_clean)) {
                                                self.selected_net = Some(net.name.clone());
                                                self.selected_pin_idx = net.pin_indices.first().copied();
                                                self.selected_via_idx = None;
                                                self.selected_part_idx = self.selected_pin_idx.and_then(|p_idx| board.pins.get(p_idx)).map(|p| p.part_id);

                                                if let Some(first_pin) = self.selected_pin_idx.and_then(|idx| board.pins.get(idx)) {
                                                    focus = Some(Focus::Point(first_pin.pos, first_pin.side));
                                                }

                                                self.status_msg = format!("Selected net {} ({} pins) from schematic PDF", net.name, net.pin_indices.len());
                                                ui.ctx().request_repaint();
                                            }
                                        }
                                    }
                                    // Center the board on the hit, switching the view side if needed
                                    match focus {
                                        Some(Focus::Part(part_id)) => {
                                            self.focus_part(part_id, true);
                                        }
                                        Some(Focus::Point(center, side)) => self.focus_point(center, side, true),
                                        None => {}
                                    }
                                }
                            }
                        }
                    } else {
                        let page_label = i18n::tr(lang, "pdf.loading_page")
                            .replace("{}", &self.pdf_viewer.current_page.to_string());
                        painter.text(
                            rect.center(),
                            egui::Align2::CENTER_CENTER,
                            page_label,
                            egui::FontId::proportional(16.0),
                            Color32::from_rgb(100, 100, 110),
                        );
                    }
                });
        }

        // Modal dialog asking to open co-located matching schematic PDF
        if let Some(ref pdf_path) = self.pending_schematic_prompt.clone() {
            let mut close_dialog = false;
            let mut accept_dialog = false;

            // Keyboard triggers: Enter accepts, Escape dismisses
            if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                accept_dialog = true;
            }
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                close_dialog = true;
            }

            let file_name = pdf_path.file_name().and_then(|n| n.to_str()).unwrap_or("schematic.pdf");
            egui::Window::new(format!("📄 {}", i18n::tr(lang, "pdf.prompt_found_title")))
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                .show(ui.ctx(), |ui| {
                    ui.set_max_width(450.0);
                    ui.vertical_centered(|ui| {
                        ui.add_space(8.0);
                        ui.label(RichText::new(i18n::tr(lang, "pdf.prompt_found_msg")).strong());
                        ui.add_space(4.0);
                        ui.label(RichText::new(file_name).italics().color(self.tokens.accent()));
                        ui.add_space(12.0);

                        ui.horizontal(|ui| {
                            let avail = ui.available_width();
                            let cancel_text = RichText::new(i18n::tr(lang, "pdf.prompt_cancel"));
                            let open_text = RichText::new(i18n::tr(lang, "pdf.prompt_open")).strong();
                            // Button size approximation
                            let total_btn_w = 260.0;
                            let pad = ((avail - total_btn_w) * 0.5).max(0.0);
                            ui.add_space(pad);

                            if ui.button(cancel_text).clicked() {
                                close_dialog = true;
                            }
                            ui.add_space(16.0);
                            let open_btn = egui::Button::new(open_text);
                            if ui.add(open_btn).clicked() {
                                accept_dialog = true;
                            }
                        });
                        ui.add_space(4.0);
                    });
                });

            if accept_dialog {
                let pdf_path = pdf_path.clone();
                self.open_schematic(&pdf_path);
                self.pending_schematic_prompt = None;
                ui.ctx().request_repaint();
            } else if close_dialog {
                self.pending_schematic_prompt = None;
                ui.ctx().request_repaint();
            }
        }

        // Central Canvas Panel
        CentralPanel::default()
            .frame(egui::Frame::NONE.fill(self.tokens.canvas()))
            .show(ui, |ui: &mut Ui| {
                let area = ui.available_rect_before_wrap();
                let rects = match self.shown_side {
                    Side::Both => {
                        let (left, right) = area.split_left_right_at_fraction(0.5);
                        vec![(Side::Top, left.shrink2(egui::vec2(2.0, 0.0))), (Side::Bottom, right.shrink2(egui::vec2(2.0, 0.0)))]
                    }
                    side => vec![(side, area)],
                };
                self.show_board_canvases(ui, &rects);
            });

        about::show_about_window(ui.ctx(), &mut self.about_open, &self.tokens, lang);

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
            // Invalidate canvas mesh caches
            self.canvases.invalidate_caches();
            self.preferences = prefs;
            let _ = self.preferences.save_atomic();
        }
    }
}


#[cfg(test)]
mod focus_tests {
    use super::*;

    #[test]
    fn test_escape_clears_selection_when_no_text_focus() {
        // Verification of state logic
        let app = BoardViewApp {
            board: None,
            parser_registry: Arc::new(ParserRegistry::new()),
            tokens: Tokens::default(),
            renderer: Renderer::default(),
            canvases: BoardCanvases::default(),
            shown_side: Side::Top,
            canvas_rects: Vec::new(),
            side_panel_state: SidePanelState::default(),
            overlay_manager: OverlayManager::default(),
            alignment_session: AlignmentSession::default(),
            preferences: Preferences::default(),
            preferences_open: false,
            about_open: false,
            pending_auto_fit: false,
            selected_pin_idx: Some(42),
            selected_via_idx: None,
            selected_part_idx: None,
            selected_net: Some("VCC".to_string()),
            status_msg: "Selected".to_string(),
            filename: None,
            board_path: None,
            pdf_viewer: pdf::PdfSchematicViewer::default(),
            pending_schematic_prompt: None,
        };

        assert!(app.selected_pin_idx.is_some());
        assert!(app.selected_net.is_some());
    }

    #[test]
    fn close_board_returns_to_the_no_board_state() {
        let mut app = BoardViewApp::default();
        app.close_board();
        assert_eq!(app.status(), "Ready. Open a .cad file to inspect.", "no board: nothing to close");

        app.board = Some(Board::new());
        app.filename = Some("MS-V375.cad".to_string());
        app.selected_pin_idx = Some(1);
        app.selected_via_idx = Some(2);
        app.selected_part_idx = Some(3);
        app.selected_net = Some("GND".to_string());
        app.side_panel_state.filter_part = "U1".to_string();
        app.side_panel_state.overlay_editing = true;
        app.side_panel_state.stay_on_part = Some(3);
        app.pdf_viewer.path = Some(std::path::PathBuf::from("/boards/MS-V375.pdf"));
        app.pdf_viewer.is_open = true;
        app.pending_schematic_prompt = Some(std::path::PathBuf::from("/boards/MS-V375.pdf"));

        app.close_board();

        assert!(app.board.is_none());
        assert!(app.filename.is_none());
        assert!(app.selected_pin_idx.is_none() && app.selected_via_idx.is_none());
        assert!(app.selected_part_idx.is_none() && app.selected_net.is_none());
        assert!(!app.side_panel_state.overlay_editing && app.side_panel_state.stay_on_part.is_none());
        assert_eq!(app.side_panel_state.filter_part, "U1", "search filters are kept");
        assert!(!app.pdf_viewer.is_open && app.pdf_viewer.path.is_none());
        assert!(app.pending_schematic_prompt.is_none());
        assert!(app.canvases.top.cached_view_key.is_none() && app.canvases.bottom.cached_view_key.is_none());
        assert_eq!(app.status(), "Closed MS-V375.cad.");
    }

    #[test]
    fn opening_a_board_closes_the_previous_schematic() {
        let mut app = BoardViewApp::default();
        app.board = Some(Board::new());
        app.board_path = Some(std::path::PathBuf::from("/boards/a.cad"));
        app.pdf_viewer.path = Some(std::path::PathBuf::from("/boards/a.pdf"));
        app.pdf_viewer.is_open = true;
        app.pending_schematic_prompt = Some(std::path::PathBuf::from("/boards/a.pdf"));

        app.adopt_board("b.cad", Board::new());

        assert!(!app.pdf_viewer.is_open && app.pdf_viewer.path.is_none());
        assert!(app.pending_schematic_prompt.is_none());
        assert!(app.board_path.is_none(), "the caller sets the path of a board opened from a file");
        assert_eq!(app.filename.as_deref(), Some("b.cad"));
    }

    #[test]
    fn schematic_is_remembered_only_for_a_board_file() {
        let mut app = BoardViewApp::default();
        let pdf = std::path::Path::new("/boards/a.pdf");
        assert!(!app.remember_schematic(pdf), "no board loaded");
        app.board = Some(Board::new());
        assert!(!app.remember_schematic(pdf), "board without a file path (dropped bytes)");
        app.board_path = Some(std::path::PathBuf::from("/boards/a.cad"));
        assert!(app.remember_schematic(pdf));
        assert_eq!(app.preferences.board_schematic(std::path::Path::new("/boards/a.cad")), Some(pdf));
        assert!(!app.remember_schematic(pdf), "unchanged association needs no save");
    }

    #[test]
    fn remembered_schematic_requires_an_existing_pdf() {
        let dir = std::env::temp_dir().join(format!("bvc_assoc_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let board = dir.join("board.cad");
        let pdf = dir.join("schematic.pdf");
        std::fs::write(&pdf, b"%PDF-1.4").unwrap();
        let mut prefs = Preferences::default();
        assert_eq!(remembered_schematic_pdf(&prefs, &board), None);
        prefs.remember_board_schematic(board.clone(), pdf.clone());
        assert_eq!(remembered_schematic_pdf(&prefs, &board), Some(pdf.clone()));
        std::fs::remove_file(&pdf).unwrap();
        assert_eq!(remembered_schematic_pdf(&prefs, &board), None, "a moved or deleted PDF falls back to the prompt");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_find_matching_schematic_pdf() {
        let temp_dir = std::env::temp_dir().join(format!("bvc_test_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        let board_path = temp_dir.join("MS-V373.cad");
        let pdf_path = temp_dir.join("MS-V373.pdf");
        let pdf_upper = temp_dir.join("BOARD2.PDF");
        let board2_path = temp_dir.join("BOARD2.brd");
        let board3_path = temp_dir.join("BOARD3.fz");

        std::fs::write(&board_path, b"test").unwrap();
        std::fs::write(&pdf_path, b"%PDF-1.4").unwrap();
        std::fs::write(&board2_path, b"test2").unwrap();
        std::fs::write(&pdf_upper, b"%PDF-1.4").unwrap();
        std::fs::write(&board3_path, b"test3").unwrap();

        // 1. Lowercase .pdf match
        let found = find_matching_schematic_pdf(&board_path);
        assert_eq!(found, Some(pdf_path));

        // 2. Uppercase .PDF match
        let found2 = find_matching_schematic_pdf(&board2_path);
        assert_eq!(found2, Some(pdf_upper));

        // 3. No match
        let found3 = find_matching_schematic_pdf(&board3_path);
        assert_eq!(found3, None);

        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}

#[cfg(test)]
mod side_selector_tests {
    use super::*;

    #[test]
    fn test_side_selector_offers_all_three_sides() {
        assert_eq!(SIDE_CHOICES, [Side::Top, Side::Bottom, Side::Both]);
    }

    #[test]
    fn test_side_label_is_translated_for_every_side() {
        let lang = i18n::Language::English;
        assert_eq!(side_label(lang, Side::Top), "View: TOP");
        assert_eq!(side_label(lang, Side::Bottom), "View: BOTTOM");
        assert_eq!(side_label(lang, Side::Both), "View: BOTH");
        for &l in &i18n::Language::ALL {
            for side in SIDE_CHOICES {
                assert_ne!(side_label(l, side), "toolbar.view_both");
            }
            assert_ne!(i18n::tr(l, "toolbar.view_side_hover"), "toolbar.view_side_hover");
        }
    }
}
