//! Board canvas view transformation and interactive rendering.

use crate::keymap::{Action, Keymap};
use crate::overlay::OverlayManager;
use crate::overlay_align::{AlignmentSession, AlignmentState};
use crate::preferences::{Preferences, ViaDisplayMode};
use crate::render::Renderer;
use crate::theme::Tokens;
use bvc_core::{Board, Point, Rect, Side};
use egui::epaint::Mesh;
use egui::{Color32, FontId, Pos2, Rect as EguiRect, Response, RichText, Sense, Shape, Stroke, Ui};
use rayon::prelude::*;
use std::sync::Arc;

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
    pub fn effective_factors(&self) -> (f32, f32) {
        let is_bottom_flipped = self.side == Side::Bottom;
        let sx = if is_bottom_flipped ^ self.mirror_x {
            -1.0
        } else {
            1.0
        };
        let sy = if self.mirror_y { -1.0 } else { 1.0 };
        (sx, sy)
    }

    /// Transforms world point to screen point with custom reflection factors and custom rotation angle in radians
    pub fn world_to_screen_with_angle(&self, pt: Point, canvas_rect: EguiRect, sx: f32, sy: f32, theta_rad: f32) -> Pos2 {
        let center_screen = canvas_rect.center();

        // 1. Offset relative to board center and apply reflections
        let dx = (pt.x - self.center.x) as f32 * sx;
        let dy = (pt.y - self.center.y) as f32 * sy;

        // 2. CAD-to-Screen Y inversion (CAD +Y is up, screen +Y is down)
        let dy_cad = -dy;

        // 3. 2D rotation in screen plane
        let (sin_t, cos_t) = theta_rad.sin_cos();
        let x_rot = dx * cos_t - dy_cad * sin_t;
        let y_rot = dx * sin_t + dy_cad * cos_t;

        // 4. Uniform scaling and translation to canvas center
        let screen_x = center_screen.x + x_rot * self.scale;
        let screen_y = center_screen.y + y_rot * self.scale;

        Pos2::new(screen_x, screen_y)
    }

    /// Transforms world point to screen point with custom reflection factors and custom rotation
    pub fn world_to_screen_with_transform(&self, pt: Point, canvas_rect: EguiRect, sx: f32, sy: f32, rotation: Rotation) -> Pos2 {
        self.world_to_screen_with_angle(pt, canvas_rect, sx, sy, rotation.angle_rad())
    }

    /// Transforms world point to screen point with custom reflection factors
    pub fn world_to_screen_with_factors(&self, pt: Point, canvas_rect: EguiRect, sx: f32, sy: f32) -> Pos2 {
        self.world_to_screen_with_transform(pt, canvas_rect, sx, sy, self.rotation)
    }

    /// Transforms world point to screen point.
    pub fn world_to_screen(&self, pt: Point, canvas_rect: EguiRect) -> Pos2 {
        let (sx, sy) = self.effective_factors();
        self.world_to_screen_with_factors(pt, canvas_rect, sx, sy)
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

/// Everything the board meshes depend on except zoom, pan and the canvas rectangle.
///
/// Zoom and pan steps, and showing the same side in a canvas of another size or position (a
/// board tab vs one half of the Both tab), re-map the cached meshes (`BoardMeshBase`) instead of
/// re-tessellating, so the view centre, scale and canvas rectangle are not part of the key; a
/// rebuild for them is decided by `BoardMeshBase::covers`.
#[derive(Clone, PartialEq, Debug)]
pub struct ViewCacheKey {
    pub side: Side,
    pub rotation: Rotation,
    pub mirror_x: bool,
    pub mirror_y: bool,
    pub selected_pin: Option<usize>,
    pub selected_via: Option<usize>,
    pub selected_net: Option<String>,
    pub selected_part: Option<usize>,
    pub board_ptr: usize,
    pub show_net_web: bool,
    pub via_display_mode: ViaDisplayMode,
    pub show_opposite_elements: bool,
    pub show_vias: bool,
    pub show_unnamed_vias: bool,
    pub show_pins: bool,
}

impl ViewCacheKey {
    /// Key of the cached board meshes for this view, selection and display preferences.
    ///
    /// Hover is deliberately not part of the key: the hovered pin or via is drawn by a small
    /// per-frame overlay (`hover_overlay_mesh`), so moving the mouse never rebuilds the board.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        b: &Board,
        vp: &Viewport,
        selected_pin: Option<usize>,
        selected_via: Option<usize>,
        selected_part: Option<usize>,
        selected_net: Option<&str>,
        prefs: &Preferences,
    ) -> Self {
        Self {
            side: vp.side,
            rotation: vp.rotation,
            mirror_x: vp.mirror_x,
            mirror_y: vp.mirror_y,
            selected_pin,
            selected_via,
            selected_net: selected_net.map(str::to_owned),
            selected_part,
            board_ptr: b as *const Board as usize,
            show_net_web: prefs.show_net_web,
            via_display_mode: prefs.via_display_mode,
            show_opposite_elements: prefs.show_opposite_elements,
            show_vias: prefs.show_vias,
            show_unnamed_vias: prefs.show_unnamed_vias,
            show_pins: prefs.show_pins,
        }
    }
}

/// Largest zoom ratio (either way) between the view a mesh base was tessellated for and the
/// displayed view before it is rebuilt. Within it, screen-space minimum sizes (pads >= 1.5 px,
/// tiny-part markers, stroke widths) scale with the zoom and snap back on the next rebuild.
pub const MESH_ZOOM_BUCKET: f32 = std::f32::consts::SQRT_2;

/// World region whose geometry a mesh base holds: the culled view grown by half the visible size
/// on every side, so zooming out within `MESH_ZOOM_BUCKET` or panning by up to half a screen
/// stays inside it.
pub fn mesh_coverage(vp: &Viewport, rect: EguiRect) -> Rect {
    let visible = vp.visible_world_rect(rect);
    view_cull_rect(vp, rect).expand(visible.width().max(visible.height()) * 0.5)
}

/// World region culled for drawing: the visible rectangle plus a margin for pads and bodies whose
/// anchor lies just outside it.
pub fn view_cull_rect(vp: &Viewport, rect: EguiRect) -> Rect {
    let margin_mils = (50.0 / vp.scale).max(40.0) as f64;
    vp.visible_world_rect(rect).expand(margin_mils)
}

fn rect_contains_rect(outer: &Rect, inner: &Rect) -> bool {
    outer.min_x <= inner.min_x && outer.max_x >= inner.max_x && outer.min_y <= inner.min_y && outer.max_y >= inner.max_y
}

/// Board meshes tessellated once in screen space for one view, plus what is needed to re-map
/// them to a zoomed or panned view with the same side, rotation and mirroring.
#[derive(Clone)]
pub struct BoardMeshBase {
    pub meshes: Vec<Arc<Mesh>>,
    /// View centre (world) and scale the meshes were built for.
    pub center: Point,
    pub scale: f32,
    /// Screen position of `center` when built (the canvas centre).
    pub screen_center: Pos2,
    /// World region whose geometry the meshes contain.
    pub coverage: Rect,
}

impl BoardMeshBase {
    /// Whether re-mapping the meshes to `vp` is exact: at the scale they were tessellated for,
    /// the re-map is a pure translation (pan or another canvas position), which keeps the
    /// screen-space minimum sizes right. Only a zoomed re-map needs a fresh tessellation.
    pub fn is_exact_for(&self, vp: &Viewport) -> bool {
        self.scale == vp.scale
    }

    /// Whether the meshes can be re-mapped to `vp` without visible loss: the zoom ratio stays in
    /// the bucket and the culled view lies inside the tessellated region.
    pub fn covers(&self, vp: &Viewport, rect: EguiRect) -> bool {
        let ratio = vp.scale / self.scale;
        (1.0 / MESH_ZOOM_BUCKET..=MESH_ZOOM_BUCKET).contains(&ratio)
            && rect_contains_rect(&self.coverage, &view_cull_rect(vp, rect))
    }

    /// The meshes moved to `vp`. With side, rotation and mirroring unchanged, the old-screen to
    /// new-screen map is a uniform scale about the old canvas centre plus a translation, so one
    /// multiply-add per vertex replaces the whole tessellation.
    pub fn remap(&self, vp: &Viewport, rect: EguiRect) -> Vec<Arc<Mesh>> {
        let k = vp.scale / self.scale;
        let target = vp.world_to_screen(self.center, rect);
        if k == 1.0 && target == self.screen_center {
            return self.meshes.clone();
        }
        let origin = self.screen_center;
        self.meshes
            .iter()
            .map(|mesh| {
                Arc::new(Mesh {
                    indices: mesh.indices.clone(),
                    vertices: mesh
                        .vertices
                        .iter()
                        .map(|v| egui::epaint::Vertex { pos: target + (v.pos - origin) * k, ..*v })
                        .collect(),
                    texture_id: mesh.texture_id,
                })
            })
            .collect()
    }
}

/// What `sync_board_meshes` did to bring the displayed meshes up to date.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeshSync {
    /// Same view as the previous frame: nothing to do.
    Kept,
    /// Zoom or pan within the cached base: vertices re-mapped, no tessellation.
    Remapped,
    /// Key changed, zoom left the bucket or the view left the covered region: re-tessellated.
    Rebuilt,
}

/// The displayed view: centre, scale and canvas centre.
type DisplayedView = (Point, u32, Pos2);

/// 2D Point-to-Point Caliper measurement tool state.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MeasurementTool {
    pub active: bool,
    pub start: Option<Point>,
    pub end: Option<Point>,
}

impl MeasurementTool {
    pub fn is_measuring(&self) -> bool {
        self.active && self.start.is_some() && self.end.is_none()
    }

    pub fn has_measurement(&self) -> bool {
        self.start.is_some() && self.end.is_some()
    }

    pub fn reset(&mut self) {
        self.start = None;
        self.end = None;
    }

    pub fn toggle(&mut self) {
        self.active = !self.active;
        if !self.active {
            self.reset();
        }
    }
}

pub struct CanvasState {
    pub viewport: Viewport,
    pub hovered_pin: Option<usize>,
    pub hovered_via: Option<usize>,
    /// Board meshes for the displayed view, shared with the painter without copying.
    pub cached_meshes: Vec<Arc<Mesh>>,
    /// Key of `mesh_base`; set to `None` to force a rebuild on the next frame.
    pub cached_view_key: Option<ViewCacheKey>,
    /// Last tessellated meshes, re-mapped for zoom and pan steps.
    pub mesh_base: Option<BoardMeshBase>,
    displayed_view: Option<DisplayedView>,
    pub context_menu_pin: Option<(usize, Pos2)>,
    pub context_menu_part: Option<(usize, Pos2)>,
    pub navigate_to_schematic: Option<String>,
    pub measurement: MeasurementTool,
}

/// Brings `state.cached_meshes` up to date for `vp`: keeps them, re-maps the cached base for a
/// zoom or pan step, or calls `build` with the world region to tessellate. After `Remapped` the
/// caller requests one more frame so that a view that stopped moving gets rebuilt exactly.
pub fn sync_board_meshes(
    state: &mut CanvasState,
    key: ViewCacheKey,
    vp: &Viewport,
    rect: EguiRect,
    build: impl FnOnce(Rect) -> Vec<Mesh>,
) -> MeshSync {
    let view: DisplayedView = (vp.center, vp.scale.to_bits(), rect.center());
    if state.cached_view_key.as_ref() == Some(&key) {
        if state.displayed_view == Some(view) {
            // Zoom-re-mapped meshes are exact for true-size geometry but scale the screen-space
            // minimum sizes; once the view rests for a frame it is tessellated for real.
            let settled = state.mesh_base.as_ref().is_some_and(|base| base.is_exact_for(vp));
            if settled {
                return MeshSync::Kept;
            }
        } else if let Some(base) = state.mesh_base.as_ref().filter(|base| base.covers(vp, rect)) {
            state.cached_meshes = base.remap(vp, rect);
            state.displayed_view = Some(view);
            return MeshSync::Remapped;
        }
    }
    let coverage = mesh_coverage(vp, rect);
    let meshes: Vec<Arc<Mesh>> = build(coverage).into_iter().map(Arc::new).collect();
    state.cached_meshes = meshes.clone();
    state.mesh_base = Some(BoardMeshBase { meshes, center: vp.center, scale: vp.scale, screen_center: rect.center(), coverage });
    state.cached_view_key = Some(key);
    state.displayed_view = Some(view);
    MeshSync::Rebuilt
}

impl Default for CanvasState {
    fn default() -> Self {
        Self {
            viewport: Viewport::default(),
            hovered_pin: None,
            hovered_via: None,
            cached_meshes: Vec::new(),
            cached_view_key: None,
            mesh_base: None,
            displayed_view: None,
            context_menu_pin: None,
            context_menu_part: None,
            navigate_to_schematic: None,
            measurement: MeasurementTool::default(),
        }
    }
}

impl CanvasState {
    /// A canvas fixed on `side`.
    pub fn for_side(side: Side) -> Self {
        Self { viewport: Viewport { side, ..Viewport::default() }, ..Self::default() }
    }

    /// Drops everything tied to the current board: meshes, hover and the pin context menu.
    pub fn clear_board(&mut self) {
        self.cached_meshes.clear();
        self.mesh_base = None;
        self.cached_view_key = None;
        self.displayed_view = None;
        self.hovered_pin = None;
        self.hovered_via = None;
        self.context_menu_pin = None;
        self.context_menu_part = None;
        self.navigate_to_schematic = None;
        self.measurement.reset();
    }
}

/// One board view per side, each with its own pan, zoom and mesh cache, so showing a side at
/// another size or switching which sides are visible re-uses its meshes. Rotation and
/// mirroring are shared by both views.
pub struct BoardCanvases {
    pub top: CanvasState,
    pub bottom: CanvasState,
    /// Side of the canvas last hovered or clicked: it receives keyboard pan/zoom, the overlay
    /// wheel and the pin context menu.
    pub active: Side,
}

impl Default for BoardCanvases {
    fn default() -> Self {
        Self { top: CanvasState::for_side(Side::Top), bottom: CanvasState::for_side(Side::Bottom), active: Side::Top }
    }
}

