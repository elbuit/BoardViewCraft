//! PCB Background Image Overlay management.
//!
//! Handles loading reference photos, managing per-side (Top/Bottom) overlay state,
//! texture allocation with egui, and generating textured quads for the canvas.

use bvc_core::{Point, Rect, Side};
use egui::{ColorImage, Context, TextureHandle, TextureOptions, Vec2};
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
