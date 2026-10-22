//! Core data models for BoardViewCraft.

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Side {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartType {
    Smd,
    ThroughHole,
}

#[derive(Debug, Clone, Copy, PartialEq)]
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
}

/// Represents a component / part on the board (e.g., U1, R10, C45).
#[derive(Debug, Clone, PartialEq)]
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
}

/// Represents a pin of a component.
#[derive(Debug, Clone, PartialEq)]
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