impl BoardCanvases {
    /// The canvas showing `side` (`Both` maps to the top one).
    pub fn side(&self, side: Side) -> &CanvasState {
        match side {
            Side::Bottom => &self.bottom,
            _ => &self.top,
        }
    }

    pub fn side_mut(&mut self, side: Side) -> &mut CanvasState {
        match side {
            Side::Bottom => &mut self.bottom,
            _ => &mut self.top,
        }
    }

    pub fn active_state(&self) -> &CanvasState {
        self.side(self.active)
    }

    pub fn active_mut(&mut self) -> &mut CanvasState {
        self.side_mut(self.active)
    }

    fn each_mut(&mut self) -> [&mut CanvasState; 2] {
        [&mut self.top, &mut self.bottom]
    }

    pub fn rotation(&self) -> Rotation {
        self.top.viewport.rotation
    }

    pub fn mirror_x(&self) -> bool {
        self.top.viewport.mirror_x
    }

    pub fn mirror_y(&self) -> bool {
        self.top.viewport.mirror_y
    }

    pub fn set_rotation(&mut self, rotation: Rotation) {
        for state in self.each_mut() {
            state.viewport.rotation = rotation;
        }
    }

    pub fn rotate_cw(&mut self) {
        self.set_rotation(self.rotation().rotate_cw());
    }

    pub fn rotate_ccw(&mut self) {
        self.set_rotation(self.rotation().rotate_ccw());
    }

    pub fn rotate_180(&mut self) {
        self.set_rotation(self.rotation().rotate_180());
    }

    pub fn toggle_mirror_x(&mut self) {
        let mirror = !self.mirror_x();
        for state in self.each_mut() {
            state.viewport.mirror_x = mirror;
        }
    }

    pub fn toggle_mirror_y(&mut self) {
        let mirror = !self.mirror_y();
        for state in self.each_mut() {
            state.viewport.mirror_y = mirror;
        }
    }

    pub fn toggle_measure(&mut self) {
        let active = !self.active_state().measurement.active;
        for state in self.each_mut() {
            state.measurement.active = active;
            if !active {
                state.measurement.reset();
            }
        }
    }

    pub fn clear_measurement(&mut self) {
        for state in self.each_mut() {
            state.measurement.reset();
        }
    }

    /// No rotation and no mirroring; each canvas stays on its side.
    pub fn reset_orientation(&mut self) {
        for state in self.each_mut() {
            state.viewport.rotation = Rotation::Deg0;
            state.viewport.mirror_x = false;
            state.viewport.mirror_y = false;
        }
    }

    /// Forces both sides to re-tessellate on their next frame (selection or display change).
    pub fn invalidate_caches(&mut self) {
        for state in self.each_mut() {
            state.cached_view_key = None;
        }
    }

    /// Drops everything tied to the current board on both sides.
    pub fn clear_board(&mut self) {
        for state in self.each_mut() {
            state.clear_board();
        }
    }

    /// Centers both views on `center` (a newly loaded board).
    pub fn center_all(&mut self, center: Point) {
        for state in self.each_mut() {
            state.viewport.center = center;
        }
    }
}

/// Whether this frame's interaction makes a canvas the active one: a click, right-click or the
/// start of a drag. Hovering alone does not, so crossing a canvas with the pointer (e.g. on the
/// way to the side panel) never retargets the keyboard or the overlay tools.
pub fn activates_canvas(clicked: bool, secondary_clicked: bool, drag_started: bool) -> bool {
    clicked || secondary_clicked || drag_started
}

/// Alpha of the overlay photo and opacity factor of the board drawn over it. With a photo
/// shown, the photo stays opaque and the opacity setting fades the board on top of it, so the
/// pins can be lined up with the photo; without one, the board is opaque.
fn overlay_layer_alphas(photo_shown: bool, opacity: f32) -> (u8, f32) {
    let board = if photo_shown { opacity.clamp(0.0, 1.0) } else { 1.0 };
    (255, board)
}

/// Whether a canvas takes keyboard/wheel input and keeps its pin menu this frame: the active
/// canvas, or one clicked right now (the caller only marks it active after it is drawn, and a
/// touch or pen click has no hover frame before it).
fn canvas_takes_input(is_active: bool, clicked: bool, secondary_clicked: bool) -> bool {
    is_active || clicked || secondary_clicked
}

/// Keyboard pan direction (screen units, -1..1 per axis) and zoom in/out requests for this
/// frame, read from the keymap's pan and zoom actions.
fn keyboard_pan_zoom(keymap: &Keymap, input: &egui::InputState) -> (egui::Vec2, bool, bool) {
    let mut pan = egui::Vec2::ZERO;
    if keymap.pressed(Action::PanUp, input) { pan.y -= 1.0; }
    if keymap.pressed(Action::PanDown, input) { pan.y += 1.0; }
    if keymap.pressed(Action::PanLeft, input) { pan.x -= 1.0; }
    if keymap.pressed(Action::PanRight, input) { pan.x += 1.0; }
    (pan, keymap.pressed(Action::ZoomIn, input), keymap.pressed(Action::ZoomOut, input))
}

