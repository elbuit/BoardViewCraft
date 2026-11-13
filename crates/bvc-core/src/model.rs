//! Core data models for BoardViewCraft.

use std::collections::HashMap;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Side {
    #[default]
    Top,
    Bottom,
    Both,
}

impl Side {
    #[inline]
    pub fn is_top(self) -> bool {
        matches!(self, Side::Top | Side::Both)
    }

    #[inline]
    pub fn is_bottom(self) -> bool {
        matches!(self, Side::Bottom | Side::Both)
    }

    #[inline]
    pub fn visible_on(self, view_side: Side) -> bool {
        match view_side {
            Side::Top => self.is_top(),
            Side::Bottom => self.is_bottom(),
            Side::Both => true,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PartType {
    #[default]
    Smd,
    ThroughHole,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };

    #[inline]
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    #[inline]
    pub fn dist(self, other: Point) -> f64 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        (dx * dx + dy * dy).sqrt()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub min_x: f64,
    pub min_y: f64,
    pub max_x: f64,
    pub max_y: f64,
}

impl Rect {
    pub fn from_points<'a, I>(points: I) -> Option<Self>
    where
        I: IntoIterator<Item = &'a Point>,
    {
        let mut iter = points.into_iter();
        let first = *iter.next()?;
        let mut min_x = first.x;
        let mut max_x = first.x;
        let mut min_y = first.y;
        let mut max_y = first.y;

        for p in iter {
            if p.x < min_x { min_x = p.x; }
            if p.x > max_x { max_x = p.x; }
            if p.y < min_y { min_y = p.y; }
            if p.y > max_y { max_y = p.y; }
        }

        Some(Self { min_x, min_y, max_x, max_y })
    }

    pub fn expand(&self, margin: f64) -> Self {
        Self {
            min_x: self.min_x - margin,
            min_y: self.min_y - margin,
            max_x: self.max_x + margin,
            max_y: self.max_y + margin,
        }
    }

    #[inline]
    pub fn width(&self) -> f64 {
        (self.max_x - self.min_x).max(0.0)
    }

    #[inline]
    pub fn height(&self) -> f64 {
        (self.max_y - self.min_y).max(0.0)
    }

    #[inline]
    pub fn center(&self) -> Point {
        Point::new((self.min_x + self.max_x) * 0.5, (self.min_y + self.max_y) * 0.5)
    }
    #[inline]
    pub fn intersects(&self, other: &Rect) -> bool {
        self.min_x <= other.max_x
            && self.max_x >= other.min_x
            && self.min_y <= other.max_y
            && self.max_y >= other.min_y
    }

    #[inline]
    pub fn contains(&self, p: Point) -> bool {
        p.x >= self.min_x && p.x <= self.max_x && p.y >= self.min_y && p.y <= self.max_y
    }
}

/// Physical shape of a pad. Formats that do not describe pads leave `Round`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PadShape {
    #[default]
    Round,
    Rect,
    Square,
}

/// Represents a component / part on the board (e.g., U1, R10, C45).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Part {
    pub id: usize,
    pub name: String,
    pub mfg_code: String,
    pub side: Side,
    pub part_type: PartType,
    pub p1: Point,
    pub p2: Point,
    /// Indices of pins that belong to this part in the board's pin list.
    pub pin_indices: Vec<usize>,
    /// Precomputed bounding box enclosing the part's pins (with margin)
    pub bounding_box: Option<Rect>,
    /// Body outline in board coordinates (closed implicitly); empty when the format has none.
    pub outline: Vec<Point>,
    /// Placement rotation in degrees (counter-clockwise, board coordinates).
    pub rotation_deg: f64,
}

/// Represents a pin of a component.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Pin {
    pub id: usize,
    /// Component ID this pin belongs to.
    pub part_id: usize,
    /// Pin label/number (e.g., "1", "A1", "GND").
    pub name: String,
    pub pos: Point,
    pub side: Side,
    /// Name of the net connected to this pin.
    pub net_name: String,
    pub radius: f64,
    /// Pad shape; `Round` with no size means "draw a marker of `radius`".
    pub shape: PadShape,
    /// Pad width (local X, before rotation) when the format gives a physical size.
    pub width: Option<f64>,
    /// Pad height (local Y, before rotation) when the format gives a physical size.
    pub height: Option<f64>,
    /// Pad rotation in degrees (counter-clockwise, board coordinates); dimensions stay local.
    pub rotation_deg: f64,
}

impl Pin {
    /// Physical pad size `(width, height)` when the format provided one.
    #[inline]
    pub fn pad_size(&self) -> Option<(f64, f64)> {
        match (self.width, self.height) {
            (Some(w), Some(h)) if w > 0.0 && h > 0.0 => Some((w, h)),
            (Some(w), None) if w > 0.0 => Some((w, w)),
            (None, Some(h)) if h > 0.0 => Some((h, h)),
            _ => None,
        }
    }

