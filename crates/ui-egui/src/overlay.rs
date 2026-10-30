//! PCB Background Image Overlay management.
//!
//! Handles loading reference photos, managing per-side (Top/Bottom) overlay state,
//! texture allocation with egui, and generating textured quads for the canvas.

use crate::overlay_align::{AlignmentResult, OverlaySnapshot};
use bvc_core::{Point, Rect, Side};
use egui::{ColorImage, Context, Pos2, TextureHandle, TextureOptions, Vec2};
use std::path::{Path, PathBuf};

pub struct OverlaySlot {
    pub file_path: Option<PathBuf>,
    pub texture: Option<TextureHandle>,
    pub orig_dimensions: (u32, u32),
    pub base_size: Vec2, // original fitted base size in board world mils
    pub zoom: f32, // zoom multiplier (1.0 = 100%)
    pub center: Point,
    pub size: Vec2, // in board world mils
    pub aspect_locked: bool,
    pub mirror_x: bool,
    pub mirror_y: bool,
    pub opacity: f32, // 0.0 .. 1.0
    pub fine_rotation_deg: f32, // in degrees relative to PCB
    pub locked: bool,
    pub locked_board_rotation_deg: f32, // PCB rotation angle (rad) at the exact moment of locking
    pub locked_board_sx: f32, // PCB sx at the moment of locking
    pub locked_board_sy: f32, // PCB sy at the moment of locking
    pub visible: bool,
}

impl Default for OverlaySlot {
    fn default() -> Self {
        Self::new()
    }
}

impl OverlaySlot {
    pub fn new() -> Self {
        Self {
            file_path: None,
            texture: None,
            orig_dimensions: (0, 0),
            base_size: Vec2::new(1000.0, 1000.0),
            zoom: 1.0,
            center: Point::ZERO,
            size: Vec2::new(1000.0, 1000.0),
            aspect_locked: true,
            mirror_x: false,
            mirror_y: false,
            opacity: 0.5,
            fine_rotation_deg: 0.0,
            locked: false,
            locked_board_rotation_deg: 0.0,
            locked_board_sx: 1.0,
            locked_board_sy: 1.0,
            visible: true,
        }
    }

    pub fn load_from_bytes(&mut self, ctx: &Context, path: &Path, bytes: &[u8], board_bbox: Option<Rect>) -> Result<(), String> {
        let dyn_img = image::load_from_memory(bytes).map_err(|e| format!("Failed to decode image: {e}"))?;
        let rgba = dyn_img.to_rgba8();
        let (width, height) = rgba.dimensions();

        let color_image = ColorImage::from_rgba_unmultiplied([width as usize, height as usize], rgba.as_raw());
        let texture_name = format!("pcb_overlay_{}", path.file_name().and_then(|f| f.to_str()).unwrap_or("unnamed"));
        let handle = ctx.load_texture(texture_name, color_image, TextureOptions::LINEAR);

        self.file_path = Some(path.to_path_buf());
        self.texture = Some(handle);
        self.orig_dimensions = (width, height);
        self.zoom = 1.0;

        // Initialize default size and center matching the PCB bounding box
        if let Some(bbox) = board_bbox {
            self.center = bbox.center();
            let aspect = width as f32 / height.max(1) as f32;
            let bbox_w = bbox.width() as f32;
            let bbox_h = bbox.height() as f32;

            if bbox_w / bbox_h.max(1.0) > aspect {
                // Board is wider than image: fit height, compute width
                let h = bbox_h;
                let w = h * aspect;
                self.size = Vec2::new(w, h);
            } else {
                // Board is taller: fit width, compute height
                let w = bbox_w;
                let h = w / aspect.max(0.001);
                self.size = Vec2::new(w, h);
            }
        } else {
            self.size = Vec2::new(width as f32, height as f32);
        }

        self.base_size = self.size;
        self.visible = true;
        self.fine_rotation_deg = 0.0;
        self.mirror_x = false;
        self.mirror_y = false;
        self.locked = false;
        self.locked_board_rotation_deg = 0.0;
        self.locked_board_sx = 1.0;
        self.locked_board_sy = 1.0;
        Ok(())
    }