/// Draws and handles one board canvas in `rect`. Only the active canvas (`is_active`) takes
/// keyboard pan/zoom, the overlay wheel and shows the pin context menu, so two canvases on
/// screen never react to the same key twice.
#[allow(clippy::too_many_arguments)]
pub fn show_canvas(
    ui: &mut Ui,
    rect: EguiRect,
    is_active: bool,
    tokens: &Tokens,
    board: Option<&Board>,
    canvas_state: &mut CanvasState,
    overlay_manager: &mut OverlayManager,
    alignment_session: &mut AlignmentSession,
    overlay_mode_active: bool,
    selected_pin_idx: &mut Option<usize>,
    selected_via_idx: &mut Option<usize>,
    selected_part_idx: &mut Option<usize>,
    selected_net: &mut Option<String>,
    renderer: &mut Renderer,
    prefs: &Preferences,
) -> Response {
    // A stable id per side keeps drag state across frames whatever else is laid out around it.
    let canvas_id = egui::Id::unique(("board_canvas", canvas_state.viewport.side));
    let response = ui.interact(rect, canvas_id, Sense::click_and_drag());
    ui.advance_cursor_after_rect(rect);
    let is_active = canvas_takes_input(is_active, response.clicked(), response.secondary_clicked());
    let painter = ui.painter_at(rect);

    // Background fill
    painter.rect_filled(rect, 0.0, tokens.canvas);

    let vp = &mut canvas_state.viewport;

    // Direct Overlay Manipulation:
    // Active when either Alt is held, OR the right-panel Overlay tab is active (and overlay is loaded and unlocked),
    // and calibration session is NOT actively capturing clicks.
    let is_alt_down = ui.input(|i| i.modifiers.alt);
    let active_slot = overlay_manager.slot_for_side(vp.side);
    // Alignment clicks belong to the canvas of the side being aligned only.
    let is_calibrating = alignment_session.is_active() && alignment_session.side.is_none_or(|s| s == vp.side);
    let is_overlay_interacting = !is_calibrating && (is_alt_down || overlay_mode_active) && active_slot.is_loaded() && !active_slot.locked;

    if is_overlay_interacting {
        if response.dragged_by(egui::PointerButton::Primary) || response.dragged_by(egui::PointerButton::Secondary) {
            let drag_screen = response.drag_delta();
            // Convert screen drag delta to world delta
            let (sx, sy) = vp.effective_factors();
            let theta = vp.rotation.angle_rad();
            let (sin_t, cos_t) = theta.sin_cos();

            let dx_rot = drag_screen.x / vp.scale;
            let dy_rot = drag_screen.y / vp.scale;
            let dx_cad = dx_rot * cos_t + dy_rot * sin_t;
            let dy_cad = -dx_rot * sin_t + dy_rot * cos_t;
            let dy = -dy_cad;

            active_slot.center.x += (dx_cad / sx) as f64;
            active_slot.center.y += (dy / sy) as f64;
            ui.ctx().request_repaint();
        }

        let scroll_y = if is_active { ui.input(|i| i.smooth_scroll_delta.y) } else { 0.0 };
        if scroll_y != 0.0 {
            let scale_factor = if scroll_y > 0.0 { 1.05 } else { 1.0 / 1.05 };
            let new_zoom = active_slot.zoom * scale_factor;
            active_slot.set_zoom(new_zoom);
            ui.ctx().request_repaint();
        }
    }

    // Render Overlay Photo Quad (behind board elements)
    if active_slot.visible {
        if let Some(ref texture) = active_slot.texture {
            let half_w = (active_slot.size.x * 0.5) as f64;
            let half_h = (active_slot.size.y * 0.5) as f64;
            let fine_rad = active_slot.fine_rotation_deg.to_radians();
            let (sin_f, cos_f) = fine_rad.sin_cos();

            // Corner 0: top-left (uv 0,0) in CAD space (+Y up, -Y down)
            let local_corners = [
                (-half_w, half_h),
                (half_w, half_h),
                (half_w, -half_h),
                (-half_w, -half_h),
            ];

            let world_corners: Vec<Point> = local_corners
                .iter()
                .map(|&(lx, ly)| {
                    let rx = lx * (cos_f as f64) - ly * (sin_f as f64);
                    let ry = lx * (sin_f as f64) + ly * (cos_f as f64);
                    Point::new(active_slot.center.x + rx, active_slot.center.y + ry)
                })
                .collect();

            let (final_sx, final_sy, final_rot_rad) = active_slot.overlay_factors(vp);

            let p0 = vp.world_to_screen_with_angle(world_corners[0], rect, final_sx, final_sy, final_rot_rad);
            let p1 = vp.world_to_screen_with_angle(world_corners[1], rect, final_sx, final_sy, final_rot_rad);
            let p2 = vp.world_to_screen_with_angle(world_corners[2], rect, final_sx, final_sy, final_rot_rad);
            let p3 = vp.world_to_screen_with_angle(world_corners[3], rect, final_sx, final_sy, final_rot_rad);

            let (alpha, _) = overlay_layer_alphas(true, active_slot.opacity);
            let tint = Color32::from_white_alpha(alpha);

            let mut img_mesh = Mesh::with_texture(texture.id());
            img_mesh.add_rect_with_uv(
                EguiRect::from_min_max(Pos2::ZERO, Pos2::ZERO), // dummy rect, custom vertices below
                EguiRect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0)),
                tint,
            );
            // Replace dummy vertices with projected quad
            img_mesh.vertices.clear();
            img_mesh.indices.clear();

            let uv0 = Pos2::new(0.0, 0.0);
            let uv1 = Pos2::new(1.0, 0.0);
            let uv2 = Pos2::new(1.0, 1.0);
            let uv3 = Pos2::new(0.0, 1.0);

            let idx0 = img_mesh.vertices.len() as u32;
            img_mesh.vertices.push(egui::epaint::Vertex { pos: p0, uv: uv0, color: tint });
            img_mesh.vertices.push(egui::epaint::Vertex { pos: p1, uv: uv1, color: tint });
            img_mesh.vertices.push(egui::epaint::Vertex { pos: p2, uv: uv2, color: tint });
            img_mesh.vertices.push(egui::epaint::Vertex { pos: p3, uv: uv3, color: tint });

            img_mesh.indices.extend_from_slice(&[idx0, idx0 + 1, idx0 + 2, idx0, idx0 + 2, idx0 + 3]);
            painter.add(img_mesh);
        }
    }

    // Grid rendering (cadcraft style)
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

    // Interactive Pan (Middle click or Right click drag, or left-drag if not clicking an element or moving overlay)
    let should_pan_canvas = !is_overlay_interacting
        && (response.dragged_by(egui::PointerButton::Middle)
            || response.dragged_by(egui::PointerButton::Secondary)
            || response.dragged_by(egui::PointerButton::Primary));

    if should_pan_canvas {
        let delta = response.drag_delta();
        // Move viewport center so the point under the mouse follows the cursor exactly:
        // Screen point center_screen shifted by -delta corresponds to the new world center.
        let center_screen = rect.center();
        vp.center = vp.screen_to_world(center_screen - delta, rect);
    }

    // Keyboard pan and zoom (keymap actions, arrows and + / - by default) when not typing in text boxes
    let wants_keyboard = ui.ctx().egui_wants_keyboard_input() || ui.memory(|m| m.focused().is_some());
    if is_active && !wants_keyboard {
        let (pan_delta, plus, minus) = ui.input(|i| keyboard_pan_zoom(&prefs.keymap, i));

        let pan_step = (50.0 / vp.scale) as f64;
        let (sx, sy) = vp.effective_factors();
        let rot_rad = vp.rotation.angle_rad();
        let cos = rot_rad.cos() as f64;
        let sin = rot_rad.sin() as f64;

        if pan_delta != egui::Vec2::ZERO {
            let dx_screen = pan_delta.x as f64 * pan_step;
            let dy_screen = pan_delta.y as f64 * pan_step;

            // Invert rotation and mirroring to shift world center accurately
            let dx_rot = dx_screen / (sx as f64);
            let dy_rot = dy_screen / (sy as f64);
            let dx_world = dx_rot * cos + dy_rot * sin;
            let dy_world = -dx_rot * sin + dy_rot * cos;

            vp.center.x += dx_world;
            vp.center.y += dy_world;
            ui.ctx().request_repaint();
        }

        if plus {
            vp.scale = (vp.scale * 1.25).clamp(0.001, 100.0);
            ui.ctx().request_repaint();
        }
        if minus {
            vp.scale = (vp.scale / 1.25).clamp(0.001, 100.0);
            ui.ctx().request_repaint();
        }
    }

    // Zoom on mouse wheel (only zooms PCB viewport when NOT scaling the overlay)
    let scroll_y = ui.input(|i| i.smooth_scroll_delta.y);
    if scroll_y != 0.0 && !is_overlay_interacting {
        if let Some(hover_pos) = response.hover_pos() {
            let mouse_world_before = vp.screen_to_world(hover_pos, rect);
            let line_points = ui.ctx().options(|o| o.input_options.line_scroll_speed);
            vp.scale = (vp.scale * wheel_zoom_factor(scroll_y, line_points)).clamp(0.001, 100.0);
            let mouse_world_after = vp.screen_to_world(hover_pos, rect);

            vp.center.x += mouse_world_before.x - mouse_world_after.x;
            vp.center.y += mouse_world_before.y - mouse_world_after.y;
        }
    }

    // Hover detection (only detect pins/vias if their display is enabled in preferences)
    let mut current_hover_pin: Option<usize> = None;
    let mut current_hover_via: Option<usize> = None;

    if let Some(hover_pos) = response.hover_pos() {
        if let Some(b) = board {
            let world_pos = vp.screen_to_world(hover_pos, rect);
            let hit_tolerance = (15.0 / vp.scale) as f64;
            if prefs.show_pins {
                current_hover_pin = b.find_pin_near(world_pos, vp.side, hit_tolerance);
            }
            if current_hover_pin.is_none() && prefs.show_vias {
                current_hover_via = b.find_via_near(world_pos, vp.side, hit_tolerance);
            }
        }
    }
    canvas_state.hovered_pin = current_hover_pin;
    canvas_state.hovered_via = current_hover_via;

    // Selection and Context Menu on click (or pin alignment calibration / measurement click interceptor)
    let is_ctrl_down = ui.input(|i| i.modifiers.ctrl || i.modifiers.command);
    let secondary_clicked = response.secondary_clicked();
    let is_context_trigger = secondary_clicked || (response.clicked() && is_ctrl_down);

    if canvas_state.measurement.active {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
    }

    if is_calibrating && response.clicked() {
        if let Some(hover_pos) = response.hover_pos() {
            match alignment_session.state.clone() {
                AlignmentState::SelectingCadPin { .. } => {
                    // Try to snap to hovered pin or find closest pin within 25px
                    let snapped_pin = current_hover_pin.or_else(|| {
                        board.and_then(|b| {
                            let world_pos = vp.screen_to_world(hover_pos, rect);
                            let snap_world_dist = (25.0 / vp.scale) as f64;
                            b.find_pin_near(world_pos, vp.side, snap_world_dist)
                        })
                    });

                    if let Some(pin_idx) = snapped_pin {
                        if let Some(b) = board {
                            if let Some(pin) = b.pins.get(pin_idx) {
                                let part_name = b.parts.get(pin.part_id).map(|p| p.name.clone()).unwrap_or_else(|| "?".into());
                                alignment_session.select_cad_pin(pin_idx, pin.name.clone(), part_name, pin.pos);
                                ui.ctx().request_repaint();
                            }
                        }
                    }
                }
                AlignmentState::SelectingPhotoPoint { .. } => {
                    // Store clicked position in invariant photo local coordinates
                    let local_pos = active_slot.screen_to_photo_local(hover_pos, rect, vp);
                    alignment_session.select_photo_point(local_pos);
                    ui.ctx().request_repaint();
                }
                _ => {}
            }
        }
    } else if canvas_state.measurement.active && response.clicked() {
        if let Some(hover_pos) = response.hover_pos() {
            // Snap to pin or via center if close enough (within 15px)
            let snapped_point = current_hover_pin
                .and_then(|idx| board.and_then(|b| b.pins.get(idx).map(|p| p.pos)))
                .or_else(|| {
                    current_hover_via.and_then(|idx| board.and_then(|b| b.vias.get(idx).map(|v| v.pos)))
                })
                .unwrap_or_else(|| vp.screen_to_world(hover_pos, rect));

            if canvas_state.measurement.start.is_none() {
                canvas_state.measurement.start = Some(snapped_point);
                canvas_state.measurement.end = None;
            } else if canvas_state.measurement.end.is_none() {
                canvas_state.measurement.end = Some(snapped_point);
            } else {
                // If a completed measurement was already displayed, restart with a new first point
                canvas_state.measurement.start = Some(snapped_point);
                canvas_state.measurement.end = None;
            }
            ui.ctx().request_repaint();
        }
    } else if is_context_trigger {
        if let Some(pin_idx) = current_hover_pin {
            if let Some(b) = board {
                if let Some(pin) = b.pins.get(pin_idx) {
                    *selected_pin_idx = Some(pin_idx);
                    *selected_via_idx = None;
                    *selected_part_idx = Some(pin.part_id);
                    *selected_net = if pin.net_name.is_empty() {
                        None
                    } else {
                        Some(pin.net_name.clone())
                    };

                    let click_pos = response.hover_pos().unwrap_or(rect.center());
                    canvas_state.context_menu_pin = Some((pin_idx, click_pos));
                    canvas_state.context_menu_part = None;
                }
            }
        } else if let (Some(b), Some(hover_pos)) = (board, response.hover_pos()) {
            let world_pos = vp.screen_to_world(hover_pos, rect);
            if let Some(part_idx) = b.find_part_near(world_pos, vp.side) {
                *selected_pin_idx = None;
                *selected_via_idx = None;
                *selected_part_idx = Some(part_idx);
                *selected_net = None;
                canvas_state.context_menu_pin = None;
                canvas_state.context_menu_part = Some((part_idx, hover_pos));
            } else {
                canvas_state.context_menu_pin = None;
                canvas_state.context_menu_part = None;
            }
        } else {
            canvas_state.context_menu_pin = None;
            canvas_state.context_menu_part = None;
        }
    } else if response.clicked() {
        canvas_state.context_menu_pin = None;
        canvas_state.context_menu_part = None;
        if let Some(pin_idx) = current_hover_pin {
            if *selected_pin_idx == Some(pin_idx) {
                // Clicking the already-selected pin deselects it
                *selected_pin_idx = None;
                *selected_via_idx = None;
                *selected_part_idx = None;
                *selected_net = None;
            } else if let Some(b) = board {
                if let Some(pin) = b.pins.get(pin_idx) {
                    *selected_pin_idx = Some(pin_idx);
                    *selected_via_idx = None;
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
                    if *selected_via_idx == Some(via_idx) || (*selected_pin_idx == None && selected_net.as_deref() == Some(&via.net_name)) {
                        // Clicking the same via deselects
                        *selected_pin_idx = None;
                        *selected_via_idx = None;
                        *selected_net = None;
                    } else {
                        *selected_pin_idx = None;
                        *selected_via_idx = Some(via_idx);
                        *selected_net = if via.net_name.is_empty() {
                            None
                        } else {
                            Some(via.net_name.clone())
                        };
                    }
                }
            }
        } else if let (Some(b), Some(hover_pos)) = (board, response.hover_pos()) {
            // Hit-test component body/bounding box if neither pin nor via was clicked
            let world_pos = vp.screen_to_world(hover_pos, rect);
            if let Some(part_idx) = b.find_part_near(world_pos, vp.side) {
                if *selected_part_idx == Some(part_idx) && selected_pin_idx.is_none() {
                    // Clicking selected part body deselects it
                    *selected_pin_idx = None;
                    *selected_via_idx = None;
                    *selected_part_idx = None;
                    *selected_net = None;
                } else {
                    *selected_pin_idx = None;
                    *selected_via_idx = None;
                    *selected_part_idx = Some(part_idx);
                    *selected_net = None;
                }
            } else {
                // Clicking empty canvas clears selection
                *selected_pin_idx = None;
                *selected_via_idx = None;
                *selected_part_idx = None;
                *selected_net = None;
            }
        }
    }

    // Render Board elements
    if let Some(b) = board {
        let current_key = ViewCacheKey::new(
            b,
            vp,
            *selected_pin_idx,
            *selected_via_idx,
            *selected_part_idx,
            selected_net.as_deref(),
            prefs,
        );

        // Rebuilt meshes have no hover: the hovered pin or via is drawn by the overlay below.
        let view = vp.clone();
        let sync = sync_board_meshes(canvas_state, current_key, &view, rect, |coverage| {
            generate_board_meshes_in(
                b,
                &view,
                rect,
                coverage,
                tokens,
                *selected_pin_idx,
                *selected_via_idx,
                *selected_part_idx,
                selected_net.as_deref(),
                None,
                None,
                renderer,
                prefs.show_net_web,
                prefs.via_display_mode,
                prefs.show_opposite_elements,
                prefs.show_vias,
                prefs.show_unnamed_vias,
                prefs.show_pins,
            )
        });
        let vp = &canvas_state.viewport;
        if sync == MeshSync::Remapped {
            ui.ctx().request_repaint();
        }
        // The board over a shown photo is faded by the overlay opacity (the photo stays opaque).
        let photo_shown = active_slot.visible && active_slot.texture.is_some();
        let (_, board_opacity) = overlay_layer_alphas(photo_shown, active_slot.opacity);
        let mut board_painter = painter.clone();
        board_painter.set_opacity(board_opacity);
        // Emitting a cached mesh only bumps its reference count (a faded board copies it).
        for mesh in &canvas_state.cached_meshes {
            board_painter.add(Shape::Mesh(Arc::clone(mesh)));
        }
        let hover_mesh = hover_overlay_mesh(b, vp, rect, tokens, current_hover_pin, current_hover_via, *selected_pin_idx);
        if !hover_mesh.is_empty() {
            painter.add(hover_mesh);
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

                    // Dynamic font size scaled with component footprint dimensions (clamp between 10.0 and 28.0 pt)
                    let min_dim = w_screen.min(h_screen);
                    let font_size = label_font_size(10.0 + (min_dim - 24.0).max(0.0) * 0.12, 10.0, 28.0);

                    let center = vp.world_to_screen(c_rect.center(), rect);
                    painter.text(
                        center,
                        egui::Align2::CENTER_CENTER,
                        &part.name,
                        FontId::proportional(font_size),
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
            let is_high_density = active_membership.is_high_density();

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

                // Screen-space LOD decimation:
                // 1) The actively selected pin and hovered pin always render text if large enough (> 4px).
                // 2) If the net is high density (e.g. GND/power with many pins), suppress text for all non-selected net pins.
                // 3) Other net pins require at least 8.0px screen radius to avoid drowning the frame in text layouts.
                // 4) Unselected pins require at least 8.0px.
                if !is_selected && !is_hovered {
                    if is_net_selected {
                        if is_high_density || r_screen < 8.0 {
                            continue;
                        }
                    } else if r_screen < 8.0 {
                        continue;
                    }
                } else if r_screen < 4.0 {
                    continue;
                }

                let screen_pos = vp.world_to_screen(pin.pos, rect);

                if is_selected {
                    // For selected pin: show net name (or fallback to pin name), drawn inside circle outline or negative text
                    let display_text = if !pin.net_name.is_empty() {
                        &pin.net_name
                    } else if !pin.name.is_empty() {
                        &pin.name
                    } else {
                        ""
                    };

                    if !display_text.is_empty() {
                        let font_size = label_font_size(r_screen * 0.9, 8.0, 16.0);
                        painter.text(
                            screen_pos,
                            egui::Align2::CENTER_CENTER,
                            display_text,
                            FontId::monospace(font_size),
                            tokens.selection,
                        );
                    }
                } else if !pin.name.is_empty() {
                    let font_size = label_font_size(r_screen * 1.1, 8.0, 14.0);
                    let text_color = if is_hovered {
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

        // Ring around selected pin: drawn as a single outer circle contour
        if let Some(idx) = *selected_pin_idx {
            if let Some(pin) = b.pins.get(idx) {
                let screen_pos = vp.world_to_screen(pin.pos, rect);
                let screen_r = (pin.radius as f32 * vp.scale).clamp(3.0, 40.0);
                painter.circle_stroke(screen_pos, screen_r + 3.0, Stroke::new(2.0, tokens.selection));
            }
        }

        // Render Pin Context Menu Popup if open (only one canvas shows it)
        if !is_active {
            canvas_state.context_menu_pin = None;
            canvas_state.context_menu_part = None;
        }
        if let Some((pin_idx, menu_pos)) = canvas_state.context_menu_pin {
            if let Some(pin) = b.pins.get(pin_idx) {
                let part_name = b
                    .parts
                    .get(pin.part_id)
                    .map(|p| p.name.as_str())
                    .unwrap_or("?");
                let lang = prefs.ui_language();

                let mut close_menu = false;
                let menu_id = egui::Id::unique(("pin_context_menu_area", vp.side));

                egui::Area::new(menu_id)
                    .fixed_pos(menu_pos)
                    .order(egui::Order::Foreground)
                    .show(ui.ctx(), |ui| {
                        egui::Frame::menu(ui.style())
                            .fill(tokens.chrome)
                            .stroke(Stroke::new(1.0, tokens.border))
                            .corner_radius(4.0)
                            .inner_margin(8.0)
                            .show(ui, |ui| {
                                ui.set_min_width(180.0);
                                ui.heading(
                                    RichText::new(format!("{}.{}", part_name, pin.name))
                                        .color(tokens.accent)
                                        .strong(),
                                );
                                ui.separator();

                                ui.horizontal(|ui| {
                                    ui.label(RichText::new(crate::i18n::tr(lang, "context_menu.net")).color(tokens.text_dim));
                                    ui.label(
                                        RichText::new(if pin.net_name.is_empty() {
                                            "UNCONNECTED"
                                        } else {
                                            &pin.net_name
                                        })
                                        .color(tokens.text)
                                        .strong(),
                                    );
                                });

                                ui.horizontal(|ui| {
                                    ui.label(RichText::new(crate::i18n::tr(lang, "context_menu.side")).color(tokens.text_dim));
                                    ui.label(
                                        RichText::new(if pin.side.is_top() { "TOP" } else { "BOTTOM" })
                                            .color(tokens.text),
                                    );
                                });

                                ui.add_space(6.0);
                                ui.separator();

                                if !pin.net_name.is_empty() {
                                    if ui
                                        .button(RichText::new(crate::i18n::tr(lang, "context_menu.copy_net")).color(tokens.text))
                                        .clicked()
                                    {
                                        ui.ctx().copy_text(pin.net_name.clone());
                                        close_menu = true;
                                    }
                                }

                                let pin_info = format!(
                                    "{}.{} ({})",
                                    part_name,
                                    pin.name,
                                    if pin.net_name.is_empty() {
                                        "UNCONNECTED"
                                    } else {
                                        &pin.net_name
                                    }
                                );
                                if ui
                                    .button(RichText::new(crate::i18n::tr(lang, "context_menu.copy_pin_info")).color(tokens.text))
                                    .clicked()
                                {
                                    ui.ctx().copy_text(pin_info);
                                    close_menu = true;
                                }

                                if !pin.net_name.is_empty() {
                                    let schematic_net = format!("{} ({})", crate::i18n::tr(lang, "context_menu.show_in_schematic"), &pin.net_name);
                                    if ui
                                        .button(RichText::new(schematic_net).color(tokens.accent).strong())
                                        .clicked()
                                    {
                                        canvas_state.navigate_to_schematic = Some(pin.net_name.clone());
                                        close_menu = true;
                                    }
                                }

                                if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                                    close_menu = true;
                                }
                            });
                    });

                if close_menu {
                    canvas_state.context_menu_pin = None;
                }
            }
        }

        // Render Component Context Menu Popup if open
        if let Some((part_idx, menu_pos)) = canvas_state.context_menu_part {
            if let Some(part) = b.parts.get(part_idx) {
                let lang = prefs.ui_language();
                let mut close_menu = false;
                let menu_id = egui::Id::unique(("part_context_menu_area", vp.side));

                egui::Area::new(menu_id)
                    .fixed_pos(menu_pos)
                    .order(egui::Order::Foreground)
                    .show(ui.ctx(), |ui| {
                        egui::Frame::menu(ui.style())
                            .fill(tokens.chrome)
                            .stroke(Stroke::new(1.0, tokens.border))
                            .corner_radius(4.0)
                            .inner_margin(8.0)
                            .show(ui, |ui| {
                                ui.set_min_width(180.0);
                                ui.heading(
                                    RichText::new(&part.name)
                                        .color(tokens.accent)
                                        .strong(),
                                );
                                ui.separator();

                                ui.horizontal(|ui| {
                                    ui.label(RichText::new(crate::i18n::tr(lang, "context_menu.side")).color(tokens.text_dim));
                                    let side_str = match part.side {
                                        Side::Top => "TOP",
                                        Side::Bottom => "BOTTOM",
                                        Side::Both => "BOTH",
                                    };
                                    ui.label(RichText::new(side_str).color(tokens.text));
                                });

                                ui.horizontal(|ui| {
                                    ui.label(RichText::new(crate::i18n::tr(lang, "context_menu.pins")).color(tokens.text_dim));
                                    ui.label(RichText::new(part.pin_indices.len().to_string()).color(tokens.text));
                                });

                                ui.add_space(6.0);
                                ui.separator();

                                let schematic_label = format!("{} ({})", crate::i18n::tr(lang, "context_menu.show_in_schematic"), &part.name);
                                if ui
                                    .button(RichText::new(schematic_label).color(tokens.accent).strong())
                                    .clicked()
                                {
                                    canvas_state.navigate_to_schematic = Some(part.name.clone());
                                    close_menu = true;
                                }

                                ui.separator();

                                if ui
                                    .button(RichText::new(crate::i18n::tr(lang, "context_menu.copy_part_name")).color(tokens.text))
                                    .clicked()
                                {
                                    ui.ctx().copy_text(part.name.clone());
                                    close_menu = true;
                                }

                                if ui
                                    .button(RichText::new(crate::i18n::tr(lang, "context_menu.copy_part_details")).color(tokens.text))
                                    .clicked()
                                {
                                    let tsv = crate::side_panel::export_part_details_tsv(b, part);
                                    ui.ctx().copy_text(tsv);
                                    close_menu = true;
                                }

                                if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                                    close_menu = true;
                                }
                            });
                    });

                if close_menu {
                    canvas_state.context_menu_part = None;
                }
            }
        }
    }

    // Render Calibration Overlays (Markers, Snap Rings, Residuals, HUD Banner)
    if is_calibrating {
        render_calibration_overlays(
            &painter,
            rect,
            &canvas_state.viewport,
            active_slot,
            alignment_session,
            board,
            response.hover_pos(),
            prefs.ui_language(),
        );
    }

    // Render 2D Measurement Caliper Overlay
    if canvas_state.measurement.active || canvas_state.measurement.has_measurement() {
        render_measurement_overlay(
            &painter,
            rect,
            &canvas_state.viewport,
            &canvas_state.measurement,
            board,
            current_hover_pin,
            current_hover_via,
            response.hover_pos(),
            tokens,
        );
    }

    response
}

/// Renders interactive calibration markers, snap rings, error residual vectors, and top HUD banner.
pub fn render_calibration_overlays(
    painter: &egui::Painter,
    canvas_rect: EguiRect,
    vp: &Viewport,
    active_slot: &crate::overlay::OverlaySlot,
    session: &AlignmentSession,
    board: Option<&Board>,
    hover_pos: Option<Pos2>,
    lang: crate::i18n::Language,
) {
    let cyan = Color32::from_rgb(0x00, 0xe5, 0xff); // #00E5FF
    let magenta = Color32::from_rgb(0xff, 0x40, 0x81); // #FF4081
    let text_color = Color32::WHITE;

    // 1. Live snap ring on hovered CAD pin during SelectingCadPin
    if let AlignmentState::SelectingCadPin { .. } = session.state {
        if let (Some(b), Some(h_pos)) = (board, hover_pos) {
            let world_pos = vp.screen_to_world(h_pos, canvas_rect);
            let snap_dist = (25.0 / vp.scale) as f64;
            if let Some(pin_idx) = b.find_pin_near(world_pos, vp.side, snap_dist) {
                if let Some(pin) = b.pins.get(pin_idx) {
                    let pin_scr = vp.world_to_screen(pin.pos, canvas_rect);
                    // Pulsing/animated ring or distinct highlight ring
                    painter.circle_stroke(pin_scr, 14.0, Stroke::new(2.0, cyan));
                    painter.circle_stroke(pin_scr, 20.0, Stroke::new(1.0, Color32::from_rgba_unmultiplied(0x00, 0xe5, 0xff, 120)));
                }
            }
        }
    }

    // 2. Render correspondence markers (CAD crosshairs and Photo diamonds)
    for (i, pair) in session.pairs.iter().enumerate() {
        let num_str = format!("{}", i + 1);

        // CAD pin marker: Cyan circle with center crosshairs and label
        let cad_scr = vp.world_to_screen(pair.cad_pos, canvas_rect);
        painter.circle_filled(cad_scr, 12.0, Color32::from_rgba_unmultiplied(0x00, 0x30, 0x40, 200));
        painter.circle_stroke(cad_scr, 12.0, Stroke::new(2.0, cyan));
        // Crosshair lines
        painter.line_segment([Pos2::new(cad_scr.x - 16.0, cad_scr.y), Pos2::new(cad_scr.x + 16.0, cad_scr.y)], Stroke::new(1.5, cyan));
        painter.line_segment([Pos2::new(cad_scr.x, cad_scr.y - 16.0), Pos2::new(cad_scr.x, cad_scr.y + 16.0)], Stroke::new(1.5, cyan));
        // Text label
        painter.text(
            Pos2::new(cad_scr.x + 16.0, cad_scr.y - 10.0),
            egui::Align2::LEFT_CENTER,
            format!("{}: {}.{}", num_str, pair.part_name, pair.pin_name),
            FontId::proportional(12.0),
            cyan,
        );

        // Photo point marker: Magenta diamond with number
        if let Some(photo_pos) = pair.photo_pos {
            let photo_scr = active_slot.photo_local_to_screen(photo_pos, canvas_rect, vp);
            let s = 10.0;
            let diamond_pts = vec![
                Pos2::new(photo_scr.x, photo_scr.y - s),
                Pos2::new(photo_scr.x + s, photo_scr.y),
                Pos2::new(photo_scr.x, photo_scr.y + s),
                Pos2::new(photo_scr.x - s, photo_scr.y),
            ];
            painter.add(egui::Shape::convex_polygon(
                diamond_pts.clone(),
                Color32::from_rgba_unmultiplied(0x50, 0x00, 0x20, 200),
                Stroke::new(2.0, magenta),
            ));
            painter.text(
                Pos2::new(photo_scr.x + 14.0, photo_scr.y + 10.0),
                egui::Align2::LEFT_CENTER,
                format!("P{}", num_str),
                FontId::proportional(11.0),
                magenta,
            );
        }
    }

    // 3. Error residual vectors if alignment result is computed
    if let Some(Ok(ref result)) = session.live_result {
        for (i, pair) in session.pairs.iter().enumerate() {
            if let Some(res_mils) = result.residuals_mils.get(i) {
                if let Some(photo_pos) = pair.photo_pos {
                    let cad_scr = vp.world_to_screen(pair.cad_pos, canvas_rect);
                    // Color threshold:
                    // < 2.0 mils -> Green
                    // 2.0 ..= 5.0 mils -> Yellow
                    // > 5.0 mils -> Red
                    let vec_color = if *res_mils < 2.0 {
                        Color32::from_rgb(0x4c, 0xaf, 0x50) // Green
                    } else if *res_mils <= 5.0 {
                        Color32::from_rgb(0xff, 0xeb, 0x3b) // Yellow
                    } else {
                        Color32::from_rgb(0xf4, 0x43, 0x36) // Red
                    };

                    // Photo screen pos
                    let photo_scr = active_slot.photo_local_to_screen(photo_pos, canvas_rect, vp);
                    painter.line_segment([cad_scr, photo_scr], Stroke::new(2.0, vec_color));
                    let mid = Pos2::new((cad_scr.x + photo_scr.x) * 0.5, (cad_scr.y + photo_scr.y) * 0.5);
                    painter.text(
                        mid,
                        egui::Align2::CENTER_CENTER,
                        format!("{:.1} mil", res_mils),
                        FontId::proportional(11.0),
                        vec_color,
                    );
                }
            }
        }
    }

    // 4. Floating Guidance HUD Banner at top center of canvas
    let banner_text = match &session.state {
        AlignmentState::SelectingCadPin { pair_index } => {
            let step = pair_index + 1;
            crate::i18n::tr(lang, "overlay.align_banner_cad").replace("{}", &format!("{step}"))
        }
        AlignmentState::SelectingPhotoPoint { pair_index, pin_name, part_name, .. } => {
            let step = pair_index + 1;
            let base = crate::i18n::tr(lang, "overlay.align_banner_photo").replace("{}", &format!("{step}"));
            format!("{base} ({part_name}.{pin_name})")
        }
        AlignmentState::ReadyToApply => {
            let count = session.pairs.len();
            let base = crate::i18n::tr(lang, "overlay.align_banner_ready").replace("{}", &format!("{count}"));
            if let Some(Ok(ref res)) = session.live_result {
                format!("{base} RMSE: {:.2} mils", res.rmse_mils)
            } else {
                base
            }
        }
        AlignmentState::Idle => String::new(),
    };

    if !banner_text.is_empty() {
        let banner_w = 460.0;
        let banner_h = 32.0;
        let banner_rect = EguiRect::from_center_size(
            Pos2::new(canvas_rect.center().x, canvas_rect.min.y + 36.0),
            egui::vec2(banner_w, banner_h),
        );

        painter.rect(
            banner_rect,
            6.0,
            Color32::from_rgba_unmultiplied(0x1a, 0x22, 0x30, 230),
            Stroke::new(1.5, cyan),
            egui::StrokeKind::Outside,
        );
        painter.text(
            banner_rect.center(),
            egui::Align2::CENTER_CENTER,
            banner_text,
            FontId::proportional(13.0),
            text_color,
        );
    }
}

/// Helper function to stroke a dashed line between two points in screen space.
fn stroke_dashed_line(
    painter: &egui::Painter,
    p1: Pos2,
    p2: Pos2,
    stroke: Stroke,
    dash_len: f32,
    gap_len: f32,
) {
    let dir = p2 - p1;
    let dist = dir.length();
    if dist < 1e-3 {
        return;
    }
    let unit = dir / dist;
    let period = dash_len + gap_len;
    let mut traveled = 0.0;

    while traveled < dist {
        let seg_start = p1 + unit * traveled;
        let seg_len = dash_len.min(dist - traveled);
        let seg_end = seg_start + unit * seg_len;
        painter.line_segment([seg_start, seg_end], stroke);
        traveled += period;
    }
}

/// Renders 2D caliper measurement line, snap circles and floating measurement badge.
pub fn render_measurement_overlay(
    painter: &egui::Painter,
    canvas_rect: EguiRect,
    vp: &Viewport,
    measurement: &MeasurementTool,
    board: Option<&Board>,
    hover_pin: Option<usize>,
    hover_via: Option<usize>,
    hover_pos: Option<Pos2>,
    tokens: &Tokens,
) {
    let amber = tokens.accent;
    let cyan = Color32::from_rgb(0x00, 0xe5, 0xff);

    // Live snap candidate if hovering pin/via during measurement
    if measurement.active && measurement.end.is_none() {
        if let Some(pin_idx) = hover_pin {
            if let Some(pin) = board.and_then(|b| b.pins.get(pin_idx)) {
                let pin_scr = vp.world_to_screen(pin.pos, canvas_rect);
                painter.circle_stroke(pin_scr, 9.0, Stroke::new(1.5, cyan));
            }
        } else if let Some(via_idx) = hover_via {
            if let Some(via) = board.and_then(|b| b.vias.get(via_idx)) {
                let via_scr = vp.world_to_screen(via.pos, canvas_rect);
                painter.circle_stroke(via_scr, 9.0, Stroke::new(1.5, cyan));
            }
        }
    }

    let start_world = measurement.start;
    if start_world.is_none() {
        return;
    }
    let p1_world = start_world.unwrap();
    let p1_scr = vp.world_to_screen(p1_world, canvas_rect);

    // Draw start marker
    painter.circle_filled(p1_scr, 4.0, amber);
    painter.circle_stroke(p1_scr, 8.0, Stroke::new(1.5, amber));

    // Determine second point (either fixed end or live cursor/snapped position)
    let p2_world = measurement.end.or_else(|| {
        if !measurement.active {
            return None;
        }
        hover_pin
            .and_then(|idx| board.and_then(|b| b.pins.get(idx).map(|p| p.pos)))
            .or_else(|| hover_via.and_then(|idx| board.and_then(|b| b.vias.get(idx).map(|v| v.pos))))
            .or_else(|| hover_pos.map(|h| vp.screen_to_world(h, canvas_rect)))
    });

    if let Some(p2_world) = p2_world {
        let p2_scr = vp.world_to_screen(p2_world, canvas_rect);

        // Draw dashed measurement line
        stroke_dashed_line(painter, p1_scr, p2_scr, Stroke::new(1.5, amber), 6.0, 4.0);

        // Draw end marker
        painter.circle_filled(p2_scr, 4.0, amber);
        painter.circle_stroke(p2_scr, 8.0, Stroke::new(1.5, amber));

        // Compute distances:
        // World coordinates in bvc-core are in mils (1 mil = 0.0254 mm).
        let dx_mils = (p2_world.x - p1_world.x).abs();
        let dy_mils = (p2_world.y - p1_world.y).abs();
        let dist_mils = (dx_mils * dx_mils + dy_mils * dy_mils).sqrt();

        let dx_mm = dx_mils * 0.0254;
        let dy_mm = dy_mils * 0.0254;
        let dist_mm = dist_mils * 0.0254;

        // Render measurement badge at midpoint
        let mid_scr = Pos2::new(
            (p1_scr.x + p2_scr.x) * 0.5,
            (p1_scr.y + p2_scr.y) * 0.5 - 20.0,
        );

        let label_text = format!("{dist_mm:.3} mm ({dist_mils:.1} mils)");
        let delta_text = format!("ΔX: {dx_mm:.2} mm  ΔY: {dy_mm:.2} mm");

        let badge_w = 175.0;
        let badge_h = 36.0;
        let badge_rect = EguiRect::from_center_size(mid_scr, egui::vec2(badge_w, badge_h));

        painter.rect(
            badge_rect,
            6.0,
            Color32::from_rgba_unmultiplied(0x1a, 0x1d, 0x24, 235),
            Stroke::new(1.2, amber),
            egui::StrokeKind::Outside,
        );

        painter.text(
            Pos2::new(badge_rect.center().x, badge_rect.min.y + 10.0),
            egui::Align2::CENTER_CENTER,
            label_text,
            FontId::monospace(11.5),
            Color32::WHITE,
        );

        painter.text(
            Pos2::new(badge_rect.center().x, badge_rect.min.y + 24.0),
            egui::Align2::CENTER_CENTER,
            delta_text,
            FontId::monospace(9.5),
            amber,
        );
    }
}

/// Generates render meshes for all visible board entities using spatial frustum queries,
/// precomputed net membership, and subpixel LOD tessellation.
/// Below this projected size (pixels) a part body becomes a small display-only marker.
const TINY_PART_PX: f64 = 3.0;
/// Minimum on-screen size (pixels) of a tiny-part marker.
const TINY_PART_MARKER_PX: f32 = 2.6;
/// Minimum on-screen pad radius / half-size (pixels) for ordinary and net-highlighted pads.
const PAD_MARKER_PX: f32 = 1.0;
const PAD_MARKER_NET_PX: f32 = 2.2;
/// Zoom applied by one mouse-wheel notch (one scrolled line).
pub const WHEEL_NOTCH_ZOOM: f32 = 2.5;

/// Zoom factor for this frame's smoothed wheel delta (`scroll_y` points, positive zooms in;
/// `line_points` is egui's points per scrolled line).
///
/// egui spreads one wheel notch over several frames. The factor is proportional to the delta,
/// so a notch zooms by `WHEEL_NOTCH_ZOOM` whatever the frame rate; a fixed step per frame with a
/// non-zero delta zoomed less per notch when frames were slow (and far too fast on trackpads).
pub fn wheel_zoom_factor(scroll_y: f32, line_points: f32) -> f32 {
    WHEEL_NOTCH_ZOOM.powf(scroll_y / line_points.max(1.0))
}

/// Step (points) between the font sizes canvas labels may use.
pub const LABEL_FONT_STEP: f32 = 2.0;

/// Font size (points) of a zoom-dependent canvas label: clamped to `min..=max` and snapped to a
/// multiple of `LABEL_FONT_STEP`.
///
/// egui caches laid-out text and rasterised glyphs per exact font size. A size that follows the
/// zoom continuously misses both caches on every zoom frame (re-layout of every visible label
/// plus new glyphs in the font atlas), which made wheel zoom cost 7-20 ms per frame in labels
/// alone. A few fixed sizes keep labels and glyphs cached between zoom steps.
pub fn label_font_size(raw: f32, min: f32, max: f32) -> f32 {
    ((raw.clamp(min, max) / LABEL_FONT_STEP).round() * LABEL_FONT_STEP).clamp(min, max)
}

/// Below this on-screen half-size (pixels) a pad is drawn as an equal-area square point.
const PAD_POINT_PX: f32 = 1.5;

/// Appends one part body: its outline (or bounding box) filled and stroked, or a marker when tiny.
///
/// Portions adapted from TRACE Boardviewer (MIT), Copyright (c) 2026 TRACE Boardviewer contributors
/// (`drawBodies` in `src/components/board-render-2d.ts`).
#[allow(clippy::too_many_arguments)]
fn append_part_body(
    mesh: &mut Mesh,
    part: &bvc_core::Part,
    bbox: Rect,
    is_selected: bool,
    scale: f32,
    axis_x: egui::Vec2,
    axis_y: egui::Vec2,
    to_screen: &dyn Fn(Point) -> Pos2,
    tokens: &Tokens,
) {
    let (line_color, line_px) = if is_selected { (tokens.selection, 2.0) } else { (tokens.part_body_line, 0.95) };
    if bbox.width().max(bbox.height()) * (scale as f64) < TINY_PART_PX {
        // Display-only footprint marker keeps tiny parts visible without changing geometry.
        let at_least = |v: egui::Vec2, px: f32| {
            let len = v.length();
            if len < px && len > 0.0 { v * (px / len) } else { v }
        };
        let half_u = at_least(axis_x * (bbox.width() * 0.5) as f32, TINY_PART_MARKER_PX * 0.5);
        let half_v = at_least(axis_y * (bbox.height() * 0.5) as f32, TINY_PART_MARKER_PX * 0.5);
        let fill = if is_selected { tokens.selection } else { tokens.part_body };
        Renderer::append_rotated_rect_to_mesh(mesh, to_screen(bbox.center()), half_u, half_v, fill);
        return;
    }
    let mut outline: &[Point] = &part.outline;
    if outline.len() > 3 && outline[0] == outline[outline.len() - 1] {
        outline = &outline[..outline.len() - 1];
    }
    let points: Vec<Pos2> = if outline.len() >= 3 {
        outline.iter().map(|&p| to_screen(p)).collect()
    } else {
        [
            Point::new(bbox.min_x, bbox.min_y),
            Point::new(bbox.max_x, bbox.min_y),
            Point::new(bbox.max_x, bbox.max_y),
            Point::new(bbox.min_x, bbox.max_y),
        ]
        .into_iter()
        .map(to_screen)
        .collect()
    };
    Renderer::append_polygon_fill_to_mesh(mesh, &points, tokens.part_body);
    Renderer::append_closed_polyline_to_mesh(mesh, &points, line_px * 0.5, line_color);
}

/// On-screen shape of a pad with a known physical size.
///
/// Portions adapted from TRACE Boardviewer (MIT), Copyright (c) 2026 TRACE Boardviewer contributors
/// (`appendPadPath` in `src/lib/render-geometry.ts`).
#[derive(Debug, Clone, Copy, PartialEq)]
enum ScreenPad {
    /// Round pad of this screen radius.
    Circle(f32),
    /// Rotated rectangle given by its screen half-extent vectors.
    Rect(egui::Vec2, egui::Vec2),
    /// Equal-area axis-aligned square point of this half-size (pad too small to show its shape).
    Point(f32),
}

impl ScreenPad {
    /// Screen shape of `pin`'s pad, or `None` when the pad has no physical size (caller keeps the
    /// legacy radius-based circle). `marker_px` is the minimum radius in pixels for round pads.
    fn of(pin: &bvc_core::Pin, scale: f32, axis_x: egui::Vec2, axis_y: egui::Vec2, marker_px: f32) -> Option<Self> {
        let (w, h) = pin.pad_size()?;
        let scale_f64 = scale as f64;
        if pin.shape == bvc_core::model::PadShape::Round {
            let radius = (w.min(h) * 0.5 * scale_f64) as f32;
            let radius = radius.max(marker_px);
            return Some(if radius < PAD_POINT_PX {
                ScreenPad::Point(radius * std::f32::consts::PI.sqrt() * 0.5)
            } else {
                ScreenPad::Circle(radius)
            });
        }
        let min_world = 1.5 / scale_f64;
        let half_w = w.max(min_world) * 0.5;
        let half_h = h.max(min_world) * 0.5;
        if (half_w.max(half_h) * scale_f64) < PAD_POINT_PX as f64 {
            return Some(ScreenPad::Point(((half_w * half_h).sqrt() * scale_f64) as f32));
        }
        let (cos, sin) = bvc_core::model::quadrant(pin.rotation_deg);
        let u = axis_x * (cos * half_w) as f32 + axis_y * (sin * half_w) as f32;
        let v = axis_x * (-sin * half_h) as f32 + axis_y * (cos * half_h) as f32;
        Some(ScreenPad::Rect(u, v))
    }

    fn fill(self, mesh: &mut Mesh, center: Pos2, color: Color32) {
        match self {
            ScreenPad::Circle(r) => Renderer::append_circle_to_mesh(mesh, center, r, color),
            ScreenPad::Rect(u, v) => Renderer::append_rotated_rect_to_mesh(mesh, center, u, v, color),
            ScreenPad::Point(h) => Renderer::append_rect_filled_to_mesh(
                mesh,
                Pos2::new(center.x - h, center.y - h),
                Pos2::new(center.x + h, center.y + h),
                color,
            ),
        }
    }

    fn outline(self, mesh: &mut Mesh, center: Pos2, stroke_w: f32, color: Color32) {
        match self {
            ScreenPad::Circle(r) => Renderer::append_circle_outline_to_mesh(mesh, center, r, stroke_w, color),
            ScreenPad::Rect(u, v) => {
                Renderer::append_rotated_rect_outline_to_mesh(mesh, center, u, v, stroke_w * 0.5, color)
            }
            ScreenPad::Point(h) => {
                Renderer::append_circle_outline_to_mesh(mesh, center, h.max(1.5), stroke_w, color)
            }
        }
    }
}

/// Highlight of the hovered pin or via, drawn every frame on top of the cached board meshes
/// so that hovering never invalidates the board mesh cache. The selected pin is skipped: its
/// ring is drawn by the painter pass.
pub fn hover_overlay_mesh(
    b: &Board,
    vp: &Viewport,
    rect: EguiRect,
    tokens: &Tokens,
    hovered_pin: Option<usize>,
    hovered_via: Option<usize>,
    selected_pin: Option<usize>,
) -> Mesh {
    let mut mesh = Mesh::default();
    let screen_origin = vp.world_to_screen(vp.center, rect);
    let axis_x = vp.world_to_screen(Point::new(vp.center.x + 1.0, vp.center.y), rect) - screen_origin;
    let axis_y = vp.world_to_screen(Point::new(vp.center.x, vp.center.y + 1.0), rect) - screen_origin;
    if let Some(pin) = hovered_pin.filter(|&idx| Some(idx) != selected_pin).and_then(|idx| b.pins.get(idx)) {
        let screen_pos = vp.world_to_screen(pin.pos, rect);
        if let Some(pad) = ScreenPad::of(pin, vp.scale, axis_x, axis_y, PAD_MARKER_NET_PX) {
            pad.fill(&mut mesh, screen_pos, tokens.hover);
        } else {
            let screen_r = (pin.radius as f32 * vp.scale).clamp(2.0, 40.0);
            Renderer::append_circle_to_mesh(&mut mesh, screen_pos, screen_r, tokens.hover);
        }
    }
    if let Some(via) = hovered_via.and_then(|idx| b.vias.get(idx)) {
        let screen_pos = vp.world_to_screen(via.pos, rect);
        let screen_r = (via.radius as f32 * vp.scale).clamp(2.0, 30.0);
        Renderer::append_circle_to_mesh(&mut mesh, screen_pos, screen_r, tokens.hover);
    }
    mesh
}

/// Board meshes for the culled view (`view_cull_rect`). See `generate_board_meshes_in`.
#[cfg(test)]
#[allow(clippy::too_many_arguments)]
pub fn generate_board_meshes(
    b: &Board,
    vp: &Viewport,
    rect: EguiRect,
    tokens: &Tokens,
    selected_pin_idx: Option<usize>,
    selected_via_idx: Option<usize>,
    selected_part_idx: Option<usize>,
    selected_net: Option<&str>,
    current_hover_pin: Option<usize>,
    current_hover_via: Option<usize>,
    renderer: &mut Renderer,
    show_net_web: bool,
    via_display_mode: ViaDisplayMode,
    show_opposite_elements: bool,
    show_vias: bool,
    show_unnamed_vias: bool,
    show_pins: bool,
) -> Vec<Mesh> {
    generate_board_meshes_in(
        b,
        vp,
        rect,
        view_cull_rect(vp, rect),
        tokens,
        selected_pin_idx,
        selected_via_idx,
        selected_part_idx,
        selected_net,
        current_hover_pin,
        current_hover_via,
        renderer,
        show_net_web,
        via_display_mode,
        show_opposite_elements,
        show_vias,
        show_unnamed_vias,
        show_pins,
    )
}

/// Screen-space board meshes for `vp`, holding only parts, pads and vias inside the world
/// region `cull_world` (offscreen geometry is never tessellated).
#[allow(clippy::too_many_arguments)]
pub fn generate_board_meshes_in(
    b: &Board,
    vp: &Viewport,
    rect: EguiRect,
    cull_world: Rect,
    tokens: &Tokens,
    selected_pin_idx: Option<usize>,
    selected_via_idx: Option<usize>,
    selected_part_idx: Option<usize>,
    selected_net: Option<&str>,
    current_hover_pin: Option<usize>,
    current_hover_via: Option<usize>,
    renderer: &mut Renderer,
    show_net_web: bool,
    via_display_mode: ViaDisplayMode,
    show_opposite_elements: bool,
    show_vias: bool,
    show_unnamed_vias: bool,
    show_pins: bool,
) -> Vec<Mesh> {
    let mut new_meshes = Vec::new();
    let visible_world = cull_world;

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

    // Linear part of the world-to-screen mapping (screen pixels per world unit along world X and Y).
    // Lets pads and bodies be placed with one multiply-add per corner instead of a full transform.
    let screen_origin = vp.world_to_screen(vp.center, rect);
    let axis_x = vp.world_to_screen(Point::new(vp.center.x + 1.0, vp.center.y), rect) - screen_origin;
    let axis_y = vp.world_to_screen(Point::new(vp.center.x, vp.center.y + 1.0), rect) - screen_origin;
    let to_screen = |p: Point| -> Pos2 {
        screen_origin + axis_x * (p.x - vp.center.x) as f32 + axis_y * (p.y - vp.center.y) as f32
    };

    // 2. Part bodies: filled outline plus stroke, drawn below vias and pads (TRACE Boardviewer style).
    let bodies_mesh: Mesh = b
        .parts
        .par_iter()
        .filter(|part| {
            part.side.visible_on(vp.side)
                && part.bounding_box.is_some_and(|bbox| bbox.intersects(&visible_world))
        })
        .fold(
            Mesh::default,
            |mut mesh, part| {
                if let Some(bbox) = part.bounding_box {
                    let is_part_selected = selected_part_idx == Some(part.id);
                    append_part_body(&mut mesh, part, bbox, is_part_selected, vp.scale, axis_x, axis_y, &to_screen, tokens);
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

    if !bodies_mesh.is_empty() {
        new_meshes.push(bodies_mesh);
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
            let is_net_selected = active_membership.contains_via(*idx);
            let is_hovered = current_hover_via == Some(*idx);
            let is_active = is_net_selected || is_hovered;

            // Global via visibility check (unless selected/hovered)
            if !show_vias && !is_active {
                return false;
            }

            // Unnamed via/pad filtering (empty net or "UNCONNECTED")
            let is_unnamed = via.net_name.is_empty() || via.net_name == "UNCONNECTED";
            if !show_unnamed_vias && is_unnamed && !is_active {
                return false;
            }

            let is_on_cur_side = via.side.visible_on(vp.side);
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
            let is_hovered = current_hover_pin == Some(*_idx);
            let is_selected_pin = selected_pin_idx == Some(*_idx);
            let is_active = is_net_selected || is_hovered || is_selected_pin;

            // Global pin visibility check (unless selected/hovered/net-active)
            if !show_pins && !is_active {
                return false;
            }

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
                    // Selected pin outline is rendered exclusively by the painter pass with the outer ring + label,
                    // avoiding double concentric circles.
                } else if is_hovered {
                    if let Some(pad) = ScreenPad::of(pin, vp.scale, axis_x, axis_y, PAD_MARKER_NET_PX) {
                        pad.fill(&mut hl_mesh, screen_pos, tokens.hover);
                    } else {
                        let screen_r = raw_screen_r.clamp(2.0, 40.0);
                        Renderer::append_circle_to_mesh(&mut hl_mesh, screen_pos, screen_r, tokens.hover);
                    }
                } else if is_net_selected {
                    let pad = ScreenPad::of(pin, vp.scale, axis_x, axis_y, PAD_MARKER_NET_PX);
                    if is_on_cur_side {
                        // Current side: filled highlight pad
                        if let Some(pad) = pad {
                            pad.fill(&mut hl_mesh, screen_pos, tokens.pin_highlight_cur_side);
                        } else {
                            Renderer::append_lod_circle_to_mesh(
                                &mut hl_mesh,
                                screen_pos,
                                raw_screen_r,
                                is_high_density,
                                tokens.pin_highlight_cur_side,
                            );
                        }
                    } else {
                        // Opposite side: hollow outline / border ONLY!
                        let stroke_w = if is_high_density { 1.2 } else { 1.8 };
                        if let Some(pad) = pad {
                            pad.outline(&mut hl_mesh, screen_pos, stroke_w, tokens.pin_highlight_opp_side);
                        } else {
                            Renderer::append_circle_outline_to_mesh(
                                &mut hl_mesh,
                                screen_pos,
                                raw_screen_r.clamp(1.5, 40.0),
                                stroke_w,
                                tokens.pin_highlight_opp_side,
                            );
                        }
                    }
                } else {
                    let color = if !is_on_cur_side {
                        Color32::from_rgba_premultiplied(0x35, 0x40, 0x50, 0x40)
                    } else if pin.net_name.is_empty() || pin.net_name == "UNCONNECTED" {
                        tokens.pin_unconnected
                    } else {
                        tokens.pin_default
                    };

                    if let Some(pad) = ScreenPad::of(pin, vp.scale, axis_x, axis_y, PAD_MARKER_PX) {
                        pad.fill(&mut normal_mesh, screen_pos, color);
                    } else {
                        Renderer::append_lod_circle_to_mesh(
                            &mut normal_mesh,
                            screen_pos,
                            raw_screen_r,
                            false,
                            color,
                        );
                    }
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
    // - If nothing is selected (neither pin nor via), do not render any flight lines.
    // - If an element (pin or via) is selected:
    //     * Radial: Lines radiating from the selected point to every other pin and via on the net.
    //     * Web (Telaraña): Minimum Spanning Tree (MST) route connecting all points on the net.
    if show_net_web {
        let selected_point: Option<Point> = if let Some(pidx) = selected_pin_idx {
            b.pins.get(pidx).map(|p| p.pos)
        } else if let Some(vidx) = selected_via_idx {
            b.vias.get(vidx).map(|v| v.pos)
        } else {
            None
        };

        if let (Some(origin), Some(net_name)) = (selected_point, selected_net) {
            let is_ground_or_power = net_name.eq_ignore_ascii_case("GND")
                || net_name.eq_ignore_ascii_case("GROUND")
                || net_name.starts_with("UNCONNECTED")
                || net_name.is_empty();

            if !is_ground_or_power {
                if let Some(net) = b.nets.get(net_name) {
                    let mut nodes: Vec<Point> = Vec::with_capacity(net.pin_indices.len() + net.via_indices.len());
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

                    let total_nodes = nodes.len();
                    if total_nodes >= 2 && total_nodes <= 400 {
                        let mut segments = Vec::new();

                        match via_display_mode {
                            ViaDisplayMode::Radial => {
                                // Radial: lines from the selected point to all other nodes on the net
                                let p_origin = vp.world_to_screen(origin, rect);
                                for target in &nodes {
                                    if target.dist(origin) > 1e-4 {
                                        let p_target = vp.world_to_screen(*target, rect);
                                        segments.push((p_origin, p_target));
                                    }
                                }
                            }
                            ViaDisplayMode::Web => {
                                // Web (Telaraña): Euclidean MST linking all points along optimal path
                                let n = nodes.len();
                                let mut in_mst = vec![false; n];
                                let mut min_dist = vec![f64::INFINITY; n];
                                let mut parent = vec![0usize; n];

                                in_mst[0] = true;
                                for v in 1..n {
                                    min_dist[v] = nodes[0].dist(nodes[v]);
                                    parent[v] = 0;
                                }

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

    fn key_input(keys: &[(egui::Key, egui::Modifiers)]) -> egui::InputState {
        let events = keys
            .iter()
            .map(|&(key, modifiers)| egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers })
            .collect();
        let raw = egui::RawInput { events, ..egui::RawInput::default() };
        egui::InputState::default().begin_pass(raw, false, 1.0, egui::InputOptions::default())
    }

    #[test]
    fn keyboard_pan_zoom_follows_default_keymap() {
        use egui::{Key, Modifiers};
        let km = Keymap::default();
        let none = Modifiers::NONE;
        assert_eq!(keyboard_pan_zoom(&km, &key_input(&[(Key::ArrowUp, none)])), (egui::vec2(0.0, -1.0), false, false));
        assert_eq!(
            keyboard_pan_zoom(&km, &key_input(&[(Key::ArrowRight, none), (Key::ArrowDown, none)])),
            (egui::vec2(1.0, 1.0), false, false)
        );
        assert_eq!(keyboard_pan_zoom(&km, &key_input(&[(Key::ArrowLeft, none)])).0, egui::vec2(-1.0, 0.0));
        assert_eq!(keyboard_pan_zoom(&km, &key_input(&[(Key::Equals, none)])), (egui::Vec2::ZERO, true, false));
        assert_eq!(keyboard_pan_zoom(&km, &key_input(&[(Key::Plus, Modifiers::SHIFT)])), (egui::Vec2::ZERO, true, false));
        assert_eq!(keyboard_pan_zoom(&km, &key_input(&[(Key::Minus, none)])), (egui::Vec2::ZERO, false, true));
    }

    #[test]
    fn keyboard_pan_zoom_uses_rebound_keys() {
        use crate::keymap::{Binding, Mods};
        use egui::{Key, Modifiers};
        let mut km = Keymap::default();
        km.set(Action::PanUp, Binding::new(Key::W, Mods::NONE));
        km.set(Action::ZoomOut, Binding::new(Key::Z, Mods::SHIFT));
        let none = Modifiers::NONE;
        assert_eq!(keyboard_pan_zoom(&km, &key_input(&[(Key::W, none)])).0, egui::vec2(0.0, -1.0));
        assert_eq!(keyboard_pan_zoom(&km, &key_input(&[(Key::ArrowUp, none)])).0, egui::Vec2::ZERO);
        assert_eq!(keyboard_pan_zoom(&km, &key_input(&[(Key::Z, Modifiers::SHIFT)])), (egui::Vec2::ZERO, false, true));
        assert_eq!(keyboard_pan_zoom(&km, &key_input(&[(Key::Minus, none)])), (egui::Vec2::ZERO, false, false));
    }

    #[test]
    fn test_measurement_tool_lifecycle() {
        let mut tool = MeasurementTool::default();
        assert!(!tool.active);
        assert!(!tool.has_measurement());

        tool.toggle();
        assert!(tool.active);
        assert!(!tool.has_measurement());

        tool.start = Some(Point::new(100.0, 100.0));
        assert!(tool.is_measuring());
        assert!(!tool.has_measurement());

        tool.end = Some(Point::new(200.0, 100.0));
        assert!(!tool.is_measuring());
        assert!(tool.has_measurement());

        tool.reset();
        assert!(!tool.has_measurement());
        assert!(tool.active); // Tool remains active for the next measurement until toggled off

        tool.toggle();
        assert!(!tool.active);
    }

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
            None,
            Some("IFPC_TXC*"),
            None,
            None,
            &mut renderer,
            true, // show_net_web
            ViaDisplayMode::Web,
            false, // show_opposite_elements
            true,  // show_vias
            false, // show_unnamed_vias
            true,  // show_pins
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
                ..Default::default()
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
            Some(0),
            None,
            None,
            Some("GND"),
            None,
            None,
            &mut renderer,
            true,
            ViaDisplayMode::Web,
            false,
            true,
            false,
            true,
        );

        // Benchmark pass
        let start = Instant::now();
        let meshes = generate_board_meshes(
            &board,
            &vp,
            canvas_rect,
            &tokens,
            Some(0),
            None,
            None,
            Some("GND"),
            None,
            None,
            &mut renderer,
            true,
            ViaDisplayMode::Web,
            false,
            true,
            false,
            true,
        );
        let elapsed = start.elapsed();

        assert!(!meshes.is_empty(), "Meshes must be produced for GND");
        println!("Dense net tessellation (4,755 GND nodes) took {:?}", elapsed);
        assert!(elapsed.as_millis() < 80, "Tessellation took {:?}, expected < 80ms in debug build", elapsed);
    }

    fn axes(vp: &Viewport, rect: EguiRect) -> (egui::Vec2, egui::Vec2) {
        let o = vp.world_to_screen(vp.center, rect);
        (
            vp.world_to_screen(Point::new(vp.center.x + 1.0, vp.center.y), rect) - o,
            vp.world_to_screen(Point::new(vp.center.x, vp.center.y + 1.0), rect) - o,
        )
    }

    fn rect_pad(w: f64, h: f64, rot: f64) -> Pin {
        Pin {
            shape: bvc_core::model::PadShape::Rect,
            width: Some(w),
            height: Some(h),
            rotation_deg: rot,
            radius: w.min(h) / 2.0,
            ..Default::default()
        }
    }

    #[test]
    fn screen_pad_follows_pad_rotation_and_view() {
        let rect = EguiRect::from_min_size(Pos2::ZERO, egui::vec2(800.0, 600.0));
        let mut vp = Viewport::default();
        vp.scale = 1.0;
        let (ax, ay) = axes(&vp, rect);
        // 10 x 40 pad: tall on screen unrotated, wide when rotated 90 degrees.
        let Some(ScreenPad::Rect(u, v)) = ScreenPad::of(&rect_pad(10.0, 40.0, 0.0), vp.scale, ax, ay, PAD_MARKER_PX) else {
            panic!("expected a rect")
        };
        assert!((u.length() - 5.0).abs() < 1e-4 && (v.length() - 20.0).abs() < 1e-4);
        assert!(v.y.abs() > v.x.abs());
        let Some(ScreenPad::Rect(_, v)) = ScreenPad::of(&rect_pad(10.0, 40.0, 90.0), vp.scale, ax, ay, PAD_MARKER_PX) else {
            panic!("expected a rect")
        };
        assert!(v.x.abs() > v.y.abs());
        // View rotation turns it back to tall.
        vp.rotation = vp.rotation.rotate_cw();
        let (ax, ay) = axes(&vp, rect);
        let Some(ScreenPad::Rect(_, v)) = ScreenPad::of(&rect_pad(10.0, 40.0, 90.0), vp.scale, ax, ay, PAD_MARKER_PX) else {
            panic!("expected a rect")
        };
        assert!(v.y.abs() > v.x.abs());
    }

    #[test]
    fn screen_pad_small_sizes_and_sizeless_pins() {
        let rect = EguiRect::from_min_size(Pos2::ZERO, egui::vec2(800.0, 600.0));
        let mut vp = Viewport::default();
        vp.scale = 0.01;
        let (ax, ay) = axes(&vp, rect);
        assert!(matches!(ScreenPad::of(&rect_pad(10.0, 40.0, 0.0), vp.scale, ax, ay, PAD_MARKER_PX), Some(ScreenPad::Point(_))));
        // Sizeless pins keep the legacy circle path.
        assert_eq!(ScreenPad::of(&Pin { radius: 5.0, ..Default::default() }, vp.scale, ax, ay, PAD_MARKER_PX), None);
        // A round pad never shrinks below the marker radius.
        let round = Pin { width: Some(2.0), height: Some(2.0), radius: 1.0, ..Default::default() };
        vp.scale = 1.0;
        let (ax, ay) = axes(&vp, rect);
        assert_eq!(ScreenPad::of(&round, vp.scale, ax, ay, PAD_MARKER_NET_PX), Some(ScreenPad::Circle(PAD_MARKER_NET_PX)));
    }

    #[test]
    fn fixture_pcie_fingers_render_tall_and_bodies_are_filled() {
        let Some(board) = ms_v375_fixture() else { return };
        let rect = EguiRect::from_min_size(Pos2::ZERO, egui::vec2(1920.0, 1080.0));
        let mut vp = Viewport::default();
        vp.fit_to_rect(board.bounding_box.unwrap(), rect);
        vp.scale *= 8.0;
        let (ax, ay) = axes(&vp, rect);
        let cn1 = &board.parts[board.part_by_name["CN1"]];
        let mut tall = 0;
        for &pi in &cn1.pin_indices {
            if let Some(ScreenPad::Rect(u, v)) = ScreenPad::of(&board.pins[pi], vp.scale, ax, ay, PAD_MARKER_PX) {
                let (long, short) = if u.length() > v.length() { (u, v) } else { (v, u) };
                if long.y.abs() > long.x.abs() && long.length() > 2.0 * short.length() {
                    tall += 1;
                }
            }
        }
        assert!(tall >= 160, "{tall} CN1 fingers render as tall thin rectangles");

        // Bodies are filled: the body mesh contains the body fill colour.
        let tokens = Tokens::DARK;
        let mut renderer = Renderer::new();
        let meshes = generate_board_meshes(
            &board, &vp, rect, &tokens, None, None, None, None, None, None, &mut renderer,
            false, ViaDisplayMode::Web, false, true, false, true,
        );
        assert!(meshes.iter().any(|m| m.vertices.iter().any(|v| v.color == tokens.part_body)));
        assert!(meshes.iter().any(|m| m.vertices.iter().any(|v| v.color == tokens.pin_default)));
    }

    fn two_pin_board() -> Board {
        let mut board = Board::new();
        board.parts.push(bvc_core::Part { name: "R1".into(), ..Default::default() });
        for (i, x) in [0.0, 100.0].into_iter().enumerate() {
            board.pins.push(Pin { id: i, part_id: 0, pos: Point::new(x, 0.0), radius: 10.0, net_name: "N1".into(), ..Default::default() });
        }
        board.vias.push(Via { pos: Point::new(50.0, 50.0), radius: 8.0, net_name: "N1".into(), id: 0, side: Side::Top });
        board.rebuild_indices();
        board
    }

    /// Idle frames and hovering keep the cache key, so the board meshes are not rebuilt;
    /// view, selection and display preferences still invalidate it.
    /// Runs `sync_board_meshes` for `vp` with a real tessellation and reports what it did.
    fn sync(state: &mut CanvasState, board: &Board, vp: &Viewport, rect: EguiRect, selected_pin: Option<usize>) -> MeshSync {
        let prefs = Preferences::default();
        let key = ViewCacheKey::new(board, vp, selected_pin, None, None, None, &prefs);
        let mut renderer = Renderer::new();
        sync_board_meshes(state, key, vp, rect, |coverage| {
            generate_board_meshes_in(
                board, vp, rect, coverage, &Tokens::DARK, selected_pin, None, None, None, None, None, &mut renderer,
                true, ViaDisplayMode::Web, false, true, false, true,
            )
        })
    }

    #[test]
    fn zoom_and_pan_within_the_bucket_remap_instead_of_tessellating() {
        let board = two_pin_board();
        let rect = EguiRect::from_min_size(Pos2::ZERO, egui::vec2(800.0, 600.0));
        let mut vp = Viewport::default();
        vp.fit_to_rect(board.bounding_box.unwrap(), rect);
        let mut state = CanvasState::default();

        assert_eq!(sync(&mut state, &board, &vp, rect, None), MeshSync::Rebuilt);
        assert_eq!(sync(&mut state, &board, &vp, rect, None), MeshSync::Kept);

        // Every step of a wheel zoom (and a pan) inside the bucket only moves vertices.
        let base_scale = vp.scale;
        for step in [1.05, 1.1, 1.2, 1.3, 1.4] {
            vp.scale = base_scale * step;
            vp.center.x += 0.5;
            assert_eq!(sync(&mut state, &board, &vp, rect, None), MeshSync::Remapped, "zoom x{step}");
        }
        for step in [0.95, 0.8, 0.72] {
            vp.scale = base_scale * step;
            assert_eq!(sync(&mut state, &board, &vp, rect, None), MeshSync::Remapped, "zoom x{step}");
        }
        // The view rests for a frame: tessellated exactly once, then kept.
        assert_eq!(sync(&mut state, &board, &vp, rect, None), MeshSync::Rebuilt);
        assert_eq!(sync(&mut state, &board, &vp, rect, None), MeshSync::Kept);

        // Leaving the zoom bucket, or changing anything in the key, re-tessellates.
        vp.scale *= MESH_ZOOM_BUCKET * 1.01;
        assert_eq!(sync(&mut state, &board, &vp, rect, None), MeshSync::Rebuilt);
        assert_eq!(sync(&mut state, &board, &vp, rect, Some(0)), MeshSync::Rebuilt);
        state.cached_view_key = None;
        assert_eq!(sync(&mut state, &board, &vp, rect, Some(0)), MeshSync::Rebuilt);
    }

    #[test]
    fn panning_out_of_the_covered_region_re_tessellates() {
        let board = two_pin_board();
        let rect = EguiRect::from_min_size(Pos2::ZERO, egui::vec2(800.0, 600.0));
        let mut vp = Viewport::default();
        vp.fit_to_rect(board.bounding_box.unwrap(), rect);
        vp.scale *= 20.0;
        let mut state = CanvasState::default();
        assert_eq!(sync(&mut state, &board, &vp, rect, None), MeshSync::Rebuilt);
        let screen_w = (rect.width() / vp.scale) as f64;
        vp.center.x += screen_w * 0.4;
        assert_eq!(sync(&mut state, &board, &vp, rect, None), MeshSync::Remapped);
        vp.center.x += screen_w * 0.4;
        assert_eq!(sync(&mut state, &board, &vp, rect, None), MeshSync::Rebuilt);
    }

    #[test]
    fn remapped_meshes_match_a_fresh_tessellation_of_true_size_pads() {
        let mut board = Board::new();
        board.parts.push(bvc_core::Part { name: "U1".into(), ..Default::default() });
        for (i, (x, rot)) in [(0.0, 0.0), (60.0, 90.0), (120.0, 45.0)].into_iter().enumerate() {
            board.pins.push(Pin { id: i, pos: Point::new(x, 10.0), net_name: "N".into(), ..rect_pad(30.0, 12.0, rot) });
        }
        board.rebuild_indices();
        let rect = EguiRect::from_min_size(Pos2::new(250.0, 40.0), egui::vec2(800.0, 600.0));
        let mut vp = Viewport { rotation: Rotation::Deg90, mirror_x: true, ..Viewport::default() };
        vp.fit_to_rect(board.bounding_box.unwrap(), rect);
        let mut state = CanvasState::default();
        sync(&mut state, &board, &vp, rect, None);
        let coverage = state.mesh_base.as_ref().unwrap().coverage;

        vp.scale *= 1.3;
        vp.center = Point::new(vp.center.x + 7.0, vp.center.y - 3.0);
        assert_eq!(sync(&mut state, &board, &vp, rect, None), MeshSync::Remapped);
        let fresh = generate_board_meshes_in(
            &board, &vp, rect, coverage, &Tokens::DARK, None, None, None, None, None, None, &mut Renderer::new(),
            true, ViaDisplayMode::Web, false, true, false, true,
        );
        // Pads have true size, so the re-mapped pad mesh (the last one) equals a fresh one.
        let (remapped, fresh) = (state.cached_meshes.last().unwrap(), fresh.last().unwrap());
        assert_eq!(remapped.indices, fresh.indices);
        assert_eq!(remapped.vertices.len(), fresh.vertices.len());
        for (a, b) in remapped.vertices.iter().zip(&fresh.vertices) {
            assert!((a.pos - b.pos).length() < 1e-2, "{:?} vs {:?}", a.pos, b.pos);
            assert_eq!(a.color, b.color);
        }
    }

    #[test]
    fn offscreen_pads_are_not_tessellated() {
        let board = two_pin_board();
        let rect = EguiRect::from_min_size(Pos2::ZERO, egui::vec2(800.0, 600.0));
        let mut vp = Viewport { center: board.pins[0].pos, scale: 40.0, ..Viewport::default() };
        let pin_vertices = |vp: &Viewport| -> Vec<Pos2> {
            let meshes = generate_board_meshes_in(
                &board, vp, rect, mesh_coverage(vp, rect), &Tokens::DARK, None, None, None, None, None, None,
                &mut Renderer::new(), true, ViaDisplayMode::Web, false, false, false, true,
            );
            meshes.last().unwrap().vertices.iter().map(|v| v.pos).collect()
        };
        // Zoomed onto pin 0, pin 1 (100 mil away) is outside the covered region: only pin 0 is drawn.
        let near = pin_vertices(&vp);
        let pin0 = vp.world_to_screen(board.pins[0].pos, rect);
        assert!(!near.is_empty() && near.iter().all(|p| (*p - pin0).length() <= 10.0 * 40.0 + 1.0));
        // Zoomed out, both pins are inside the covered region.
        vp.scale = 1.0;
        let pin1 = vp.world_to_screen(board.pins[1].pos, rect);
        assert!(pin_vertices(&vp).iter().any(|p| (*p - pin1).length() <= 11.0));
    }

    #[test]
    fn fixture_deep_zoom_stays_within_vertex_budget() {
        let Some(board) = ms_v375_fixture() else { return };
        let rect = EguiRect::from_min_size(Pos2::ZERO, egui::vec2(2560.0, 1346.0));
        let mut vp = Viewport::default();
        vp.fit_to_rect(board.bounding_box.unwrap(), rect);
        let fit_scale = vp.scale;
        let cn1 = board.parts.iter().find(|p| p.name == "CN1").and_then(|p| p.bounding_box).unwrap();
        let vertices = |vp: &Viewport| -> usize {
            generate_board_meshes_in(
                &board, vp, rect, mesh_coverage(vp, rect), &Tokens::DARK, None, None, None, None, None, None,
                &mut Renderer::new(), true, ViaDisplayMode::Web, false, true, false, true,
            )
            .iter()
            .map(|m| m.vertices.len())
            .sum()
        };
        let fit = vertices(&vp);
        vp.center = cn1.center();
        for zoom in [8.0, 30.0] {
            vp.scale = fit_scale * zoom;
            let deep = vertices(&vp);
            assert!(deep * 4 <= fit, "x{zoom}: {deep} vertices vs {fit} at fit");
        }
    }

    #[test]
    fn wheel_zoom_depends_on_scrolled_distance_not_frame_count() {
        let line = 40.0;
        assert!((wheel_zoom_factor(line, line) - WHEEL_NOTCH_ZOOM).abs() < 1e-5);
        assert!((wheel_zoom_factor(-line, line) - 1.0 / WHEEL_NOTCH_ZOOM).abs() < 1e-5);
        assert_eq!(wheel_zoom_factor(0.0, line), 1.0);
        // One notch smoothed over few (slow) or many (fast) frames zooms the same.
        for frames in [[40.0].as_slice(), &[25.0, 10.0, 5.0], &[8.0; 5]] {
            let total: f32 = frames.iter().map(|&d| wheel_zoom_factor(d, line)).product();
            assert!((total - WHEEL_NOTCH_ZOOM).abs() < 1e-4, "{frames:?}: {total}");
        }
    }

    #[test]
    fn label_font_sizes_snap_to_a_few_cached_sizes() {
        let mut sizes: Vec<f32> = (0..=1000).map(|i| label_font_size(6.0 + i as f32 * 0.012, 8.0, 14.0)).collect();
        assert!(sizes.windows(2).all(|w| w[0] <= w[1]), "monotonic");
        assert!(sizes.iter().all(|s| (8.0..=14.0).contains(s) && s % LABEL_FONT_STEP == 0.0));
        sizes.dedup();
        assert_eq!(sizes, [8.0, 10.0, 12.0, 14.0]);
        assert_eq!(label_font_size(500.0, 10.0, 28.0), 28.0);
    }

    #[test]
    fn view_cache_key_is_stable_across_idle_and_hover_frames() {
        let board = two_pin_board();
        let rect = EguiRect::from_min_size(Pos2::ZERO, egui::vec2(800.0, 600.0));
        let mut vp = Viewport::default();
        vp.fit_to_rect(board.bounding_box.unwrap(), rect);
        let prefs = Preferences::default();
        let key = |vp: &Viewport, pin: Option<usize>, prefs: &Preferences| {
            ViewCacheKey::new(&board, vp, pin, None, None, None, prefs)
        };
        let idle = key(&vp, None, &prefs);
        // Same view on the next frame (hover state is not an input of the key).
        assert_eq!(idle, key(&vp, None, &prefs));
        assert_eq!(idle, key(&vp.clone(), None, &prefs.clone()));

        // Zoom and pan re-map the cached meshes (`sync_board_meshes`), so they are not in the key.
        let mut panned = vp.clone();
        panned.center.x += 1.0;
        panned.scale *= 1.2;
        assert_eq!(idle, key(&panned, None, &prefs));
        let mut flipped = vp.clone();
        flipped.side = Side::Bottom;
        assert_ne!(idle, key(&flipped, None, &prefs));
        assert_ne!(idle, key(&vp, Some(0), &prefs));
        let mut no_pins = prefs.clone();
        no_pins.show_pins = false;
        assert_ne!(idle, key(&vp, None, &no_pins));
    }

    /// A side shown at another canvas size or position (Top tab vs one half of Both) re-uses its
    /// meshes: a translation-only re-map is exact, so it is not re-tessellated when it rests.
    #[test]
    fn same_side_at_a_different_canvas_size_reuses_its_meshes() {
        let board = two_pin_board();
        let full = EguiRect::from_min_size(Pos2::new(0.0, 30.0), egui::vec2(800.0, 600.0));
        let half = EguiRect::from_min_size(Pos2::new(0.0, 30.0), egui::vec2(800.0, 296.0));
        let mut vp = Viewport::default();
        vp.fit_to_rect(board.bounding_box.unwrap(), full);
        let mut state = CanvasState::default();
        assert_eq!(sync(&mut state, &board, &vp, full, None), MeshSync::Rebuilt);
        assert_eq!(sync(&mut state, &board, &vp, half, None), MeshSync::Remapped, "Top -> Both");
        assert_eq!(sync(&mut state, &board, &vp, half, None), MeshSync::Kept, "resting in Both");
        assert_eq!(sync(&mut state, &board, &vp, full, None), MeshSync::Remapped, "Both -> Top");
        assert_eq!(sync(&mut state, &board, &vp, full, None), MeshSync::Kept);
        // The translated meshes sit where a fresh tessellation for the half canvas puts them.
        sync(&mut state, &board, &vp, half, None);
        let pad = state.cached_meshes.last().unwrap().vertices[0].pos;
        let mut fresh_state = CanvasState::default();
        sync(&mut fresh_state, &board, &vp, half, None);
        let fresh = fresh_state.cached_meshes.last().unwrap().vertices[0].pos;
        assert!((pad - fresh).length() < 1e-2, "{pad:?} vs {fresh:?}");
    }

    #[test]
    fn board_canvases_share_rotation_and_mirroring() {
        let mut canvases = BoardCanvases::default();
        assert_eq!(canvases.top.viewport.side, Side::Top);
        assert_eq!(canvases.bottom.viewport.side, Side::Bottom);
        canvases.rotate_cw();
        canvases.toggle_mirror_x();
        for state in [&canvases.top, &canvases.bottom] {
            assert_eq!(state.viewport.rotation, Rotation::Deg90);
            assert!(state.viewport.mirror_x && !state.viewport.mirror_y);
        }
        canvases.rotate_ccw();
        canvases.rotate_180();
        canvases.toggle_mirror_y();
        assert_eq!(canvases.bottom.viewport.rotation, Rotation::Deg180);
        assert!(canvases.bottom.viewport.mirror_y);
        canvases.reset_orientation();
        for state in [&canvases.top, &canvases.bottom] {
            assert_eq!(state.viewport.rotation, Rotation::Deg0);
            assert!(!state.viewport.mirror_x && !state.viewport.mirror_y);
        }
        assert_eq!(canvases.bottom.viewport.side, Side::Bottom, "reset keeps each canvas on its side");
    }

    /// Each side keeps its own cache: showing only one side leaves the hidden side's meshes ready.
    #[test]
    fn a_hidden_side_keeps_its_mesh_cache() {
        let board = two_pin_board();
        let rect = EguiRect::from_min_size(Pos2::ZERO, egui::vec2(800.0, 600.0));
        let mut canvases = BoardCanvases::default();
        for side in [Side::Top, Side::Bottom] {
            let state = canvases.side_mut(side);
            state.viewport.fit_to_rect(board.bounding_box.unwrap(), rect);
            let vp = state.viewport.clone();
            assert_eq!(sync(state, &board, &vp, rect, None), MeshSync::Rebuilt);
        }
        // Only the top side is shown for a while; the bottom one is untouched.
        canvases.top.viewport.scale *= 3.0;
        let vp = canvases.top.viewport.clone();
        assert_eq!(sync(&mut canvases.top, &board, &vp, rect, None), MeshSync::Rebuilt);
        let vp = canvases.bottom.viewport.clone();
        assert_eq!(sync(&mut canvases.bottom, &board, &vp, rect, None), MeshSync::Kept);

        canvases.invalidate_caches();
        assert!(canvases.top.cached_view_key.is_none() && canvases.bottom.cached_view_key.is_none());
        canvases.clear_board();
        assert!(canvases.top.mesh_base.is_none() && canvases.bottom.cached_meshes.is_empty());
    }

    /// The hover highlight is a tiny per-frame mesh, and the cached board meshes do not depend on hover.
    #[test]
    fn hover_overlay_is_small_and_separate_from_board_meshes() {
        let board = two_pin_board();
        let rect = EguiRect::from_min_size(Pos2::ZERO, egui::vec2(800.0, 600.0));
        let mut vp = Viewport::default();
        vp.fit_to_rect(board.bounding_box.unwrap(), rect);
        let tokens = Tokens::DARK;

        let pin_hover = hover_overlay_mesh(&board, &vp, rect, &tokens, Some(1), None, None);
        assert!(!pin_hover.is_empty() && pin_hover.vertices.len() <= 32, "{} vertices", pin_hover.vertices.len());
        assert!(pin_hover.vertices.iter().all(|v| v.color == tokens.hover));
        let via_hover = hover_overlay_mesh(&board, &vp, rect, &tokens, None, Some(0), None);
        assert!(!via_hover.is_empty() && via_hover.vertices.len() <= 32);
        // The selected pin keeps its painter ring only.
        assert!(hover_overlay_mesh(&board, &vp, rect, &tokens, Some(1), None, Some(1)).is_empty());
        assert!(hover_overlay_mesh(&board, &vp, rect, &tokens, None, None, None).is_empty());
    }

    /// Whole-board vertex budget on the GenCAD fixture (skips when absent): filled bodies and
    /// real pads must stay within a few vertices per pin.
    #[test]
    fn fixture_fit_view_stays_within_vertex_budget() {
        let Some(board) = ms_v375_fixture() else { return };
        let rect = EguiRect::from_min_size(Pos2::ZERO, egui::vec2(2560.0, 1346.0));
        let mut vp = Viewport::default();
        vp.fit_to_rect(board.bounding_box.unwrap(), rect);
        let tokens = Tokens::DARK;
        let mut renderer = Renderer::new();
        let meshes = generate_board_meshes(
            &board, &vp, rect, &tokens, None, None, None, None, None, None, &mut renderer,
            true, ViaDisplayMode::Web, false, true, false, true,
        );
        let vertices: usize = meshes.iter().map(|m| m.vertices.len()).sum();
        assert!(meshes.len() <= 8, "{} meshes", meshes.len());
        assert!(vertices <= 12 * board.pins.len(), "{vertices} vertices for {} pins", board.pins.len());
    }

    /// Loads the MS-V375 GenCAD fixture from `tmp/` (skips when absent).
    fn ms_v375_fixture() -> Option<Board> {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tmp/MS-V375 REV 7.0.cad");
        let data = std::fs::read(&path).ok()?;
        Some(bvc_core::ParserRegistry::default().parse_file("MS-V375.cad", &data).expect("parse fixture"))
    }

    /// Median mesh build time on the fixture: whole board and an 8x zoom. Prints timings for comparison.
    #[test]
    fn bench_fixture_mesh_build() {
        let Some(board) = ms_v375_fixture() else { return };
        let canvas_rect = EguiRect::from_min_size(Pos2::ZERO, egui::vec2(1920.0, 1080.0));
        let tokens = Tokens::DARK;
        let mut renderer = Renderer::new();
        let mut vp = Viewport::default();
        vp.fit_to_rect(board.bounding_box.unwrap(), canvas_rect);
        let mut zoomed = vp.clone();
        zoomed.scale *= 8.0;
        for (label, view) in [("fit", &vp), ("zoom8", &zoomed)] {
            let mut times = Vec::new();
            let mut vertices = 0;
            for _ in 0..11 {
                let start = Instant::now();
                let meshes = generate_board_meshes(
                    &board, view, canvas_rect, &tokens, None, None, None, None, None, None, &mut renderer,
                    true, ViaDisplayMode::Web, false, true, false, true,
                );
                times.push(start.elapsed());
                vertices = meshes.iter().map(|m| m.vertices.len()).sum::<usize>();
            }
            times.sort();
            println!("fixture mesh build [{label}]: median {:?}, min {:?}, {vertices} vertices", times[5], times[0]);
            assert!(vertices > 0);
        }
    }

    /// A click on a canvas that is not active yet (touch or pen: no hover frame before it)
    /// makes it take the input that frame, so a right-click menu is not closed at once.
    /// Only a click, right-click or drag start makes a canvas active; merely hovering it (e.g.
    /// crossing the bottom canvas on the way to the side panel) does not.
    #[test]
    fn only_a_click_or_drag_activates_a_canvas() {
        assert!(!activates_canvas(false, false, false));
        assert!(activates_canvas(true, false, false));
        assert!(activates_canvas(false, true, false));
        assert!(activates_canvas(false, false, true));
    }

    /// With a photo shown, the photo is drawn opaque and the opacity slider fades the board
    /// drawn on top of it, so pins stay visible over the photo while aligning.
    #[test]
    fn overlay_opacity_fades_the_board_not_the_photo() {
        assert_eq!(overlay_layer_alphas(true, 0.4), (255, 0.4));
        assert_eq!(overlay_layer_alphas(true, 1.7), (255, 1.0));
        assert_eq!(overlay_layer_alphas(false, 0.4), (255, 1.0), "no photo: the board is opaque");
    }

    #[test]
    fn a_clicked_canvas_takes_input_before_it_is_active() {
        assert!(canvas_takes_input(true, false, false));
        assert!(!canvas_takes_input(false, false, false));
        assert!(canvas_takes_input(false, true, false));
        assert!(canvas_takes_input(false, false, true));
    }

    #[test]
    fn test_both_side_is_unmirrored_like_top() {
        for &mirror_x in &[false, true] {
            for &mirror_y in &[false, true] {
                let vp = |side| Viewport { side, mirror_x, mirror_y, ..Viewport::default() };
                assert_eq!(
                    vp(Side::Both).effective_factors(),
                    vp(Side::Top).effective_factors(),
                    "Both must render like Top (mx={mirror_x}, my={mirror_y})"
                );
                assert_ne!(vp(Side::Both).effective_factors().0, vp(Side::Bottom).effective_factors().0);
            }
        }
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
        let sides = [Side::Top, Side::Bottom, Side::Both];
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

    #[test]
    fn test_via_display_mode_flight_lines() {
        let mut board = Board::new();
        // 2 pins, 2 vias on net "TEST_NET"
        board.pins.push(Pin {
            id: 0,
            part_id: 0,
            name: "1".into(),
            pos: Point::new(0.0, 0.0),
            side: Side::Top,
            net_name: "TEST_NET".into(),
            radius: 10.0,
            ..Default::default()
        });
        board.pins.push(Pin {
            id: 1,
            part_id: 0,
            name: "2".into(),
            pos: Point::new(100.0, 0.0),
            side: Side::Top,
            net_name: "TEST_NET".into(),
            radius: 10.0,
            ..Default::default()
        });
        board.vias.push(Via {
            id: 0,
            pos: Point::new(50.0, 50.0),
            side: Side::Both,
            net_name: "TEST_NET".into(),
            radius: 8.0,
        });
        board.vias.push(Via {
            id: 1,
            pos: Point::new(50.0, -50.0),
            side: Side::Both,
            net_name: "TEST_NET".into(),
            radius: 8.0,
        });
        board.rebuild_indices();

        let vp = Viewport::default();
        let canvas_rect = EguiRect::from_min_size(Pos2::ZERO, egui::vec2(800.0, 600.0));
        let tokens = Tokens::DARK;
        let mut renderer = Renderer::new();

        // 1. If nothing is selected, NO flight lines are rendered at all
        let meshes_none = generate_board_meshes(
            &board,
            &vp,
            canvas_rect,
            &tokens,
            None,
            None,
            None,
            Some("TEST_NET"),
            None,
            None,
            &mut renderer,
            true,
            ViaDisplayMode::Web,
            false,
            true,
            false,
            true,
        );
        // Only board outline/fill meshes, no net web
        assert!(!meshes_none.iter().any(|m| m.vertices.len() == 4 || m.vertices.len() == 12));

        // 2. Web mode with pin 0 selected: 4 nodes -> 3 segments = 12 vertices
        let meshes_web = generate_board_meshes(
            &board,
            &vp,
            canvas_rect,
            &tokens,
            Some(0),
            None,
            None,
            Some("TEST_NET"),
            None,
            None,
            &mut renderer,
            true,
            ViaDisplayMode::Web,
            false,
            true,
            false,
            true,
        );
        let web = meshes_web.last().unwrap();
        assert_eq!(web.vertices.len(), 12);

        // 3. Radial mode with pin 0 selected: lines from pin 0 to the other 3 nodes (pin 1, via 0, via 1) = 3 segments = 12 vertices
        let meshes_radial = generate_board_meshes(
            &board,
            &vp,
            canvas_rect,
            &tokens,
            Some(0),
            None,
            None,
            Some("TEST_NET"),
            None,
            None,
            &mut renderer,
            true,
            ViaDisplayMode::Radial,
            false,
            true,
            false,
            true,
        );
        let web_radial = meshes_radial.last().unwrap();
        assert_eq!(web_radial.vertices.len(), 12);

        // 4. Radial mode with via 0 selected: lines from via 0 to pin 0, pin 1, via 1 = 3 segments = 12 vertices
        let meshes_radial_via = generate_board_meshes(
            &board,
            &vp,
            canvas_rect,
            &tokens,
            None,
            Some(0),
            None,
            Some("TEST_NET"),
            None,
            None,
            &mut renderer,
            true,
            ViaDisplayMode::Radial,
            false,
            true,
            false,
            true,
        );
        let web_radial_via = meshes_radial_via.last().unwrap();
        assert_eq!(web_radial_via.vertices.len(), 12);
    }
}