    /// Axis-aligned half extents of the pad's physical size after rotation, if the size is known.
    /// Portions adapted from TRACE Boardviewer (MIT), Copyright (c) 2026 TRACE Boardviewer contributors
    /// (`padExtent` in `src/lib/formats/common.ts`).
    pub fn pad_half_extents(&self) -> Option<(f64, f64)> {
        let (w, h) = self.pad_size()?;
        if self.shape == PadShape::Round && w == h {
            return Some((w * 0.5, h * 0.5));
        }
        let (cos, sin) = quadrant(self.rotation_deg);
        let (cos, sin) = (cos.abs(), sin.abs());
        Some((w * 0.5 * cos + h * 0.5 * sin, w * 0.5 * sin + h * 0.5 * cos))
    }

    /// Tests whether `p` lies strictly inside this pad's geometry.
    /// If min_radius is provided (e.g. minimum visible pixel marker in world units),
    /// pads that render with a minimum visual radius respect that threshold.
    pub fn contains_point(&self, p: Point, min_radius: f64) -> bool {
        if let Some((w, h)) = self.pad_size() {
            if self.shape == PadShape::Round {
                let r = (w.min(h) * 0.5).max(min_radius);
                return self.pos.dist(p) <= r;
            }
            let half_w = (w * 0.5).max(min_radius);
            let half_h = (h * 0.5).max(min_radius);
            let dx = p.x - self.pos.x;
            let dy = p.y - self.pos.y;
            let (cos, sin) = quadrant(self.rotation_deg);
            let local_x = dx * cos + dy * sin;
            let local_y = -dx * sin + dy * cos;
            local_x.abs() <= half_w && local_y.abs() <= half_h
        } else {
            let r = self.radius.max(min_radius);
            self.pos.dist(p) <= r
        }
    }
}

/// Cosine and sine of `degrees`, exact for multiples of 90 degrees.
pub fn quadrant(degrees: f64) -> (f64, f64) {
    let turns = degrees / 90.0;
    if turns.fract() == 0.0 {
        let q = (turns as i64).rem_euclid(4) as usize;
        return ([1.0, 0.0, -1.0, 0.0][q], [0.0, 1.0, 0.0, -1.0][q]);
    }
    let a = degrees.to_radians();
    (a.cos(), a.sin())
}

/// Represents a via or test nail on the board.
#[derive(Debug, Clone, PartialEq)]
pub struct Via {
    pub id: usize,
    pub pos: Point,
    pub side: Side,
    pub net_name: String,
    pub radius: f64,
}

/// Represents an electrical net connecting multiple pins and vias.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Net {
    pub name: String,
    pub pin_indices: Vec<usize>,
    pub via_indices: Vec<usize>,
}

/// Compact O(1) bitset representation for net membership checking.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NetMembership {
    pin_bits: Vec<u64>,
    via_bits: Vec<u64>,
    pub total_nodes: usize,
}

impl NetMembership {
    #[inline]
    pub fn empty() -> Self {
        Self::default()
    }

    /// Precomputes a bitmask of pin and via indices belonging to the given net.
    pub fn from_net(pin_count: usize, via_count: usize, net: Option<&Net>) -> Self {
        let Some(net) = net else {
            return Self::empty();
        };

        let pin_words = (pin_count + 63) / 64;
        let mut pin_bits = vec![0u64; pin_words];
        for &idx in &net.pin_indices {
            if idx < pin_count {
                pin_bits[idx / 64] |= 1u64 << (idx % 64);
            }
        }

        let via_words = (via_count + 63) / 64;
        let mut via_bits = vec![0u64; via_words];
        for &idx in &net.via_indices {
            if idx < via_count {
                via_bits[idx / 64] |= 1u64 << (idx % 64);
            }
        }

        Self {
            pin_bits,
            via_bits,
            total_nodes: net.pin_indices.len() + net.via_indices.len(),
        }
    }

    /// Fast O(1) membership test for pins.
    #[inline]
    pub fn contains_pin(&self, pin_idx: usize) -> bool {
        let word = pin_idx / 64;
        if word < self.pin_bits.len() {
            (self.pin_bits[word] & (1u64 << (pin_idx % 64))) != 0
        } else {
            false
        }
    }

    /// Fast O(1) membership test for vias.
    #[inline]
    pub fn contains_via(&self, via_idx: usize) -> bool {
        let word = via_idx / 64;
        if word < self.via_bits.len() {
            (self.via_bits[word] & (1u64 << (via_idx % 64))) != 0
        } else {
            false
        }
    }

