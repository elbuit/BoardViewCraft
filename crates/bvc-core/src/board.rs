//! Board container and indexing structures for BoardViewCraft.

use std::collections::BTreeMap;
use crate::model::{BoardMetadata, Net, NetMembership, Part, Pin, Point, Rect, Side, Via};
use crate::spatial::SpatialGrid;

/// High-level board representation with fast lookups.
#[derive(Debug, Clone, Default)]
pub struct Board {
    pub parts: Vec<Part>,
    pub pins: Vec<Pin>,
    pub vias: Vec<Via>,
    pub outline_polygon: Vec<Point>,
    pub bounding_box: Option<Rect>,

    // Lookup caches
    pub nets: BTreeMap<String, Net>,
    pub part_by_name: BTreeMap<String, usize>,
    pub spatial_grid: SpatialGrid,

    /// Repair metadata (part values, readings, voltages) when the source format carries it.
    pub metadata: BoardMetadata,
}

/// Margin (mils) around a pin centre whose pad has no known physical size.
const SIZELESS_PAD_MARGIN: f64 = 6.0;

/// Target minimum extent of an outline axis, as a fraction of the shape's pin pitch.
const OUTLINE_MIN_EXTENT_PITCH_FRACTION: f64 = 0.6;

/// Lower bound (in mils) on the extent a thin axis is grown to (0.3 mm).
const OUTLINE_MIN_EXTENT_MILS: f64 = 0.3 * crate::model::MILS_PER_MM;

/// Upper bound (in mils) on the extent a thin axis is grown to (5.0 mm).
const OUTLINE_MAX_GROWN_EXTENT_MILS: f64 = 5.0 * crate::model::MILS_PER_MM;

/// Typical pin pitch of a component (in mils): the median distance over pins to the
/// nearest distinct neighbor pad.
///
/// Pads closer than 1% of the pin cloud's maximum extent (duplicate names/split fingers on one pad)
/// are treated as coincident and ignored. Returns None when fewer than two distinct pad positions exist.
pub fn compute_pin_pitch(pins: &[&Pin]) -> Option<f64> {
    let pts: Vec<Point> = pins.iter().map(|p| p.pos).collect();
    let n = pts.len();
    if n < 2 {
        return None;
    }
    let min_x = pts.iter().map(|p| p.x).fold(f64::INFINITY, f64::min);
    let max_x = pts.iter().map(|p| p.x).fold(f64::NEG_INFINITY, f64::max);
    let min_y = pts.iter().map(|p| p.y).fold(f64::INFINITY, f64::min);
    let max_y = pts.iter().map(|p| p.y).fold(f64::NEG_INFINITY, f64::max);
    let extent = (max_x - min_x).max(max_y - min_y);
    let eps2 = (extent * 0.01).powi(2);

    let step = if n > 64 { (n / 32).max(1) } else { 1 };
    let mut nearest = Vec::with_capacity(n);

    for (i, p1) in pts.iter().enumerate().step_by(step) {
        let mut best_d2: Option<f64> = None;
        for (j, p2) in pts.iter().enumerate() {
            if i == j {
                continue;
            }
            let dx = p1.x - p2.x;
            let dy = p1.y - p2.y;
            let d2 = dx * dx + dy * dy;
            if d2 <= eps2 {
                continue;
            }
            if best_d2.map_or(true, |best| d2 < best) {
                best_d2 = Some(d2);
            }
        }
        if let Some(d2) = best_d2 {
            nearest.push(d2.sqrt());
        }
    }

    if nearest.is_empty() {
        None
    } else {
        nearest.sort_by(|a, b| a.total_cmp(b));
        Some(nearest[nearest.len() / 2])
    }
}