    pub fn set_zoom(&mut self, new_zoom: f32) {
        let z = new_zoom.clamp(0.01, 100.0);
        let factor = z / self.zoom.max(0.001);
        self.zoom = z;
        self.size *= factor;
    }

    pub fn toggle_lock(&mut self, current_rotation_rad: f32, current_sx: f32, current_sy: f32) {
        if !self.locked {
            // Locking now: freeze current PCB orientation as baseline so nothing jumps or rotates
            self.locked = true;
            self.locked_board_rotation_deg = current_rotation_rad;
            self.locked_board_sx = current_sx;
            self.locked_board_sy = current_sy;
        } else {
            // Unlocking
            self.locked = false;
        }
    }

    pub fn clear(&mut self) {
        self.file_path = None;
        self.texture = None;
        self.orig_dimensions = (0, 0);
    }

    pub fn is_loaded(&self) -> bool {
        self.texture.is_some()
    }

    /// Captures a snapshot of current transform parameters for rollback.
    pub fn snapshot(&self) -> OverlaySnapshot {
        OverlaySnapshot {
            center: self.center,
            zoom: self.zoom,
            size: self.size,
            fine_rotation_deg: self.fine_rotation_deg,
            locked: self.locked,
        }
    }

    /// Restores transform parameters from a snapshot.
    pub fn restore_snapshot(&mut self, snapshot: OverlaySnapshot) {
        self.center = snapshot.center;
        self.zoom = snapshot.zoom;
        self.size = snapshot.size;
        self.fine_rotation_deg = snapshot.fine_rotation_deg;
        self.locked = snapshot.locked;
    }

    /// Applies calculated alignment result to slot transform.
    pub fn apply_alignment_result(
        &mut self,
        res: &AlignmentResult,
        auto_lock: bool,
        current_rotation_rad: f32,
        current_sx: f32,
        current_sy: f32,
    ) {
        self.center = res.center;
        self.set_zoom(res.zoom);
        self.fine_rotation_deg = res.rotation_deg;
        if auto_lock {
            self.locked = true;
            self.locked_board_rotation_deg = current_rotation_rad;
            self.locked_board_sx = current_sx;
            self.locked_board_sy = current_sy;
        }
    }

    /// Computes effective reflection factors and rotation for this overlay slot.
    pub fn overlay_factors(&self, vp: &crate::canvas::Viewport) -> (f32, f32, f32) {
        let (vp_sx, vp_sy) = vp.effective_factors();
        if self.locked {
            let sx = if self.mirror_x { -vp_sx } else { vp_sx };
            let sy = if self.mirror_y { -vp_sy } else { vp_sy };
            (sx, sy, vp.rotation.angle_rad())
        } else {
            let sx = if self.mirror_x { -1.0 } else { 1.0 };
            let sy = if self.mirror_y { -1.0 } else { 1.0 };
            (sx, sy, 0.0)
        }
    }

    /// Converts a photo local point (in unzoomed mils relative to image center) to screen coordinates.
    pub fn photo_local_to_screen(&self, local_pt: Point, canvas_rect: egui::Rect, vp: &crate::canvas::Viewport) -> Pos2 {
        let (sin_f, cos_f) = (self.fine_rotation_deg.to_radians() as f64).sin_cos();

        // 1. Scale by zoom
        let z = self.zoom as f64;
        let scaled_x = local_pt.x * z;
        let scaled_y = local_pt.y * z;

        // 2. Mirroring
        let mx = if self.mirror_x { -scaled_x } else { scaled_x };
        let my = if self.mirror_y { -scaled_y } else { scaled_y };

        // 3. Fine rotation
        let rx = mx * cos_f - my * sin_f;
        let ry = mx * sin_f + my * cos_f;

        // 4. Translate to board world position
        let world_pt = Point::new(self.center.x + rx, self.center.y + ry);

        // 5. Project world point to screen with slot factors
        let (sx, sy, rot) = self.overlay_factors(vp);
        vp.world_to_screen_with_angle(world_pt, canvas_rect, sx, sy, rot)
    }

