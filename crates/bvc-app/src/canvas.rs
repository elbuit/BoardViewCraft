//! Board canvas view transformation and interactive rendering.

use crate::preferences::Preferences;
use crate::render::Renderer;
use crate::theme::Tokens;
use bvc_core::{Board, Point, Rect, Side};
use egui::epaint::Mesh;
use egui::{Color32, FontId, Pos2, Rect as EguiRect, Response, Sense, Stroke, Ui};
use rayon::prelude::*;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Rotation {
    #[default]
    Deg0,
    Deg90,
    Deg180,
    Deg270,
}

impl Rotation {
    pub fn rotate_cw(self) -> Self {
        match self {
            Rotation::Deg0 => Rotation::Deg90,
            Rotation::Deg90 => Rotation::Deg180,
            Rotation::Deg180 => Rotation::Deg270,
            Rotation::Deg270 => Rotation::Deg0,
        }
    }

    pub fn rotate_ccw(self) -> Self {
        match self {
            Rotation::Deg0 => Rotation::Deg270,
            Rotation::Deg270 => Rotation::Deg180,
            Rotation::Deg180 => Rotation::Deg90,
            Rotation::Deg90 => Rotation::Deg0,
        }
    }

    pub fn rotate_180(self) -> Self {
        match self {
            Rotation::Deg0 => Rotation::Deg180,
            Rotation::Deg90 => Rotation::Deg270,
            Rotation::Deg180 => Rotation::Deg0,
            Rotation::Deg270 => Rotation::Deg90,
        }
    }

    pub fn angle_rad(self) -> f32 {
        match self {
            Rotation::Deg0 => 0.0,
            Rotation::Deg90 => std::f32::consts::FRAC_PI_2,
            Rotation::Deg180 => std::f32::consts::PI,
            Rotation::Deg270 => 3.0 * std::f32::consts::FRAC_PI_2,
        }
    }

    pub fn display_str(self) -> &'static str {
        match self {
            Rotation::Deg0 => "0°",
            Rotation::Deg90 => "90°",
            Rotation::Deg180 => "180°",
            Rotation::Deg270 => "270°",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Viewport {
    /// Center of the view in board coordinates (mils)
    pub center: Point,
    /// Pixels per mil
    pub scale: f32,
    /// Active side view (Top or Bottom)
    pub side: Side,
    /// Rotation in screen plane
    pub rotation: Rotation,
    /// Mirror horizontally (X axis)
    pub mirror_x: bool,
    /// Mirror vertically (Y axis)
    pub mirror_y: bool,
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            center: Point::ZERO,
            scale: 0.05,
            side: Side::Top,
            rotation: Rotation::Deg0,
            mirror_x: false,
            mirror_y: false,
        }
    }
}

impl Viewport {
    /// Effective scale multipliers including side and user mirror flags
    #[inline]
    fn effective_factors(&self) -> (f32, f32) {
        let is_bottom_flipped = self.side == Side::Bottom;
        let sx = if is_bottom_flipped ^ self.mirror_x {
            -1.0
        } else {
            1.0
        };
        let sy = if self.mirror_y { -1.0 } else { 1.0 };
        (sx, sy)
    }

    /// Transforms world point to screen point.
    pub fn world_to_screen(&self, pt: Point, canvas_rect: EguiRect) -> Pos2 {
        let center_screen = canvas_rect.center();
        let (sx, sy) = self.effective_factors();

        // 1. Offset relative to board center and apply reflections
        let dx = (pt.x - self.center.x) as f32 * sx;
        let dy = (pt.y - self.center.y) as f32 * sy;

        // 2. CAD-to-Screen Y inversion (CAD +Y is up, screen +Y is down)
        let dy_cad = -dy;

        // 3. 2D rotation in screen plane
        let theta = self.rotation.angle_rad();
        let (sin_t, cos_t) = theta.sin_cos();
        let x_rot = dx * cos_t - dy_cad * sin_t;
        let y_rot = dx * sin_t + dy_cad * cos_t;

        // 4. Uniform scaling and translation to canvas center
        let screen_x = center_screen.x + x_rot * self.scale;
        let screen_y = center_screen.y + y_rot * self.scale;

        Pos2::new(screen_x, screen_y)
    }

    /// Transforms screen point to world point. Exact mathematical inverse of `world_to_screen`.
    pub fn screen_to_world(&self, screen_pt: Pos2, canvas_rect: EguiRect) -> Point {
        let center_screen = canvas_rect.center();
        let (sx, sy) = self.effective_factors();

        // 1. Un-center and un-scale
        let x_rot = (screen_pt.x - center_screen.x) / self.scale;
        let y_rot = (screen_pt.y - center_screen.y) / self.scale;

        // 2. Reverse rotation by -theta
        let theta = self.rotation.angle_rad();
        let (sin_t, cos_t) = theta.sin_cos();
        let dx = x_rot * cos_t + y_rot * sin_t;
        let dy_cad = -x_rot * sin_t + y_rot * cos_t;

        // 3. Reverse CAD Y inversion
        let dy = -dy_cad;

        // 4. Reverse reflections and restore world center
        let world_x = self.center.x + (dx / sx) as f64;
        let world_y = self.center.y + (dy / sy) as f64;

        Point::new(world_x, world_y)
    }