/// Synthesizes a component body outline (as 4 rotated corner points in board coordinates)
/// from the part's pins and placement rotation when the file format does not provide an explicit outline.
pub fn synthesize_part_outline(part: &Part, pins: &[Pin]) -> Vec<Point> {
    let pin_list: Vec<&Pin> = part.pin_indices.iter().filter_map(|&idx| pins.get(idx)).collect();
    if pin_list.is_empty() {
        return Vec::new();
    }

    let center = Point::new(
        pin_list.iter().map(|p| p.pos.x).sum::<f64>() / pin_list.len() as f64,
        pin_list.iter().map(|p| p.pos.y).sum::<f64>() / pin_list.len() as f64,
    );

    let theta_deg = if part.rotation_deg.abs() > 1e-3 {
        part.rotation_deg
    } else if let Some(pin) = pin_list.iter().find(|p| p.rotation_deg.abs() > 1e-3) {
        if pin_list.iter().all(|p| p.rotation_deg.abs() < 1e-3 || (p.rotation_deg - pin.rotation_deg).abs() < 1e-3) {
            pin.rotation_deg
        } else {
            0.0
        }
    } else if pin_list.len() == 2 && pin_list[0].pos.dist(pin_list[1].pos) > 1e-3 {
        let dx = pin_list[1].pos.x - pin_list[0].pos.x;
        let dy = pin_list[1].pos.y - pin_list[0].pos.y;
        dy.atan2(dx).to_degrees()
    } else {
        0.0
    };

    let (cos_t, sin_t) = crate::model::quadrant(theta_deg);

    let mut min_lx = f64::INFINITY;
    let mut max_lx = f64::NEG_INFINITY;
    let mut min_ly = f64::INFINITY;
    let mut max_ly = f64::NEG_INFINITY;

    for pin in &pin_list {
        let (w, h) = pin.pad_size().unwrap_or((SIZELESS_PAD_MARGIN * 2.0, SIZELESS_PAD_MARGIN * 2.0));
        let half_w = w * 0.5;
        let half_h = h * 0.5;
        let (cos_p, sin_p) = crate::model::quadrant(pin.rotation_deg);
        let u_x = cos_p * half_w;
        let u_y = sin_p * half_w;
        let v_x = -sin_p * half_h;
        let v_y = cos_p * half_h;

        let pad_corners = [
            Point::new(pin.pos.x + u_x + v_x, pin.pos.y + u_y + v_y),
            Point::new(pin.pos.x - u_x + v_x, pin.pos.y - u_y + v_y),
            Point::new(pin.pos.x - u_x - v_x, pin.pos.y - u_y - v_y),
            Point::new(pin.pos.x + u_x - v_x, pin.pos.y + u_y - v_y),
        ];

        for corner in pad_corners {
            let dx = corner.x - center.x;
            let dy = corner.y - center.y;
            let lx = dx * cos_t + dy * sin_t;
            let ly = -dx * sin_t + dy * cos_t;
            min_lx = min_lx.min(lx);
            max_lx = max_lx.max(lx);
            min_ly = min_ly.min(ly);
            max_ly = max_ly.max(ly);
        }
    }

    if !min_lx.is_finite() || !max_lx.is_finite() || !min_ly.is_finite() || !max_ly.is_finite() {
        return Vec::new();
    }

    // Expand thin/degenerate axes for collinear or 2-pin passives based on pin pitch
    if let Some(pitch) = compute_pin_pitch(&pin_list) {
        let target_extent = (OUTLINE_MIN_EXTENT_PITCH_FRACTION * pitch)
            .clamp(OUTLINE_MIN_EXTENT_MILS, OUTLINE_MAX_GROWN_EXTENT_MILS);

        let extent_x = max_lx - min_lx;
        if extent_x < target_extent {
            let pad = (target_extent - extent_x) * 0.5;
            min_lx -= pad;
            max_lx += pad;
        }

        let extent_y = max_ly - min_ly;
        if extent_y < target_extent {
            let pad = (target_extent - extent_y) * 0.5;
            min_ly -= pad;
            max_ly += pad;
        }
    }

    let local_corners = [
        (min_lx, min_ly),
        (max_lx, min_ly),
        (max_lx, max_ly),
        (min_lx, max_ly),
    ];

    local_corners
        .iter()
        .map(|&(lx, ly)| {
            let wx = center.x + (lx * cos_t - ly * sin_t);
            let wy = center.y + (lx * sin_t + ly * cos_t);
            Point::new(wx, wy)
        })
        .collect()
}