    /// Converts a screen coordinate to a photo local point (in unzoomed mils relative to image center).
    pub fn screen_to_photo_local(&self, screen_pt: Pos2, canvas_rect: egui::Rect, vp: &crate::canvas::Viewport) -> Point {
        let (sx, sy, rot) = self.overlay_factors(vp);
        let center_screen = canvas_rect.center();

        // Invert world_to_screen_with_angle:
        // 1. Un-center and un-scale screen position
        let x_rot = (screen_pt.x - center_screen.x) / vp.scale;
        let y_rot = (screen_pt.y - center_screen.y) / vp.scale;

        // 2. Un-rotate screen angle
        let (sin_r, cos_r) = rot.sin_cos();
        let dx_scr = x_rot * cos_r + y_rot * sin_r;
        let dy_cad = -x_rot * sin_r + y_rot * cos_r;
        let dy_scr = -dy_cad;

        // 3. Un-scale viewport reflections and restore world center
        let world_x = vp.center.x + (dx_scr / sx) as f64;
        let world_y = vp.center.y + (dy_scr / sy) as f64;

        // 4. Invert slot translation
        let dx_slot = world_x - self.center.x;
        let dy_slot = world_y - self.center.y;

        // 5. Invert fine rotation
        let (sin_f, cos_f) = (self.fine_rotation_deg.to_radians() as f64).sin_cos();
        let mx = dx_slot * cos_f + dy_slot * sin_f;
        let my = -dx_slot * sin_f + dy_slot * cos_f;

        // 6. Invert mirroring
        let scaled_x = if self.mirror_x { -mx } else { mx };
        let scaled_y = if self.mirror_y { -my } else { my };

        // 7. Invert zoom
        let z = (self.zoom as f64).max(0.0001);
        Point::new(scaled_x / z, scaled_y / z)
    }

    /// Converts a board world coordinate into overlay normalized UV coordinates [0.0..1.0].
    /// Returns None if overlay dimensions or size are invalid.
    #[allow(dead_code)]
    pub fn world_to_overlay_uv(&self, pt: Point) -> Option<Pos2> {
        if self.size.x <= 0.0 || self.size.y <= 0.0 {
            return None;
        }
        let half_w = (self.size.x * 0.5) as f64;
        let half_h = (self.size.y * 0.5) as f64;
        let fine_rad = self.fine_rotation_deg.to_radians();
        let (sin_f, cos_f) = (fine_rad as f64).sin_cos();

        // Translate relative to overlay center
        let dx = pt.x - self.center.x;
        let dy = pt.y - self.center.y;

        // Un-rotate by fine_rotation_deg
        let lx = dx * cos_f + dy * sin_f;
        let ly = -dx * sin_f + dy * cos_f;

        // Account for overlay mirror flags
        let mx = if self.mirror_x { -lx } else { lx };
        let my = if self.mirror_y { -ly } else { ly };

        // Normalize [-half_w..half_w] -> [0.0..1.0] in CAD space (+Y up, UV v=0 top)
        let u = ((mx + half_w) / (self.size.x as f64)).clamp(0.0, 1.0) as f32;
        let v = ((half_h - my) / (self.size.y as f64)).clamp(0.0, 1.0) as f32;

        Some(Pos2::new(u, v))
    }

    /// Converts overlay normalized UV coordinates [0.0..1.0] to board world coordinates.
    #[allow(dead_code)]
    pub fn overlay_uv_to_world(&self, uv: Pos2) -> Point {
        let half_w = (self.size.x * 0.5) as f64;
        let half_h = (self.size.y * 0.5) as f64;
        let fine_rad = self.fine_rotation_deg.to_radians();
        let (sin_f, cos_f) = (fine_rad as f64).sin_cos();

        // Normalized [0.0..1.0] -> local mils [-half_w..half_w] in CAD space (+Y up, UV v=0 top)
        let mut mx = (uv.x as f64) * (self.size.x as f64) - half_w;
        let mut my = half_h - (uv.y as f64) * (self.size.y as f64);

        if self.mirror_x {
            mx = -mx;
        }
        if self.mirror_y {
            my = -my;
        }

        // Rotate by fine_rotation_deg
        let dx = mx * cos_f - my * sin_f;
        let dy = mx * sin_f + my * cos_f;

        Point::new(self.center.x + dx, self.center.y + dy)
    }
}