    /// Returns true if the net has > 250 elements (high density, e.g. GND/VCC).
    #[inline]
    pub fn is_high_density(&self) -> bool {
        self.total_nodes > 250
    }
}


/// Repair metadata carried by some board formats (part values, measured readings, voltages).
///
/// Entries are keyed by name rather than by part/pin index: names survive any re-ordering of
/// `Board::parts`/`Board::pins` and match how the source files reference them (e.g. XZZ
/// `=497=N1214(1)`). Keys are normalised to ASCII uppercase; lookups are case-insensitive.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BoardMetadata {
    part_values: HashMap<String, String>,
    pin_readings: HashMap<(String, String), String>,
    net_readings: HashMap<String, String>,
    net_voltages: HashMap<String, String>,
    net_signals: HashMap<String, String>,
}

fn meta_key(name: &str) -> String {
    name.trim().to_ascii_uppercase()
}

/// Inserts `value` under `key` when both are non-empty; returns whether it was stored.
fn meta_insert<K: std::hash::Hash + Eq>(map: &mut HashMap<K, String>, key: K, value: &str) -> bool {
    let value = value.trim();
    if value.is_empty() {
        return false;
    }
    map.insert(key, value.to_string());
    true
}

impl BoardMetadata {
    /// True when no metadata of any kind is present.
    pub fn is_empty(&self) -> bool {
        self.part_values.is_empty()
            && self.pin_readings.is_empty()
            && self.net_readings.is_empty()
            && self.net_voltages.is_empty()
            && self.net_signals.is_empty()
    }

    /// Component value (e.g. `560`, `R22`, `10uF`).
    pub fn set_part_value(&mut self, part: &str, value: &str) {
        let key = meta_key(part);
        if !key.is_empty() {
            meta_insert(&mut self.part_values, key, value);
        }
    }

    pub fn part_value(&self, part: &str) -> Option<&str> {
        self.part_values.get(&meta_key(part)).map(String::as_str)
    }

    /// Measured diode/resistance reading for one pin of a part.
    pub fn set_pin_reading(&mut self, part: &str, pin: &str, value: &str) {
        let key = (meta_key(part), meta_key(pin));
        if !key.0.is_empty() && !key.1.is_empty() {
            meta_insert(&mut self.pin_readings, key, value);
        }
    }

    pub fn pin_reading(&self, part: &str, pin: &str) -> Option<&str> {
        self.pin_readings.get(&(meta_key(part), meta_key(pin))).map(String::as_str)
    }

    /// Measured diode/resistance reading of a net.
    pub fn set_net_reading(&mut self, net: &str, value: &str) {
        let key = meta_key(net);
        if !key.is_empty() {
            meta_insert(&mut self.net_readings, key, value);
        }
    }

    pub fn net_reading(&self, net: &str) -> Option<&str> {
        self.net_readings.get(&meta_key(net)).map(String::as_str)
    }

    /// Expected voltage of a net.
    pub fn set_net_voltage(&mut self, net: &str, value: &str) {
        let key = meta_key(net);
        if !key.is_empty() {
            meta_insert(&mut self.net_voltages, key, value);
        }
    }

    pub fn net_voltage(&self, net: &str) -> Option<&str> {
        self.net_voltages.get(&meta_key(net)).map(String::as_str)
    }

    /// Free-text signal description of a net.
    pub fn set_net_signal(&mut self, net: &str, value: &str) {
        let key = meta_key(net);
        if !key.is_empty() {
            meta_insert(&mut self.net_signals, key, value);
        }
    }

    pub fn net_signal(&self, net: &str) -> Option<&str> {
        self.net_signals.get(&meta_key(net)).map(String::as_str)
    }

    pub fn part_value_count(&self) -> usize {
        self.part_values.len()
    }

    pub fn pin_reading_count(&self) -> usize {
        self.pin_readings.len()
    }
}

#[cfg(test)]
mod metadata_tests {
    use super::BoardMetadata;

    #[test]
    fn lookups_are_case_insensitive_and_skip_empty_values() {
        let mut meta = BoardMetadata::default();
        assert!(meta.is_empty());
        meta.set_part_value("r10", "10k");
        meta.set_part_value("R11", "  ");
        meta.set_pin_reading("u1", "a1", "0.512");
        meta.set_net_voltage("pp3v3", "3.3V");
        assert_eq!(meta.part_value("R10"), Some("10k"));
        assert_eq!(meta.part_value("R11"), None);
        assert_eq!(meta.pin_reading("U1", "A1"), Some("0.512"));
        assert_eq!(meta.net_voltage("PP3V3"), Some("3.3V"));
        assert!(!meta.is_empty());
    }
}