/// Bounding box of a part: its outline when it has one, otherwise the union of its pads' rotated
/// physical extents (pads without a size contribute their centre plus [`SIZELESS_PAD_MARGIN`]).
///
/// Portions adapted from TRACE Boardviewer (MIT), Copyright (c) 2026 TRACE Boardviewer contributors
/// (component bounds in `src/lib/formats/common.ts`).
pub fn compute_part_bounds(part: &Part, pins: &[Pin]) -> Option<Rect> {
    if part.outline.len() >= 3 {
        return Rect::from_points(&part.outline);
    }
    let mut bounds: Option<Rect> = None;
    for pin in part.pin_indices.iter().filter_map(|&idx| pins.get(idx)) {
        let (hx, hy) = pin.pad_half_extents().unwrap_or((SIZELESS_PAD_MARGIN, SIZELESS_PAD_MARGIN));
        let r = Rect {
            min_x: pin.pos.x - hx,
            min_y: pin.pos.y - hy,
            max_x: pin.pos.x + hx,
            max_y: pin.pos.y + hy,
        };
        bounds = Some(match bounds {
            None => r,
            Some(b) => Rect {
                min_x: b.min_x.min(r.min_x),
                min_y: b.min_y.min(r.min_y),
                max_x: b.max_x.max(r.max_x),
                max_y: b.max_y.max(r.max_y),
            },
        });
    }
    bounds.filter(|b| b.min_x.is_finite() && b.max_x.is_finite() && b.min_y.is_finite() && b.max_y.is_finite())
}

impl Board {
    pub fn new() -> Self {
        Self::default()
    }

    /// Rebuilds index tables (nets and part_by_name) from current parts, pins, and vias.
    pub fn rebuild_indices(&mut self) {
        self.part_by_name.clear();
        for (idx, part) in self.parts.iter().enumerate() {
            self.part_by_name.insert(part.name.to_ascii_uppercase(), idx);
        }

        self.nets.clear();
        for (idx, pin) in self.pins.iter().enumerate() {
            if !pin.net_name.is_empty() {
                let entry = self.nets.entry(pin.net_name.clone()).or_insert_with(|| Net {
                    name: pin.net_name.clone(),
                    pin_indices: Vec::new(),
                    via_indices: Vec::new(),
                });
                entry.pin_indices.push(idx);
            }
        }

        for (idx, via) in self.vias.iter().enumerate() {
            if !via.net_name.is_empty() {
                let entry = self.nets.entry(via.net_name.clone()).or_insert_with(|| Net {
                    name: via.net_name.clone(),
                    pin_indices: Vec::new(),
                    via_indices: Vec::new(),
                });
                entry.via_indices.push(idx);
            }
        }

        // Link pins to parts if not already linked
        for part in &mut self.parts {
            part.pin_indices.clear();
        }
        for (pin_idx, pin) in self.pins.iter().enumerate() {
            if pin.part_id < self.parts.len() {
                self.parts[pin.part_id].pin_indices.push(pin_idx);
            }
        }

        // Precompute part outlines and bounding boxes if not already defined by the file format
        for part in &mut self.parts {
            if part.outline.is_empty() {
                part.outline = synthesize_part_outline(part, &self.pins);
            }
            if part.bounding_box.is_none() {
                part.bounding_box = compute_part_bounds(part, &self.pins);
            }
        }

        // Compute board bounding box
        self.recompute_bounds();

        // Build 2D uniform spatial grid for fast frustum queries
        self.spatial_grid = SpatialGrid::build(&self.pins, &self.vias, SpatialGrid::DEFAULT_CELL_SIZE);
    }

    pub fn recompute_bounds(&mut self) {
        let mut pts = Vec::with_capacity(self.pins.len() + self.vias.len() + self.outline_polygon.len());
        for p in &self.outline_polygon {
            pts.push(p);
        }
        for pin in &self.pins {
            pts.push(&pin.pos);
        }
        for via in &self.vias {
            pts.push(&via.pos);
        }

        if let Some(mut bbox) = Rect::from_points(pts) {
            if bbox.width() < 10.0 {
                bbox.max_x += 10.0;
            }
            if bbox.height() < 10.0 {
                bbox.max_y += 10.0;
            }
            self.bounding_box = Some(bbox);
        } else {
            self.bounding_box = None;
        }
    }