    /// Returns the visible world rectangle for the given screen canvas rect.
    pub fn visible_world_rect(&self, canvas_rect: EguiRect) -> Rect {
        let p1 = self.screen_to_world(canvas_rect.left_top(), canvas_rect);
        let p2 = self.screen_to_world(canvas_rect.right_top(), canvas_rect);
        let p3 = self.screen_to_world(canvas_rect.left_bottom(), canvas_rect);
        let p4 = self.screen_to_world(canvas_rect.right_bottom(), canvas_rect);

        let min_x = p1.x.min(p2.x).min(p3.x).min(p4.x);
        let max_x = p1.x.max(p2.x).max(p3.x).max(p4.x);
        let min_y = p1.y.min(p2.y).min(p3.y).min(p4.y);
        let max_y = p1.y.max(p2.y).max(p3.y).max(p4.y);

        Rect {
            min_x,
            min_y,
            max_x,
            max_y,
        }
    }

    /// Fits the board bounding box to the given canvas rectangle (~90% fill).
    pub fn fit_to_rect(&mut self, bbox: Rect, canvas_rect: EguiRect) {
        let board_w = bbox.width().max(10.0) as f32;
        let board_h = bbox.height().max(10.0) as f32;

        let margin = 40.0;
        let avail_w = ((canvas_rect.width() - margin) * 0.9).max(10.0);
        let avail_h = ((canvas_rect.height() - margin) * 0.9).max(10.0);

        let (dim_w, dim_h) = match self.rotation {
            Rotation::Deg0 | Rotation::Deg180 => (board_w, board_h),
            Rotation::Deg90 | Rotation::Deg270 => (board_h, board_w),
        };

        let scale_x = avail_w / dim_w;
        let scale_y = avail_h / dim_h;
        self.scale = scale_x.min(scale_y).clamp(0.001, 100.0);
        self.center = bbox.center();
    }

