//! Core data models for BoardViewCraft.

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