    /// Finds a pin whose physical pad geometry contains `world_pos` on the given side.
    pub fn find_pin_at(&self, world_pos: Point, view_side: Side, min_radius: f64) -> Option<usize> {
        let mut best: Option<(usize, f64)> = None;

        if !self.spatial_grid.is_empty() {
            let search_margin = (min_radius + 50.0).max(100.0);
            let q_rect = Rect {
                min_x: world_pos.x - search_margin,
                max_x: world_pos.x + search_margin,
                min_y: world_pos.y - search_margin,
                max_y: world_pos.y + search_margin,
            };
            let candidates = self.spatial_grid.query_pins_in_rect(&q_rect);
            for &idx in &candidates {
                if let Some(pin) = self.pins.get(idx) {
                    if !pin.side.visible_on(view_side) {
                        continue;
                    }
                    if pin.contains_point(world_pos, min_radius) {
                        let d = pin.pos.dist(world_pos);
                        match best {
                            None => best = Some((idx, d)),
                            Some((_, best_d)) if d < best_d => best = Some((idx, d)),
                            _ => {}
                        }
                    }
                }
            }
            return best.map(|(idx, _)| idx);
        }

        for (idx, pin) in self.pins.iter().enumerate() {
            if !pin.side.visible_on(view_side) {
                continue;
            }
            if pin.contains_point(world_pos, min_radius) {
                let d = pin.pos.dist(world_pos);
                match best {
                    None => best = Some((idx, d)),
                    Some((_, best_d)) if d < best_d => best = Some((idx, d)),
                    _ => {}
                }
            }
        }
        best.map(|(idx, _)| idx)
    }

    /// Finds a pin near `world_pos` on the given side within tolerance `max_dist`.
    pub fn find_pin_near(&self, world_pos: Point, view_side: Side, max_dist: f64) -> Option<usize> {
        let mut best: Option<(usize, f64)> = None;

        if !self.spatial_grid.is_empty() {
            let search_margin = (max_dist + 50.0).max(100.0);
            let q_rect = Rect {
                min_x: world_pos.x - search_margin,
                max_x: world_pos.x + search_margin,
                min_y: world_pos.y - search_margin,
                max_y: world_pos.y + search_margin,
            };
            let candidates = self.spatial_grid.query_pins_in_rect(&q_rect);
            for &idx in &candidates {
                if let Some(pin) = self.pins.get(idx) {
                    if !pin.side.visible_on(view_side) {
                        continue;
                    }
                    let d = pin.pos.dist(world_pos);
                    let hit_radius = (pin.radius * 1.5).max(max_dist);
                    if d <= hit_radius {
                        match best {
                            None => best = Some((idx, d)),
                            Some((_, best_d)) if d < best_d => best = Some((idx, d)),
                            _ => {}
                        }
                    }
                }
            }
            return best.map(|(idx, _)| idx);
        }

        for (idx, pin) in self.pins.iter().enumerate() {
            if !pin.side.visible_on(view_side) {
                continue;
            }
            let d = pin.pos.dist(world_pos);
            let hit_radius = (pin.radius * 1.5).max(max_dist);
            if d <= hit_radius {
                match best {
                    None => best = Some((idx, d)),
                    Some((_, best_d)) if d < best_d => best = Some((idx, d)),
                    _ => {}
                }
            }
        }
        best.map(|(idx, _)| idx)
    }