    /// Sets zoom to a preset factor (where 1.0 = 100% = 0.05 scale).
    pub fn zoom_preset(&mut self, factor: f32, _canvas_rect: EguiRect) {
        let new_scale = (0.05 * factor).clamp(0.001, 100.0);
        self.scale = new_scale;
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct ViewCacheKey {
    pub center: (i64, i64), // scaled integer for float comparison or approx
    pub scale_bits: u32,
    pub side: Side,
    pub rotation: Rotation,
    pub mirror_x: bool,
    pub mirror_y: bool,
    pub rect_size: (u32, u32),
    pub selected_pin: Option<usize>,
    pub selected_net: Option<String>,
    pub selected_part: Option<usize>,
    pub hovered_pin: Option<usize>,
    pub hovered_via: Option<usize>,
    pub board_ptr: usize,
    pub show_net_web: bool,
    pub show_opposite_elements: bool,
}

pub struct CanvasState {
    pub viewport: Viewport,
    pub hovered_pin: Option<usize>,
    pub hovered_via: Option<usize>,
    pub cached_meshes: Vec<egui::Mesh>,
    pub cached_view_key: Option<ViewCacheKey>,
}

impl Default for CanvasState {
    fn default() -> Self {
        Self {
            viewport: Viewport::default(),
            hovered_pin: None,
            hovered_via: None,
            cached_meshes: Vec::new(),
            cached_view_key: None,
        }
    }
}

pub fn show_canvas(
    ui: &mut Ui,
    tokens: &Tokens,
    board: Option<&Board>,
    canvas_state: &mut CanvasState,
    selected_pin_idx: &mut Option<usize>,
    selected_part_idx: &mut Option<usize>,
    selected_net: &mut Option<String>,
    renderer: &mut Renderer,
    prefs: &Preferences,
) -> Response {
    let (response, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
    let rect = response.rect;

    // Background fill
    painter.rect_filled(rect, 0.0, tokens.canvas);

    // Grid rendering (cadcraft style)
    let vp = &mut canvas_state.viewport;
    let grid_spacing_mils = 100.0; // 100 mils = 0.1 inch
    let screen_grid_spacing = (grid_spacing_mils * vp.scale as f64) as f32;

    if screen_grid_spacing > 8.0 && screen_grid_spacing < 400.0 {
        // Draw subtle dot/line grid
        let start_world = vp.screen_to_world(rect.min, rect);
        let end_world = vp.screen_to_world(rect.max, rect);

        let min_x = start_world.x.min(end_world.x);
        let max_x = start_world.x.max(end_world.x);
        let min_y = start_world.y.min(end_world.y);
        let max_y = start_world.y.max(end_world.y);

        let first_x = (min_x / grid_spacing_mils).floor() * grid_spacing_mils;
        let first_y = (min_y / grid_spacing_mils).floor() * grid_spacing_mils;

        let mut x = first_x;
        while x <= max_x {
            let p1 = vp.world_to_screen(Point::new(x, min_y), rect);
            let p2 = vp.world_to_screen(Point::new(x, max_y), rect);
            let color = if (x % 500.0).abs() < 1e-3 {
                tokens.grid_major
            } else {
                tokens.grid_minor
            };
            painter.line_segment([p1, p2], Stroke::new(1.0, color));
            x += grid_spacing_mils;
        }

        let mut y = first_y;
        while y <= max_y {
            let p1 = vp.world_to_screen(Point::new(min_x, y), rect);
            let p2 = vp.world_to_screen(Point::new(max_x, y), rect);
            let color = if (y % 500.0).abs() < 1e-3 {
                tokens.grid_major
            } else {
                tokens.grid_minor
            };
            painter.line_segment([p1, p2], Stroke::new(1.0, color));
            y += grid_spacing_mils;
        }
    }

    // Interactive Pan (Middle click or Right click drag, or left-drag if not clicking an element)
    if response.dragged_by(egui::PointerButton::Middle)
        || response.dragged_by(egui::PointerButton::Secondary)
        || response.dragged_by(egui::PointerButton::Primary)
    {
        let delta = response.drag_delta();
        // Move viewport center so the point under the mouse follows the cursor exactly:
        // Screen point center_screen shifted by -delta corresponds to the new world center.
        let center_screen = rect.center();
        vp.center = vp.screen_to_world(center_screen - delta, rect);
    }

    // Zoom on mouse wheel
    let scroll_y = ui.input(|i| i.smooth_scroll_delta.y);
    if scroll_y != 0.0 {
        if let Some(hover_pos) = response.hover_pos() {
            let mouse_world_before = vp.screen_to_world(hover_pos, rect);
            let zoom_factor = if scroll_y > 0.0 { 1.15 } else { 1.0 / 1.15 };
            vp.scale = (vp.scale * zoom_factor).clamp(0.001, 100.0);
            let mouse_world_after = vp.screen_to_world(hover_pos, rect);

            vp.center.x += mouse_world_before.x - mouse_world_after.x;
            vp.center.y += mouse_world_before.y - mouse_world_after.y;
        }
    }

    // Hover detection
    let mut current_hover_pin: Option<usize> = None;
    let mut current_hover_via: Option<usize> = None;

    if let Some(hover_pos) = response.hover_pos() {
        if let Some(b) = board {
            let world_pos = vp.screen_to_world(hover_pos, rect);
            let hit_tolerance = (15.0 / vp.scale) as f64;
            current_hover_pin = b.find_pin_near(world_pos, vp.side, hit_tolerance);
            if current_hover_pin.is_none() {
                current_hover_via = b.find_via_near(world_pos, vp.side, hit_tolerance);
            }
        }
    }
    canvas_state.hovered_pin = current_hover_pin;
    canvas_state.hovered_via = current_hover_via;

    // Selection on click
    if response.clicked() {
        if let Some(pin_idx) = current_hover_pin {
            if *selected_pin_idx == Some(pin_idx) {
                // Clicking the already-selected pin deselects it
                *selected_pin_idx = None;
                *selected_part_idx = None;
                *selected_net = None;
            } else if let Some(b) = board {
                if let Some(pin) = b.pins.get(pin_idx) {
                    *selected_pin_idx = Some(pin_idx);
                    *selected_part_idx = Some(pin.part_id);
                    *selected_net = if pin.net_name.is_empty() {
                        None
                    } else {
                        Some(pin.net_name.clone())
                    };
                }
            }
        } else if let Some(via_idx) = current_hover_via {
            if let Some(b) = board {
                if let Some(via) = b.vias.get(via_idx) {
                    if *selected_pin_idx == None && selected_net.as_deref() == Some(&via.net_name) {
                        // Clicking the same via deselects
                        *selected_net = None;
                    } else {
                        *selected_pin_idx = None;
                        *selected_net = if via.net_name.is_empty() {
                            None
                        } else {
                            Some(via.net_name.clone())
                        };
                    }
                }
            }
        }
    }

    // Render Board elements
    if let Some(b) = board {
        let board_ptr = b as *const _ as usize;
        let current_key = ViewCacheKey {
            center: ((vp.center.x * 1000.0) as i64, (vp.center.y * 1000.0) as i64),
            scale_bits: vp.scale.to_bits(),
            side: vp.side,
            rotation: vp.rotation,
            mirror_x: vp.mirror_x,
            mirror_y: vp.mirror_y,
            rect_size: (rect.width().round() as u32, rect.height().round() as u32),
            selected_pin: *selected_pin_idx,
            selected_net: selected_net.clone(),
            selected_part: *selected_part_idx,
            hovered_pin: current_hover_pin,
            hovered_via: current_hover_via,
            board_ptr,
            show_net_web: prefs.show_net_web,
            show_opposite_elements: prefs.show_opposite_elements,
        };

        if canvas_state.cached_view_key.as_ref() == Some(&current_key) {
            // Cache hit: immediately emit cached meshes, bypassing all geometry generation and filtering!
            for mesh in &canvas_state.cached_meshes {
                painter.add(mesh.clone());
            }
        } else {
            // Cache miss: generate meshes and update cache
            let new_meshes = generate_board_meshes(
                b,
                vp,
                rect,
                tokens,
                *selected_pin_idx,
                *selected_part_idx,
                selected_net.as_deref(),
                current_hover_pin,
                current_hover_via,
                renderer,
                prefs.show_net_web,
                prefs.show_opposite_elements,
            );

            for mesh in &new_meshes {
                painter.add(mesh.clone());
            }

            canvas_state.cached_meshes = new_meshes;
            canvas_state.cached_view_key = Some(current_key);
        }

        let margin_mils = (50.0 / vp.scale).max(40.0) as f64;
        let visible_world = vp.visible_world_rect(rect).expand(margin_mils);
        let selected_p_idx = *selected_part_idx;
        let selected_pn_idx = *selected_pin_idx;

        // 1. Component designator labels (with screen-space LOD decimation)
        for part in &b.parts {
            if !part.side.visible_on(vp.side) {
                continue;
            }
            if let Some(ref c_rect) = part.bounding_box {
                if c_rect.intersects(&visible_world) {
                    let is_selected = selected_p_idx == Some(part.id);
                    if !is_selected && !prefs.show_part_labels {
                        continue;
                    }

                    // Screen-space footprint dimension threshold:
                    // If not selected and projected footprint is too small, skip rendering.
                    let w_screen = c_rect.width() as f32 * vp.scale;
                    let h_screen = c_rect.height() as f32 * vp.scale;
                    if !is_selected && (w_screen < 24.0 || h_screen < 12.0) {
                        continue;
                    }

                    let center = vp.world_to_screen(c_rect.center(), rect);
                    painter.text(
                        center,
                        egui::Align2::CENTER_CENTER,
                        &part.name,
                        FontId::proportional(12.0),
                        if is_selected {
                            tokens.selection
                        } else {
                            tokens.part_text
                        },
                    );
                }
            }
        }

        // 2. Pin annotations & numbers (with screen-space LOD decimation)
        if prefs.show_pin_annotations {
            let active_membership = selected_net
                .as_deref()
                .map(|sn| b.net_membership(sn))
                .unwrap_or_default();

            let visible_pin_indices = if !b.spatial_grid.is_empty() {
                b.spatial_grid.query_pins_in_rect(&visible_world)
            } else {
                (0..b.pins.len()).collect()
            };

            for idx in visible_pin_indices {
                let pin = &b.pins[idx];
                let is_on_cur_side = pin.side.visible_on(vp.side);
                let is_selected = selected_pn_idx == Some(idx);
                let is_net_selected = active_membership.contains_pin(idx);
                let is_hovered = current_hover_pin == Some(idx);

                let allow_opposite = prefs.show_opposite_elements || is_net_selected;
                if !allow_opposite && !is_on_cur_side {
                    continue;
                }

                // Check visible frustum
                if pin.pos.x < visible_world.min_x
                    || pin.pos.x > visible_world.max_x
                    || pin.pos.y < visible_world.min_y
                    || pin.pos.y > visible_world.max_y
                {
                    continue;
                }

                let r_screen = pin.radius as f32 * vp.scale;
                if !is_selected && !is_net_selected && !is_hovered && r_screen < 8.0 {
                    continue;
                }

                if !pin.name.is_empty() {
                    let screen_pos = vp.world_to_screen(pin.pos, rect);
                    let font_size = (r_screen * 1.1).clamp(8.0, 14.0);
                    let text_color = if is_selected {
                        tokens.selection
                    } else if is_hovered {
                        tokens.hover
                    } else if is_net_selected {
                        if is_on_cur_side {
                            tokens.pin_highlight_cur_side
                        } else {
                            tokens.pin_highlight_opp_side
                        }
                    } else {
                        tokens.text
                    };

                    painter.text(
                        screen_pos,
                        egui::Align2::CENTER_CENTER,
                        &pin.name,
                        FontId::monospace(font_size),
                        text_color,
                    );
                }
            }
        }

        // Ring around selected pin
        if let Some(idx) = *selected_pin_idx {
            if let Some(pin) = b.pins.get(idx) {
                let screen_pos = vp.world_to_screen(pin.pos, rect);
                let screen_r = (pin.radius as f32 * vp.scale).clamp(2.0, 40.0);
                painter.circle_stroke(screen_pos, screen_r + 3.0, Stroke::new(2.0, tokens.selection));
            }
        }
    }

    response
}

/// Generates render meshes for all visible board entities using spatial frustum queries,
/// precomputed net membership, and subpixel LOD tessellation.
pub fn generate_board_meshes(
    b: &Board,
    vp: &Viewport,
    rect: EguiRect,
    tokens: &Tokens,
    selected_pin_idx: Option<usize>,
    selected_part_idx: Option<usize>,
    selected_net: Option<&str>,
    current_hover_pin: Option<usize>,
    current_hover_via: Option<usize>,
    renderer: &mut Renderer,
    show_net_web: bool,
    show_opposite_elements: bool,
) -> Vec<Mesh> {
    let mut new_meshes = Vec::new();

    // Compute visible world frustum with a generous margin in world coordinates
    let margin_mils = (50.0 / vp.scale).max(40.0) as f64;
    let visible_world = vp.visible_world_rect(rect).expand(margin_mils);

    // 1. Board outline polygon
    if b.outline_polygon.len() >= 3 {
        let screen_pts: Vec<Pos2> = b
            .outline_polygon
            .iter()
            .map(|p| vp.world_to_screen(*p, rect))
            .collect();

        let fill_mesh = renderer.fill_polygon(&screen_pts, tokens.board_fill);
        if !fill_mesh.is_empty() {
            new_meshes.push(fill_mesh);
        }

        let stroke_mesh = renderer.stroke_polygon(&screen_pts, 2.0, tokens.board_outline, true);
        if !stroke_mesh.is_empty() {
            new_meshes.push(stroke_mesh);
        }
    }

    // 2. Component bounding outlines
    let outlines_mesh: Mesh = b
        .parts
        .par_iter()
        .filter(|part| {
            if !part.side.visible_on(vp.side) {
                return false;
            }
            match part.bounding_box {
                Some(ref bbox) => bbox.intersects(&visible_world),
                None => false,
            }
        })
        .fold(
            Mesh::default,
            |mut mesh, part| {
                if let Some(ref c_rect) = part.bounding_box {
                    let is_part_selected = selected_part_idx == Some(part.id);
                    let stroke_color = if is_part_selected {
                        tokens.selection
                    } else {
                        tokens.part_body
                    };

                    let p1 = vp.world_to_screen(Point::new(c_rect.min_x, c_rect.min_y), rect);
                    let p2 = vp.world_to_screen(Point::new(c_rect.max_x, c_rect.min_y), rect);
                    let p3 = vp.world_to_screen(Point::new(c_rect.max_x, c_rect.max_y), rect);
                    let p4 = vp.world_to_screen(Point::new(c_rect.min_x, c_rect.max_y), rect);

                    Renderer::append_rect_outline_to_mesh(
                        &mut mesh,
                        p1,
                        p2,
                        p3,
                        p4,
                        0.75, // half_width for 1.5px stroke
                        stroke_color,
                    );
                }
                mesh
            },
        )
        .reduce(
            Mesh::default,
            |mut mesh_a, mesh_b| {
                mesh_a.append(mesh_b);
                mesh_a
            },
        );

    if !outlines_mesh.is_empty() {
        new_meshes.push(outlines_mesh);
    }

    // Precompute O(1) net membership lookup
    let active_membership = selected_net
        .map(|sn| b.net_membership(sn))
        .unwrap_or_default();
    let is_high_density = active_membership.is_high_density();

    // 3. Vias (Spatial frustum query + Batched parallel tessellation + LOD)
    let visible_via_indices = if !b.spatial_grid.is_empty() {
        b.spatial_grid.query_vias_in_rect(&visible_world)
    } else {
        (0..b.vias.len()).collect()
    };

    let (via_normal_mesh, via_highlight_mesh): (Mesh, Mesh) = visible_via_indices
        .par_iter()
        .map(|&idx| (idx, &b.vias[idx]))
        .filter(|(idx, via)| {
            let is_on_cur_side = via.side.visible_on(vp.side);
            let is_net_selected = active_membership.contains_via(*idx);
            let allow_opposite = show_opposite_elements || is_net_selected;
            if !allow_opposite && !is_on_cur_side {
                return false;
            }
            via.pos.x >= visible_world.min_x
                && via.pos.x <= visible_world.max_x
                && via.pos.y >= visible_world.min_y
                && via.pos.y <= visible_world.max_y
        })
        .fold(
            || (Mesh::default(), Mesh::default()),
            |(mut normal_mesh, mut hl_mesh), (idx, via)| {
                let is_on_cur_side = via.side.visible_on(vp.side);
                let is_net_selected = active_membership.contains_via(idx);
                let is_hovered = current_hover_via == Some(idx);

                let screen_pos = vp.world_to_screen(via.pos, rect);
                let raw_screen_r = via.radius as f32 * vp.scale;

                if is_hovered {
                    let screen_r = raw_screen_r.clamp(2.0, 30.0);
                    Renderer::append_circle_to_mesh(&mut hl_mesh, screen_pos, screen_r, tokens.hover);
                } else if is_net_selected {
                    if is_on_cur_side {
                        Renderer::append_lod_circle_to_mesh(
                            &mut hl_mesh,
                            screen_pos,
                            raw_screen_r,
                            is_high_density,
                            tokens.via_highlight,
                        );
                    } else {
                        // Opposite side via of selected net: render as distinct hollow outline
                        let stroke_w = if is_high_density { 1.2 } else { 1.6 };
                        Renderer::append_circle_outline_to_mesh(
                            &mut hl_mesh,
                            screen_pos,
                            raw_screen_r.clamp(1.5, 30.0),
                            stroke_w,
                            tokens.pin_highlight_opp_side,
                        );
                    }
                } else {
                    let color = if is_on_cur_side {
                        tokens.via_cur_side
                    } else {
                        tokens.via_opp_side
                    };

                    Renderer::append_lod_circle_to_mesh(
                        &mut normal_mesh,
                        screen_pos,
                        raw_screen_r,
                        false,
                        color,
                    );
                }

                (normal_mesh, hl_mesh)
            },
        )
        .reduce(
            || (Mesh::default(), Mesh::default()),
            |(mut norm_a, mut hl_a), (norm_b, hl_b)| {
                norm_a.append(norm_b);
                hl_a.append(hl_b);
                (norm_a, hl_a)
            },
        );

    if !via_normal_mesh.is_empty() {
        new_meshes.push(via_normal_mesh);
    }
    if !via_highlight_mesh.is_empty() {
        new_meshes.push(via_highlight_mesh);
    }

    // 4. Pins (Spatial frustum query + Batched parallel tessellation + LOD)
    let visible_pin_indices = if !b.spatial_grid.is_empty() {
        b.spatial_grid.query_pins_in_rect(&visible_world)
    } else {
        (0..b.pins.len()).collect()
    };

    let (pin_normal_mesh, pin_highlight_mesh): (Mesh, Mesh) = visible_pin_indices
        .par_iter()
        .map(|&idx| (idx, &b.pins[idx]))
        .filter(|(_idx, pin)| {
            let is_on_cur_side = pin.side.visible_on(vp.side);
            let is_net_selected = active_membership.contains_pin(*_idx);

            // Allow opposite-side pins if either:
            // 1. show_opposite_elements preference is enabled, OR
            // 2. a net is actively selected (or a pin of it is selected) and this pin belongs to it.
            let allow_opposite = show_opposite_elements || is_net_selected;
            if !allow_opposite && !is_on_cur_side {
                return false;
            }
            pin.pos.x >= visible_world.min_x
                && pin.pos.x <= visible_world.max_x
                && pin.pos.y >= visible_world.min_y
                && pin.pos.y <= visible_world.max_y
        })
        .fold(
            || (Mesh::default(), Mesh::default()),
            |(mut normal_mesh, mut hl_mesh), (idx, pin)| {
                let is_on_cur_side = pin.side.visible_on(vp.side);
                let is_selected_pin = selected_pin_idx == Some(idx);
                let is_net_selected = active_membership.contains_pin(idx);
                let is_hovered = current_hover_pin == Some(idx);

                let screen_pos = vp.world_to_screen(pin.pos, rect);
                let raw_screen_r = pin.radius as f32 * vp.scale;

                if is_selected_pin {
                    let screen_r = raw_screen_r.clamp(2.0, 40.0);
                    Renderer::append_circle_to_mesh(
                        &mut hl_mesh,
                        screen_pos,
                        screen_r,
                        tokens.selection,
                    );
                } else if is_hovered {
                    let screen_r = raw_screen_r.clamp(2.0, 40.0);
                    Renderer::append_circle_to_mesh(&mut hl_mesh, screen_pos, screen_r, tokens.hover);
                } else if is_net_selected {
                    if is_on_cur_side {
                        // Current side: filled highlight circle
                        Renderer::append_lod_circle_to_mesh(
                            &mut hl_mesh,
                            screen_pos,
                            raw_screen_r,
                            is_high_density,
                            tokens.pin_highlight_cur_side,
                        );
                    } else {
                        // Opposite side: hollow outline / border ONLY!
                        let stroke_w = if is_high_density { 1.2 } else { 1.8 };
                        Renderer::append_circle_outline_to_mesh(
                            &mut hl_mesh,
                            screen_pos,
                            raw_screen_r.clamp(1.5, 40.0),
                            stroke_w,
                            tokens.pin_highlight_opp_side,
                        );
                    }
                } else {
                    let color = if !is_on_cur_side {
                        Color32::from_rgba_premultiplied(0x35, 0x40, 0x50, 0x40)
                    } else if pin.net_name.is_empty() || pin.net_name == "UNCONNECTED" {
                        tokens.pin_unconnected
                    } else {
                        tokens.pin_default
                    };

                    Renderer::append_lod_circle_to_mesh(
                        &mut normal_mesh,
                        screen_pos,
                        raw_screen_r,
                        false,
                        color,
                    );
                }

                (normal_mesh, hl_mesh)
            },
        )
        .reduce(
            || (Mesh::default(), Mesh::default()),
            |(mut norm_a, mut hl_a), (norm_b, hl_b)| {
                norm_a.append(norm_b);
                hl_a.append(hl_b);
                (norm_a, hl_a)
            },
        );

    if !pin_normal_mesh.is_empty() {
        new_meshes.push(pin_normal_mesh);
    }
    if !pin_highlight_mesh.is_empty() {
        new_meshes.push(pin_highlight_mesh);
    }

    // 5. Net lines web (Batched line mesh inside cached render pipeline)
    // Uses Minimum Spanning Tree (MST) so that every connected pin and via on the net
    // is daisy-chained along shortest Euclidean paths, matching real EDA flight lines.
    if show_net_web {
        if let Some(net_name) = selected_net {
            let is_ground_or_power = net_name.eq_ignore_ascii_case("GND")
                || net_name.eq_ignore_ascii_case("GROUND")
                || net_name.starts_with("UNCONNECTED")
                || net_name.is_empty();

            if !is_ground_or_power {
                if let Some(net) = b.nets.get(net_name) {
                    let total_nodes = net.pin_indices.len() + net.via_indices.len();
                    if total_nodes >= 2 && total_nodes <= 300 {
                        // Collect world coordinates of all net pins and vias
                        let mut nodes: Vec<Point> = Vec::with_capacity(total_nodes);
                        for &pidx in &net.pin_indices {
                            if let Some(pin) = b.pins.get(pidx) {
                                nodes.push(pin.pos);
                            }
                        }
                        for &vidx in &net.via_indices {
                            if let Some(via) = b.vias.get(vidx) {
                                nodes.push(via.pos);
                            }
                        }

                        // Compute Euclidean Minimum Spanning Tree using Prim's algorithm
                        let n = nodes.len();
                        let mut in_mst = vec![false; n];
                        let mut min_dist = vec![f64::INFINITY; n];
                        let mut parent = vec![0usize; n];

                        in_mst[0] = true;
                        for v in 1..n {
                            min_dist[v] = nodes[0].dist(nodes[v]);
                            parent[v] = 0;
                        }

                        let mut segments = Vec::with_capacity(n - 1);
                        for _ in 1..n {
                            let mut best_u = usize::MAX;
                            let mut best_d = f64::INFINITY;
                            for u in 1..n {
                                if !in_mst[u] && min_dist[u] < best_d {
                                    best_d = min_dist[u];
                                    best_u = u;
                                }
                            }

                            if best_u == usize::MAX {
                                break;
                            }

                            in_mst[best_u] = true;
                            let p_u = vp.world_to_screen(nodes[best_u], rect);
                            let p_parent = vp.world_to_screen(nodes[parent[best_u]], rect);
                            segments.push((p_u, p_parent));

                            // Update neighbor distances from best_u
                            for v in 1..n {
                                if !in_mst[v] {
                                    let d = nodes[best_u].dist(nodes[v]);
                                    if d < min_dist[v] {
                                        min_dist[v] = d;
                                        parent[v] = best_u;
                                    }
                                }
                            }
                        }

                        if !segments.is_empty() {
                            let mut web_mesh = Mesh::default();
                            Renderer::append_lines_to_mesh(
                                &mut web_mesh,
                                &segments,
                                0.75, // half_width = 0.75 (1.5 px line)
                                tokens.net_web,
                            );
                            new_meshes.push(web_mesh);
                        }
                    }
                }
            }
        }
    }

    new_meshes
}

#[cfg(test)]
mod tests {
    use super::*;
    use bvc_core::{Pin, Point, Side, Via};
    use std::time::Instant;

    #[test]
    fn test_c35_pin1_net_rendering() {
        let file_path = "/media/no_raid/devel/boarview/MS-V375 REV 7.0.cad";
        if !std::path::Path::new(file_path).exists() {
            return;
        }
        let data = std::fs::read(file_path).expect("read cad");
        let registry = bvc_core::ParserRegistry::default();
        let board = registry.parse_file("MS-V375.cad", &data).expect("parse cad");

        let net = board.nets.get("IFPC_TXC*").expect("find IFPC_TXC*");
        println!("IFPC_TXC* pin_indices: {:?}", net.pin_indices);
        println!("IFPC_TXC* via_indices: {:?}", net.via_indices);

        let mut vp = Viewport::default();
        let bbox = board.bounding_box.unwrap();
        let canvas_rect = EguiRect::from_min_size(Pos2::ZERO, egui::vec2(1920.0, 1080.0));
        vp.fit_to_rect(bbox, canvas_rect);

        let tokens = Tokens::DARK;
        let mut renderer = Renderer::new();

        let c35_pin_idx = net.pin_indices[1]; // Pin 7768 (C35-1)
        let meshes = generate_board_meshes(
            &board,
            &vp,
            canvas_rect,
            &tokens,
            Some(c35_pin_idx),
            None,
            Some("IFPC_TXC*"),
            None,
            None,
            &mut renderer,
            true, // show_net_web
            false, // show_opposite_elements
        );

        println!("Generated {} meshes for whole board view", meshes.len());
        let last_mesh = meshes.last().unwrap();
        println!("Net web vertices: {}, indices: {}", last_mesh.vertices.len(), last_mesh.indices.len());
    }

    #[test]
    fn bench_dense_net_tessellation() {
        let mut board = Board::new();

        let num_pins = 2481;
        let num_vias = 2274;

        board.pins.reserve(num_pins);
        for i in 0..num_pins {
            let x = ((i * 13) % 8000) as f64;
            let y = ((i * 29) % 6000) as f64;
            board.pins.push(Pin {
                id: i,
                part_id: 0,
                name: format!("P{i}"),
                pos: Point::new(x, y),
                side: if i % 2 == 0 { Side::Top } else { Side::Bottom },
                net_name: "GND".into(),
                radius: 15.0,
            });
        }

        board.vias.reserve(num_vias);
        for i in 0..num_vias {
            let x = ((i * 17) % 8000) as f64;
            let y = ((i * 37) % 6000) as f64;
            board.vias.push(Via {
                id: i,
                pos: Point::new(x, y),
                side: Side::Both,
                net_name: "GND".into(),
                radius: 12.0,
            });
        }

        board.rebuild_indices();

        let mut vp = Viewport::default();
        let bbox = board.bounding_box.unwrap();
        let canvas_rect = EguiRect::from_min_size(Pos2::ZERO, egui::vec2(1920.0, 1080.0));
        vp.fit_to_rect(bbox, canvas_rect);

        let tokens = Tokens::DARK;
        let mut renderer = Renderer::new();

        // Warmup pass
        let _ = generate_board_meshes(
            &board,
            &vp,
            canvas_rect,
            &tokens,
            None,
            None,
            Some("GND"),
            None,
            None,
            &mut renderer,
            true,
            false,
        );

        // Benchmark pass
        let start = Instant::now();
        let meshes = generate_board_meshes(
            &board,
            &vp,
            canvas_rect,
            &tokens,
            None,
            None,
            Some("GND"),
            None,
            None,
            &mut renderer,
            true,
            false,
        );
        let elapsed = start.elapsed();

        assert!(!meshes.is_empty(), "Meshes must be produced for GND");
        println!("Dense net tessellation (4,755 GND nodes) took {:?}", elapsed);
        assert!(elapsed.as_millis() < 80, "Tessellation took {:?}, expected < 80ms in debug build", elapsed);
    }

    #[test]
    fn test_viewport_bijective_invariance() {
        let canvas_rect = EguiRect::from_min_size(Pos2::new(100.0, 50.0), egui::vec2(1280.0, 800.0));
        let sample_points = [
            Point::new(0.0, 0.0),
            Point::new(1234.56, -789.12),
            Point::new(-4567.89, 3210.45),
            Point::new(500.0, 500.0),
        ];

        let rotations = [
            Rotation::Deg0,
            Rotation::Deg90,
            Rotation::Deg180,
            Rotation::Deg270,
        ];
        let sides = [Side::Top, Side::Bottom];
        let mirrors = [false, true];

        for &rotation in &rotations {
            for &side in &sides {
                for &mirror_x in &mirrors {
                    for &mirror_y in &mirrors {
                        let vp = Viewport {
                            center: Point::new(250.0, -150.0),
                            scale: 0.125,
                            side,
                            rotation,
                            mirror_x,
                            mirror_y,
                        };

                        for &p in &sample_points {
                            let screen = vp.world_to_screen(p, canvas_rect);
                            let recovered = vp.screen_to_world(screen, canvas_rect);
                            let err = ((recovered.x - p.x).powi(2) + (recovered.y - p.y).powi(2)).sqrt();
                            assert!(
                                err < 1e-3,
                                "Bijective invariance failed for p={:?}, rot={:?}, side={:?}, mx={}, my={}: err={}",
                                p, rotation, side, mirror_x, mirror_y, err
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn test_zoom_presets_and_fit_to_rect() {
        let mut vp = Viewport::default();
        let canvas_rect = EguiRect::from_min_size(Pos2::ZERO, egui::vec2(1000.0, 1000.0));
        let bbox = Rect {
            min_x: 0.0,
            min_y: 0.0,
            max_x: 1000.0,
            max_y: 500.0,
        };

        // Fit to rect should fit 90% of view
        vp.fit_to_rect(bbox, canvas_rect);
        assert_eq!(vp.center, Point::new(500.0, 250.0));
        // Available width with margin = (1000 - 40) * 0.9 = 864. scale_x = 864 / 1000 = 0.864
        // Available height with margin = (1000 - 40) * 0.9 = 864. scale_y = 864 / 500 = 1.728
        // Expected scale = 0.864
        assert!((vp.scale - 0.864).abs() < 1e-4);

        // Zoom presets
        vp.zoom_preset(0.25, canvas_rect);
        assert!((vp.scale - 0.0125).abs() < 1e-5);

        vp.zoom_preset(0.50, canvas_rect);
        assert!((vp.scale - 0.0250).abs() < 1e-5);

        vp.zoom_preset(1.0, canvas_rect);
        assert!((vp.scale - 0.0500).abs() < 1e-5);

        vp.zoom_preset(1.50, canvas_rect);
        assert!((vp.scale - 0.0750).abs() < 1e-5);

        vp.zoom_preset(2.0, canvas_rect);
        assert!((vp.scale - 0.1000).abs() < 1e-5);
    }

    #[test]
    fn test_screen_space_lod_thresholds() {
        let scale = 0.05; // 100% zoom
        // Test part bounding box thresholds
        // w_screen = bbox.width() * scale, h_screen = bbox.height() * scale
        // w_screen < 24.0 || h_screen < 12.0
        let small_bbox = Rect { min_x: 0.0, min_y: 0.0, max_x: 400.0, max_y: 200.0 }; // w_screen = 20.0, h_screen = 10.0 -> decimated
        let w_screen_small = small_bbox.width() as f32 * scale;
        let h_screen_small = small_bbox.height() as f32 * scale;
        assert!(w_screen_small < 24.0 || h_screen_small < 12.0);

        let large_bbox = Rect { min_x: 0.0, min_y: 0.0, max_x: 600.0, max_y: 300.0 }; // w_screen = 30.0, h_screen = 15.0 -> rendered
        let w_screen_large = large_bbox.width() as f32 * scale;
        let h_screen_large = large_bbox.height() as f32 * scale;
        assert!(w_screen_large >= 24.0 && h_screen_large >= 12.0);

        // Test pin radius threshold: r_screen = radius * scale < 8.0
        let small_pin_radius = 120.0; // r_screen = 6.0 -> decimated
        assert!(small_pin_radius as f32 * scale < 8.0);

        let large_pin_radius = 200.0; // r_screen = 10.0 -> rendered
        assert!(large_pin_radius as f32 * scale >= 8.0);
    }

    #[test]
    fn test_pan_delta_follows_cursor() {
        let mut vp = Viewport::default();
        let canvas_rect = EguiRect::from_min_size(Pos2::new(100.0, 100.0), egui::vec2(800.0, 600.0));
        let center_screen = canvas_rect.center();

        let initial_center = vp.center;
        // Drag mouse down (+y in screen space)
        let drag_down = egui::vec2(0.0, 50.0);
        vp.center = vp.screen_to_world(center_screen - drag_down, canvas_rect);

        // A world point initially at center_screen should now be shifted down (+50.0) in screen space
        let screen_pt_after = vp.world_to_screen(initial_center, canvas_rect);
        assert!((screen_pt_after.x - center_screen.x).abs() < 1e-3);
        assert!((screen_pt_after.y - (center_screen.y + 50.0)).abs() < 1e-3);

        // Reset center and drag mouse right (+x in screen space)
        vp.center = initial_center;
        let drag_right = egui::vec2(75.0, 0.0);
        vp.center = vp.screen_to_world(center_screen - drag_right, canvas_rect);
        let screen_pt_after2 = vp.world_to_screen(initial_center, canvas_rect);
        assert!((screen_pt_after2.x - (center_screen.x + 75.0)).abs() < 1e-3);
        assert!((screen_pt_after2.y - center_screen.y).abs() < 1e-3);
    }
}