pub struct OverlayManager {
    pub top: OverlaySlot,
    pub bottom: OverlaySlot,
}

impl Default for OverlayManager {
    fn default() -> Self {
        Self {
            top: OverlaySlot::new(),
            bottom: OverlaySlot::new(),
        }
    }
}

impl OverlayManager {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn slot_for_side(&mut self, side: Side) -> &mut OverlaySlot {
        match side {
            Side::Bottom => &mut self.bottom,
            _ => &mut self.top,
        }
    }

    #[allow(dead_code)]
    pub fn slot_for_side_ref(&self, side: Side) -> &OverlaySlot {
        match side {
            Side::Bottom => &self.bottom,
            _ => &self.top,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_overlay_slot_snapshot_restore() {
        let mut slot = OverlaySlot::new();
        slot.center = Point::new(123.0, 456.0);
        slot.zoom = 1.75;
        slot.size = Vec2::new(800.0, 600.0);
        slot.fine_rotation_deg = 15.0;
        slot.locked = true;

        let snapshot = slot.snapshot();
        assert_eq!(snapshot.center, Point::new(123.0, 456.0));
        assert_eq!(snapshot.zoom, 1.75);
        assert_eq!(snapshot.size, Vec2::new(800.0, 600.0));
        assert_eq!(snapshot.fine_rotation_deg, 15.0);
        assert!(snapshot.locked);

        // Mutate slot
        slot.center = Point::ZERO;
        slot.zoom = 1.0;
        slot.fine_rotation_deg = 0.0;
        slot.locked = false;

        // Restore snapshot
        slot.restore_snapshot(snapshot);
        assert_eq!(slot.center, Point::new(123.0, 456.0));
        assert_eq!(slot.zoom, 1.75);
        assert_eq!(slot.fine_rotation_deg, 15.0);
        assert!(slot.locked);
    }

    #[test]
    fn test_overlay_slot_uv_world_roundtrip() {
        let mut slot = OverlaySlot::new();
        slot.center = Point::new(100.0, 200.0);
        slot.size = Vec2::new(500.0, 300.0);
        slot.fine_rotation_deg = 30.0;

        let uv_orig = Pos2::new(0.25, 0.75);
        let world = slot.overlay_uv_to_world(uv_orig);
        let uv_back = slot.world_to_overlay_uv(world).expect("UV conversion failed");

        assert!((uv_back.x - uv_orig.x).abs() < 1e-4);
        assert!((uv_back.y - uv_orig.y).abs() < 1e-4);
    }

    #[test]
    fn test_screen_photo_local_roundtrip_unlocked() {
        let mut slot = OverlaySlot::new();
        slot.center = Point::new(150.0, 250.0);
        slot.zoom = 1.4;
        slot.fine_rotation_deg = 25.0;
        slot.mirror_x = false;
        slot.mirror_y = false;
        slot.locked = false;

        let mut vp = crate::canvas::Viewport::default();
        vp.scale = 0.08;
        vp.center = Point::new(100.0, 100.0);
        vp.rotation = crate::canvas::Rotation::Deg90;

        let canvas_rect = egui::Rect::from_min_size(Pos2::new(100.0, 50.0), Vec2::new(800.0, 600.0));

        let local_orig = Point::new(45.0, -80.0);
        let screen = slot.photo_local_to_screen(local_orig, canvas_rect, &vp);
        let local_back = slot.screen_to_photo_local(screen, canvas_rect, &vp);

        assert!((local_back.x - local_orig.x).abs() < 1e-3);
        assert!((local_back.y - local_orig.y).abs() < 1e-3);
    }

    #[test]
    fn test_screen_photo_local_roundtrip_locked_with_mirror() {
        let mut slot = OverlaySlot::new();
        slot.center = Point::new(200.0, -150.0);
        slot.zoom = 0.85;
        slot.fine_rotation_deg = -40.0;
        slot.mirror_x = true;
        slot.mirror_y = false;
        slot.locked = true;
        slot.locked_board_rotation_deg = 0.0;
        slot.locked_board_sx = 1.0;
        slot.locked_board_sy = 1.0;

        let mut vp = crate::canvas::Viewport::default();
        vp.scale = 0.12;
        vp.center = Point::new(180.0, -120.0);
        vp.rotation = crate::canvas::Rotation::Deg180;
        vp.mirror_x = true;

        let canvas_rect = egui::Rect::from_min_size(Pos2::new(50.0, 50.0), Vec2::new(1024.0, 768.0));

        let local_orig = Point::new(-120.0, 95.0);
        let screen = slot.photo_local_to_screen(local_orig, canvas_rect, &vp);
        let local_back = slot.screen_to_photo_local(screen, canvas_rect, &vp);

        assert!((local_back.x - local_orig.x).abs() < 1e-3);
        assert!((local_back.y - local_orig.y).abs() < 1e-3);
    }

    #[test]
    fn test_locked_photo_invariance_under_zoom_and_pan() {
        let mut slot = OverlaySlot::new();
        slot.center = Point::new(300.0, 400.0);
        slot.zoom = 1.2;
        slot.fine_rotation_deg = 15.0;
        slot.locked = true;
        slot.locked_board_rotation_deg = 0.0;
        slot.locked_board_sx = 1.0;
        slot.locked_board_sy = 1.0;

        // Viewport 1: base scale and position
        let mut vp1 = crate::canvas::Viewport::default();
        vp1.scale = 0.05;
        vp1.center = Point::new(100.0, 100.0);

        // Viewport 2: zoomed in 2.5x and panned
        let mut vp2 = crate::canvas::Viewport::default();
        vp2.scale = 0.125;
        vp2.center = Point::new(250.0, 350.0);

        let canvas_rect = egui::Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));

        // A feature point in local photo coordinates
        let local_pt = Point::new(20.0, -30.0);

        // In CAD world space, what point does this correspond to?
        let (sin_f, cos_f) = (slot.fine_rotation_deg.to_radians() as f64).sin_cos();
        let z = slot.zoom as f64;
        let rx = local_pt.x * z * cos_f - local_pt.y * z * sin_f;
        let ry = local_pt.x * z * sin_f + local_pt.y * z * cos_f;
        let world_cad_pt = Point::new(slot.center.x + rx, slot.center.y + ry);

        // Under vp1, photo screen point MUST equal CAD pin screen point
        let scr_photo_vp1 = slot.photo_local_to_screen(local_pt, canvas_rect, &vp1);
        let scr_cad_vp1 = vp1.world_to_screen(world_cad_pt, canvas_rect);
        assert!((scr_photo_vp1.x - scr_cad_vp1.x).abs() < 1e-4);
        assert!((scr_photo_vp1.y - scr_cad_vp1.y).abs() < 1e-4);

        // Under vp2 (zoomed & panned), photo screen point MUST STILL equal CAD pin screen point!
        let scr_photo_vp2 = slot.photo_local_to_screen(local_pt, canvas_rect, &vp2);
        let scr_cad_vp2 = vp2.world_to_screen(world_cad_pt, canvas_rect);
        assert!((scr_photo_vp2.x - scr_cad_vp2.x).abs() < 1e-4);
        assert!((scr_photo_vp2.y - scr_cad_vp2.y).abs() < 1e-4);

        // Viewport 3: rotated 90 deg, mirrored X (Bottom side), zoomed in and panned
        let mut vp3 = crate::canvas::Viewport::default();
        vp3.scale = 0.2;
        vp3.center = Point::new(320.0, 390.0);
        vp3.rotation = crate::canvas::Rotation::Deg90;
        vp3.side = bvc_core::Side::Bottom;

        let scr_photo_vp3 = slot.photo_local_to_screen(local_pt, canvas_rect, &vp3);
        let scr_cad_vp3 = vp3.world_to_screen(world_cad_pt, canvas_rect);
        assert!((scr_photo_vp3.x - scr_cad_vp3.x).abs() < 1e-4);
        assert!((scr_photo_vp3.y - scr_cad_vp3.y).abs() < 1e-4);
    }
}