    /// Finds a via near `world_pos` within tolerance `max_dist`.
    pub fn find_via_near(&self, world_pos: Point, view_side: Side, max_dist: f64) -> Option<usize> {
        let mut best: Option<(usize, f64)> = None;

        if !self.spatial_grid.is_empty() {
            let search_margin = (max_dist + 50.0).max(100.0);
            let q_rect = Rect {
                min_x: world_pos.x - search_margin,
                max_x: world_pos.x + search_margin,
                min_y: world_pos.y - search_margin,
                max_y: world_pos.y + search_margin,
            };
            let candidates = self.spatial_grid.query_vias_in_rect(&q_rect);
            for &idx in &candidates {
                if let Some(via) = self.vias.get(idx) {
                    if !via.side.visible_on(view_side) {
                        continue;
                    }
                    let d = via.pos.dist(world_pos);
                    let hit_radius = (via.radius * 1.5).max(max_dist);
                    if d <= hit_radius {
                        match best {
                            None => best = Some((idx, d)),
                            Some((_, best_d)) if d < best_d => best = Some((idx, d)),
                            _ => {}
                        }
                    }
                }
            }
            return best.map(|(idx, _)| idx);
        }

        for (idx, via) in self.vias.iter().enumerate() {
            if !via.side.visible_on(view_side) {
                continue;
            }
            let d = via.pos.dist(world_pos);
            let hit_radius = (via.radius * 1.5).max(max_dist);
            if d <= hit_radius {
                match best {
                    None => best = Some((idx, d)),
                    Some((_, best_d)) if d < best_d => best = Some((idx, d)),
                    _ => {}
                }
            }
        }
        best.map(|(idx, _)| idx)
    }

    /// Finds a part whose bounding box contains `world_pos` on `view_side`.
    /// When multiple components overlap, prefers the one with the smallest bounding box area.
    pub fn find_part_near(&self, world_pos: Point, view_side: Side) -> Option<usize> {
        let mut best_idx: Option<usize> = None;
        let mut best_area = f64::INFINITY;

        for (idx, part) in self.parts.iter().enumerate() {
            if !part.side.visible_on(view_side) {
                continue;
            }
            if let Some(bbox) = &part.bounding_box {
                if bbox.contains(world_pos) {
                    let area = bbox.width() * bbox.height();
                    if area < best_area {
                        best_area = area;
                        best_idx = Some(idx);
                    }
                }
            }
        }

        best_idx
    }

    /// Finds part by exact or case-insensitive name.
    pub fn find_part_by_name(&self, name: &str) -> Option<&Part> {
        let key = name.trim().to_ascii_uppercase();
        self.part_by_name.get(&key).and_then(|&id| self.parts.get(id))
    }

    /// Gets net by name.
    pub fn get_net(&self, name: &str) -> Option<&Net> {
        self.nets.get(name)
    }

