//! Board container and indexing structures for BoardViewCraft.

use std::collections::BTreeMap;
use crate::model::{Net, NetMembership, Part, Pin, Point, Rect, Side, Via};
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

        // Precompute part bounding boxes if not already defined by the file format (e.g. TVW bx1/by1/bx2/by2)
        const PART_MARGIN: f64 = 6.0;
        for part in &mut self.parts {
            if part.bounding_box.is_some() {
                continue;
            }
            if part.pin_indices.is_empty() {
                part.bounding_box = None;
                continue;
            }
            let mut pmin_x = f64::INFINITY;
            let mut pmax_x = f64::NEG_INFINITY;
            let mut pmin_y = f64::INFINITY;
            let mut pmax_y = f64::NEG_INFINITY;

            for &p_idx in &part.pin_indices {
                if let Some(pin) = self.pins.get(p_idx) {
                    if pin.pos.x < pmin_x { pmin_x = pin.pos.x; }
                    if pin.pos.x > pmax_x { pmax_x = pin.pos.x; }
                    if pin.pos.y < pmin_y { pmin_y = pin.pos.y; }
                    if pin.pos.y > pmax_y { pmax_y = pin.pos.y; }
                }
            }

            if pmin_x.is_finite() {
                part.bounding_box = Some(Rect {
                    min_x: pmin_x - PART_MARGIN,
                    max_x: pmax_x + PART_MARGIN,
                    min_y: pmin_y - PART_MARGIN,
                    max_y: pmax_y + PART_MARGIN,
                });
            } else {
                part.bounding_box = None;
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