    /// Returns a fast O(1) bitset membership checker for the specified net name.
    pub fn net_membership(&self, name: &str) -> NetMembership {
        NetMembership::from_net(self.pins.len(), self.vias.len(), self.get_net(name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::PadShape;

    fn pin(idx: usize, x: f64, y: f64) -> Pin {
        Pin { id: idx, pos: Point::new(x, y), radius: 5.0, ..Default::default() }
    }

    fn part_with(pins: &[Pin]) -> Part {
        Part { pin_indices: (0..pins.len()).collect(), ..Default::default() }
    }

    fn assert_rect(r: Rect, min_x: f64, min_y: f64, max_x: f64, max_y: f64) {
        let close = |a: f64, b: f64| (a - b).abs() < 1e-9;
        assert!(
            close(r.min_x, min_x) && close(r.min_y, min_y) && close(r.max_x, max_x) && close(r.max_y, max_y),
            "got {r:?}, expected ({min_x}, {min_y}, {max_x}, {max_y})"
        );
    }

    #[test]
    fn sizeless_pads_keep_the_fixed_margin() {
        let pins = vec![pin(0, 0.0, 0.0), pin(1, 100.0, 50.0)];
        let r = compute_part_bounds(&part_with(&pins), &pins).unwrap();
        assert_rect(r, -6.0, -6.0, 106.0, 56.0);
    }

    #[test]
    fn sized_pads_contribute_their_rotated_extent() {
        let mut a = pin(0, 0.0, 0.0);
        a.shape = PadShape::Rect;
        a.width = Some(10.0);
        a.height = Some(40.0);
        let mut b = a.clone();
        b.pos = Point::new(100.0, 0.0);
        b.rotation_deg = 90.0;
        let pins = vec![a, b];
        let r = compute_part_bounds(&part_with(&pins), &pins).unwrap();
        // a: half (5, 20); b rotated 90 degrees: half (20, 5).
        assert_rect(r, -5.0, -20.0, 120.0, 20.0);
    }

    #[test]
    fn rotated_square_pad_uses_its_enclosing_box() {
        let mut a = pin(0, 0.0, 0.0);
        a.shape = PadShape::Square;
        a.width = Some(10.0);
        a.height = Some(10.0);
        a.rotation_deg = 45.0;
        let pins = vec![a];
        let r = compute_part_bounds(&part_with(&pins), &pins).unwrap();
        let h = 5.0 * std::f64::consts::SQRT_2;
        assert_rect(r, -h, -h, h, h);
    }

    #[test]
    fn mixed_sized_and_sizeless_pads() {
        let mut a = pin(0, 0.0, 0.0);
        a.width = Some(30.0);
        a.height = Some(30.0);
        let pins = vec![a, pin(1, 100.0, 0.0)];
        let r = compute_part_bounds(&part_with(&pins), &pins).unwrap();
        assert_rect(r, -15.0, -15.0, 106.0, 15.0);
    }

    #[test]
    fn outline_takes_precedence_over_pads() {
        let pins = vec![pin(0, 0.0, 0.0)];
        let mut part = part_with(&pins);
        part.outline = vec![Point::new(-50.0, -10.0), Point::new(50.0, -10.0), Point::new(50.0, 30.0), Point::new(-50.0, 30.0)];
        let r = compute_part_bounds(&part, &pins).unwrap();
        assert_rect(r, -50.0, -10.0, 50.0, 30.0);
    }

    #[test]
    fn part_without_pins_or_outline_has_no_bounds() {
        assert!(compute_part_bounds(&Part::default(), &[]).is_none());
    }

    #[test]
    fn rebuild_indices_uses_pad_aware_bounds() {
        let mut a = pin(0, 0.0, 0.0);
        a.width = Some(20.0);
        a.height = Some(60.0);
        a.shape = PadShape::Rect;
        let mut board = Board::new();
        board.parts.push(Part::default());
        board.pins.push(a);
        board.rebuild_indices();
        assert_rect(board.parts[0].bounding_box.unwrap(), -10.0, -30.0, 10.0, 30.0);
    }

    #[test]
    fn test_find_pin_at_strict_boundary() {
        let mut a = pin(0, 100.0, 100.0);
        a.width = Some(20.0);
        a.height = Some(40.0);
        a.shape = PadShape::Rect;
        a.side = Side::Top;

        let mut board = Board::new();
        board.parts.push(Part::default());
        board.pins.push(a);
        board.rebuild_indices();

        // Inside the pad rectangle: x in [90, 110], y in [80, 120]
        assert_eq!(board.find_pin_at(Point::new(100.0, 100.0), Side::Top, 1.0), Some(0));
        assert_eq!(board.find_pin_at(Point::new(109.0, 119.0), Side::Top, 1.0), Some(0));

        // In vicinity (near) but outside pad geometry
        assert_eq!(board.find_pin_at(Point::new(115.0, 100.0), Side::Top, 1.0), None);
        assert_eq!(board.find_pin_near(Point::new(115.0, 100.0), Side::Top, 20.0), Some(0));
    }

    #[test]
    fn test_synthesize_part_outline_for_rotated_component() {
        let mut a = pin(0, 525.601, 134.023);
        a.width = Some(38.976);
        a.height = Some(50.0);
        a.rotation_deg = 208.6;
        let mut b = pin(1, 584.364, 166.061);
        b.width = Some(38.976);
        b.height = Some(50.0);
        b.rotation_deg = 208.6;

        let part = Part {
            name: "C0456".into(),
            rotation_deg: 208.6,
            pin_indices: vec![0, 1],
            ..Default::default()
        };
        let pins = vec![a, b];
        let outline = synthesize_part_outline(&part, &pins);
        assert_eq!(outline.len(), 4);

        // Distance between outline[0] and outline[1] is length (~105.9 mils)
        let len = outline[0].dist(outline[1]);
        assert!((len - 105.906).abs() < 1e-2, "Expected length ~105.9, got {len}");

        // Distance between outline[1] and outline[2] is width (~50.0 mils)
        let width = outline[1].dist(outline[2]);
        assert!((width - 50.0).abs() < 1e-2, "Expected width ~50.0, got {width}");

        // Outline is rotated diagonally, so outline[0].x != outline[1].x and outline[0].y != outline[1].y
        assert!((outline[0].x - outline[1].x).abs() > 10.0);
        assert!((outline[0].y - outline[1].y).abs() > 10.0);
    }

    #[test]
    fn test_compute_pin_pitch_median_and_duplicate_filtering() {
        let p1 = pin(0, 0.0, 0.0);
        let p1_dup = pin(1, 0.05, 0.0); // Within 1% of 100 mil extent
        let p2 = pin(2, 50.0, 0.0);
        let p3 = pin(3, 100.0, 0.0);
        let pins = vec![p1, p1_dup, p2, p3];
        let pin_refs: Vec<&Pin> = pins.iter().collect();

        let pitch = compute_pin_pitch(&pin_refs).expect("valid pitch");
        // Distance from 0 to 50 is 50. Distance from 50 to 100 is 50. Median is 50.
        assert!((pitch - 50.0).abs() < 1e-3, "Expected pitch ~50, got {pitch}");
    }

    #[test]
    fn test_synthesize_part_outline_collinear_two_pin_passive_grows_thin_axis() {
        // Collinear 0805-style resistor with 80 mil pitch (2.03 mm) and sizeless pads (6 mil margin)
        let a = pin(0, -40.0, 0.0);
        let b = pin(1, 40.0, 0.0);
        let part = Part {
            name: "R101".into(),
            rotation_deg: 0.0,
            pin_indices: vec![0, 1],
            ..Default::default()
        };
        let pins = vec![a, b];
        let outline = synthesize_part_outline(&part, &pins);
        assert_eq!(outline.len(), 4);

        // Longitudinal length: 80 mil pitch + 2 * 6 mil margin = 92.0 mils
        let bbox = Rect::from_points(&outline).unwrap();
        let length = bbox.width();
        let width = bbox.height();

        assert!((length - 92.0).abs() < 1e-2, "Expected length ~92.0, got {length}");
        // Transverse width: previously only 12.0 mils (0.30 mm).
        // With 0.6 * pitch heuristic: 0.6 * 80.0 = 48.0 mils (~1.22 mm, matching 0805 body)
        assert!((width - 48.0).abs() < 1e-2, "Expected width ~48.0, got {width}");
    }

    #[test]
    fn test_synthesize_part_outline_infers_diagonal_rotation_for_two_pin_passive() {
        // Two pins at (0, 0) and (60, 60), no explicit rotation on part or pins
        let a = pin(0, 0.0, 0.0);
        let b = pin(1, 60.0, 60.0);
        let part = Part {
            name: "C102".into(),
            rotation_deg: 0.0,
            pin_indices: vec![0, 1],
            ..Default::default()
        };
        let pins = vec![a, b];
        let outline = synthesize_part_outline(&part, &pins);
        assert_eq!(outline.len(), 4);

        // Outline must be a rotated rectangle (corners are not axis-aligned)
        let edge1 = outline[0].dist(outline[1]);
        let edge2 = outline[1].dist(outline[2]);
        let (len, wid) = if edge1 > edge2 { (edge1, edge2) } else { (edge2, edge1) };

        // Pitch is sqrt(60^2 + 60^2) = ~84.85 mils
        // Axis-aligned square pad (12x12) rotated 45 degrees has diagonal half-extent 6*sqrt(2) on each side (~16.97 mils)
        // Length along 45° axis is pitch + 12*sqrt(2) = ~101.82 mils
        assert!((len - 101.82).abs() < 0.1, "Expected length ~101.82, got {len}");
        // Width is 0.6 * pitch = ~50.91 mils
        assert!((wid - 50.91).abs() < 0.1, "Expected width ~50.91, got {wid}");

        // Verify diagonal orientation: corners differ in both X and Y
        assert!((outline[0].x - outline[1].x).abs() > 5.0);
        assert!((outline[0].y - outline[1].y).abs() > 5.0);
    }
}
