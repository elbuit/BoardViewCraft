//! GenCAD 1.4 file format parser.
//!
//! Faithful implementation of OpenBoardView's `GenCADFile.cpp` logic:
//! - $HEADER: Reads UNITS (INCH, THOU, MM, MM100, USER, USERCM, USERMM).
//! - $BOARD: Reads LINE, ARC, RECTANGLE outlines into outline segments / polygon.
//! - $PADS: Reads pad definitions (kind and LINE/ARC/CIRCLE/RECTANGLE geometry) for physical pad sizes.
//! - $PADSTACKS: Reads drill sizes (drill_size > 0 => ThroughHole, else SMD), layer sides (TOP, BOTTOM, ALL)
//!   and the pads of each stack with their rotation and mirror.
//! - $SHAPES: Reads pin definitions (pin_name, pad_name, x, y, layer, rot, mirror) and body artwork.
//! - $COMPONENTS: Reads PLACE, LAYER, ROTATION, SHAPE (with MIRRORX, MIRRORY, FLIP). Computes transformed pin coordinates.
//! - $SIGNALS: Reads SIGNAL name and NODE component/pin bindings to map nets to pins.
//! - $ROUTES: Reads ROUTE net name and VIA definitions to populate board vias.
//!
//! Pad sizes/rotations, padstack resolution and component outlines follow TRACE Boardviewer.
//! Portions adapted from TRACE Boardviewer (MIT), Copyright (c) 2026 TRACE Boardviewer contributors
//! (`src/lib/gencad.ts`).

use std::collections::{BTreeMap, BTreeSet};
use std::f64::consts::PI;
use crate::board::Board;
use crate::model::{PadShape, Part, PartType, Pin, Point, Rect, Side, Via};
use crate::parser::gencad_geometry::{outer_contour, parse_geometry, rectangle, transform, GeomPath, Mirror, PathKind};
use crate::parser::{FormatParser, ParseError};

/// Half size (mils) of the body area reserved around a pad of unknown size (trace: 0.18 mm).
const MARKER_RADIUS: f64 = 0.18 * 1000.0 / 25.4;
/// Margin (mils) added around a body without artwork before using its bounds as outline (trace: 0.2 mm).
const FALLBACK_BODY_MARGIN: f64 = 0.2 * 1000.0 / 25.4;
/// Marker radius (mils) of a pin whose pad has no known size.
const SIZELESS_PIN_RADIUS: f64 = 5.0;

/// `$PADS` entry: pad kind and its local geometry.
#[derive(Debug, Clone)]
struct PadDef {
    kind: String,
    paths: Vec<GeomPath>,
}

/// Size information of a pad definition, independent from the pin that uses it.
#[derive(Debug, Clone, Copy)]
struct PadExtent {
    local_bounds: Option<Rect>,
    round: bool,
    exact_rectangle: bool,
}

impl PadExtent {
    fn of(pad: Option<&PadDef>) -> Self {
        let local_bounds = pad.and_then(|p| Rect::from_points(p.paths.iter().flat_map(|path| path.points.iter())));
        let single = |kind: PathKind| pad.is_some_and(|p| p.paths.len() == 1 && p.paths[0].kind == kind);
        Self {
            local_bounds,
            round: pad.is_some_and(|p| p.kind == "ROUND") && single(PathKind::Circle),
            exact_rectangle: pad.is_some_and(|p| p.kind == "RECTANGULAR" || p.kind == "SQUARE")
                && single(PathKind::Rectangle),
        }
    }
}

/// `PAD` row of a `$PADSTACKS` entry.
#[derive(Debug, Clone)]
struct StackPad {
    name: String,
    layer: String,
    rotation: f64,
    mirror: Mirror,
}

fn outer_layer(layer: &str) -> bool {
    matches!(layer, "TOP" | "BOTTOM" | "ALL" | "BOTH")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GenCadDimension {
    Inch,
    Thou,
    Mm,
    Mm100,
    User,
    UserCm,
    UserMm,
    Invalid,
}

#[derive(Debug, Clone)]
struct PadStackInfo {
    is_drilled: bool,
    side: Side,
    pads: Vec<StackPad>,
}

#[derive(Debug, Clone)]
struct ShapePin {
    name: String,
    pad_name: String,
    pos: Point,
    layer: String,
    rotation: f64,
    mirror: Mirror,
}

#[derive(Debug, Clone, Default)]
struct ShapeDef {
    pins: Vec<ShapePin>,
    /// Body artwork (LINE/ARC/CIRCLE/RECTANGLE rows of the shape).
    paths: Vec<GeomPath>,
}

#[derive(Debug, Clone)]
struct ComponentDef {
    name: String,
    device: String,
    place: Point,
    layer: Side,
    rotation: f64,
    shape_name: String,
    mirror_x: bool,
    mirror_y: bool,
    flip: bool,
}

/// Physical pad of one placed pin, in board coordinates.
#[derive(Debug, Clone, Copy)]
struct PlacedPad {
    /// Axis-aligned bounds of the placed pad (its centre when the size is unknown).
    bounds: Rect,
    /// Local `(width, height)` for exact rectangles, else the placed axis-aligned size; `None` if unknown.
    size: Option<(f64, f64)>,
    rotation_deg: f64,
    shape: PadShape,
    radius: f64,
}

/// Snaps angles that are a hair away from a whole degree (trigonometric noise) to it.
fn snap_degrees(deg: f64) -> f64 {
    let d = deg.rem_euclid(360.0);
    let r = d.round();
    let r = if (d - r).abs() < 1e-7 { r } else { d };
    if r >= 360.0 { r - 360.0 } else { r }
}

/// Resolves the pad of a shape pin through its padstack and places it on the board.
///
/// Portions adapted from TRACE Boardviewer (MIT), Copyright (c) 2026 TRACE Boardviewer contributors
/// (pad resolution in `parseGenCad`, `src/lib/gencad.ts`).
fn resolve_pad(
    spin: &ShapePin,
    stack: Option<&PadStackInfo>,
    pad_defs: &BTreeMap<String, PadDef>,
    pad_extents: &BTreeMap<&str, PadExtent>,
    comp_layer: Side,
    flip: bool,
    placed: &dyn Fn(Point) -> Point,
) -> PlacedPad {
    // FLIP changes padstack layer selection, independently from coordinate mirroring.
    // A BOTTOM PIN reverses its stack orientation relative to a TOP PIN.
    let reversed_pin = spin.layer == "BOTTOM";
    let target_layer = if ((comp_layer == Side::Bottom) != flip) != reversed_pin { "BOTTOM" } else { "TOP" };
    let outer: Vec<&StackPad> = stack.map(|s| s.pads.iter().filter(|p| outer_layer(&p.layer)).collect()).unwrap_or_default();
    let stack_pad = outer
        .iter()
        .find(|p| p.layer == target_layer)
        .or_else(|| outer.iter().find(|p| p.layer == "ALL" || p.layer == "BOTH"))
        .or_else(|| outer.first())
        .copied();
    let pad_name = stack_pad.map_or(spin.pad_name.as_str(), |p| p.name.as_str());
    let extent = pad_extents
        .get(pad_name)
        .copied()
        .unwrap_or_else(|| PadExtent::of(pad_defs.get(pad_name)));

    let (stack_rotation, stack_mirror) = stack_pad.map_or((0.0, Mirror::None), |p| (p.rotation, p.mirror));
    let pad_placed = |p: Point| {
        let in_stack = transform(p, stack_rotation, stack_mirror, Point::ZERO);
        placed(transform(in_stack, spin.rotation, spin.mirror, spin.pos))
    };

    let Some(lb) = extent.local_bounds.filter(|b| b.width() > 0.0 && b.height() > 0.0) else {
        let center = pad_placed(extent.local_bounds.map_or(Point::ZERO, |b| b.center()));
        return PlacedPad {
            bounds: Rect { min_x: center.x, min_y: center.y, max_x: center.x, max_y: center.y },
            size: None,
            rotation_deg: 0.0,
            shape: PadShape::Round,
            radius: SIZELESS_PIN_RADIUS,
        };
    };
    let center = pad_placed(lb.center());
    let bounds = if extent.round {
        Rect { min_x: center.x, min_y: center.y, max_x: center.x, max_y: center.y }.expand(lb.width() / 2.0)
    } else {
        Rect::from_points(&rectangle(&lb).into_iter().map(pad_placed).collect::<Vec<_>>()).expect("rectangle has points")
    };
    // Exact rectangles keep their source dimensions and carry the placed angle of their local X axis.
    let (width, height, rotation_deg) = if extent.exact_rectangle {
        let a = pad_placed(Point::ZERO);
        let b = pad_placed(Point::new(1.0, 0.0));
        (lb.width(), lb.height(), snap_degrees((b.y - a.y).atan2(b.x - a.x).to_degrees()))
    } else {
        (bounds.width(), bounds.height(), 0.0)
    };
    let shape = if extent.round {
        PadShape::Round
    } else if (width - height).abs() < 1e-6 {
        PadShape::Square
    } else {
        PadShape::Rect
    };
    let radius = if extent.round { lb.width() / 2.0 } else { width.min(height) / 2.0 };
    PlacedPad { bounds, size: Some((width, height)), rotation_deg, shape, radius }
}

pub struct GenCadParser;

impl GenCadParser {
    pub const fn new() -> Self {
        Self
    }

    /// Converts raw board coordinate to board unit (in mils).
    #[inline]
    fn board_unit_to_brd_coordinate(val: f64, dimension: GenCadDimension, dimension_unit: f64) -> f64 {
        match dimension {
            GenCadDimension::Inch => val * 1000.0,
            GenCadDimension::Invalid | GenCadDimension::Thou => val,
            GenCadDimension::Mm => val * (100.0 / 2.54),
            GenCadDimension::Mm100 => val * (1.0 / 2.54),
            GenCadDimension::User => (dimension_unit * val) / 1000.0,
            GenCadDimension::UserCm => dimension_unit * val * (10.0 / 2.54),
            GenCadDimension::UserMm => dimension_unit * val * (1.0 / 2.54),
        }
    }

    /// Discretizes an arc into line segments matching OpenBoardView `arc_to_segments`.
    fn arc_to_segments(
        p1: Point,
        p2: Point,
        pc: Point,
        arc_slice_angle_rad: f64,
    ) -> Vec<(Point, Point)> {
        let mut segments = Vec::new();
        let r = p1.dist(pc);
        let start_angle = (p1.y - pc.y).atan2(p1.x - pc.x);
        let mut end_angle = (p2.y - pc.y).atan2(p2.x - pc.x);
        if end_angle < start_angle {
            end_angle += 2.0 * PI;
        }

        let mut p_old = p1;
        let mut angle = start_angle + arc_slice_angle_rad;
        while angle < end_angle {
            let p_new = Point::new(pc.x + r * angle.cos(), pc.y + r * angle.sin());
            segments.push((p_old, p_new));
            p_old = p_new;
            angle += arc_slice_angle_rad;
        }
        segments.push((p_old, p2));
        segments
    }
}

impl Default for GenCadParser {
    fn default() -> Self {
        Self::new()
    }
}

impl FormatParser for GenCadParser {
    fn name(&self) -> &'static str {
        "GenCAD"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["cad"]
    }

    fn verify_format(&self, data: &[u8]) -> bool {
        // GenCAD files start with or contain "GENCAD" and "$HEADER"
        let s = String::from_utf8_lossy(data);
        s.contains("GENCAD") && s.contains("$HEADER")
    }

    fn parse(&self, data: &[u8]) -> Result<Board, ParseError> {
        let content = String::from_utf8_lossy(data);
        let mut board = Board::new();

        let mut dimension = GenCadDimension::Thou;
        let mut dimension_unit = 1000.0;

        let mut pad_defs: BTreeMap<String, PadDef> = BTreeMap::new();
        let mut current_pad_name: Option<String> = None;
        let mut padstacks: BTreeMap<String, PadStackInfo> = BTreeMap::new();
        let mut shapes: BTreeMap<String, ShapeDef> = BTreeMap::new();
        let mut components: Vec<ComponentDef> = Vec::new();
        let mut signals_cache: BTreeMap<(String, String), String> = BTreeMap::new(); // (comp_name, pin_name) -> net_name
        let mut outline_segments: Vec<(Point, Point)> = Vec::new();

        let mut current_section = "";
        let mut current_shape_name: Option<String> = None;
        let mut current_padstack_name: Option<String> = None;
        let mut current_padstack_info: Option<PadStackInfo> = None;
        let mut current_signal_name: Option<String> = None;
        let mut current_route_net: Option<String> = None;

        for (_line_idx, raw_line) in content.lines().enumerate() {
            let line = raw_line.trim();
            if line.is_empty() {
                continue;
            }

            // Section markers
            if line.starts_with('$') {
                let sec_upper = line.to_ascii_uppercase();
                match sec_upper.as_str() {
                    "$HEADER" => { current_section = "HEADER"; }
                    "$ENDHEADER" => { current_section = ""; }
                    "$BOARD" => { current_section = "BOARD"; }
                    "$ENDBOARD" => { current_section = ""; }
                    "$PADS" => { current_section = "PADS"; }
                    "$ENDPADS" => { current_section = ""; }
                    "$PADSTACKS" => { current_section = "PADSTACKS"; }
                    "$ENDPADSTACKS" => {
                        if let (Some(name), Some(info)) = (current_padstack_name.take(), current_padstack_info.take()) {
                            padstacks.insert(name, info);
                        }
                        current_section = "";
                    }
                    "$SHAPES" => { current_section = "SHAPES"; }
                    "$ENDSHAPES" => {
                        current_shape_name = None;
                        current_section = "";
                    }
                    "$COMPONENTS" => { current_section = "COMPONENTS"; }
                    "$ENDCOMPONENTS" => { current_section = ""; }
                    "$SIGNALS" => { current_section = "SIGNALS"; }
                    "$ENDSIGNALS" => {
                        current_signal_name = None;
                        current_section = "";
                    }
                    "$ROUTES" => { current_section = "ROUTES"; }
                    "$ENDROUTES" => {
                        current_route_net = None;
                        current_section = "";
                    }
                    _ => {
                        if sec_upper.starts_with("$END") {
                            current_section = "";
                        } else {
                            current_section = "OTHER";
                        }
                    }
                }
                continue;
            }

            match current_section {
                "HEADER" => {
                    let tokens: Vec<&str> = line.split_whitespace().collect();
                    if !tokens.is_empty() && tokens[0].eq_ignore_ascii_case("UNITS") && tokens.len() >= 2 {
                        let unit_type = tokens[1].to_ascii_uppercase();
                        match unit_type.as_str() {
                            "INCH" => dimension = GenCadDimension::Inch,
                            "THOU" => dimension = GenCadDimension::Thou,
                            "MM" => dimension = GenCadDimension::Mm,
                            "MM100" => dimension = GenCadDimension::Mm100,
                            "USER" => {
                                dimension = GenCadDimension::User;
                                if tokens.len() >= 3 {
                                    dimension_unit = tokens[2].parse::<f64>().unwrap_or(1000.0);
                                }
                            }
                            "USERCM" | "USERM" => {
                                dimension = GenCadDimension::UserCm;
                                if tokens.len() >= 3 {
                                    dimension_unit = tokens[2].parse::<f64>().unwrap_or(1.0);
                                }
                            }
                            "USERMM" => {
                                dimension = GenCadDimension::UserMm;
                                if tokens.len() >= 3 {
                                    dimension_unit = tokens[2].parse::<f64>().unwrap_or(1.0);
                                }
                            }
                            _ => {}
                        }
                    }
                }

                "BOARD" => {
                    let tokens: Vec<&str> = line.split_whitespace().collect();
                    if tokens.is_empty() {
                        continue;
                    }
                    let cmd = tokens[0].to_ascii_uppercase();
                    match cmd.as_str() {
                        "LINE" => {
                            // LINE x1 y1 x2 y2
                            if tokens.len() >= 5 {
                                if let (Ok(x1), Ok(y1), Ok(x2), Ok(y2)) = (
                                    tokens[1].parse::<f64>(),
                                    tokens[2].parse::<f64>(),
                                    tokens[3].parse::<f64>(),
                                    tokens[4].parse::<f64>(),
                                ) {
                                    let p1 = Point::new(
                                        Self::board_unit_to_brd_coordinate(x1, dimension, dimension_unit),
                                        Self::board_unit_to_brd_coordinate(y1, dimension, dimension_unit),
                                    );
                                    let p2 = Point::new(
                                        Self::board_unit_to_brd_coordinate(x2, dimension, dimension_unit),
                                        Self::board_unit_to_brd_coordinate(y2, dimension, dimension_unit),
                                    );
                                    outline_segments.push((p1, p2));
                                }
                            }
                        }
                        "RECTANGLE" => {
                            // RECTANGLE x y width height
                            if tokens.len() >= 5 {
                                if let (Ok(x), Ok(y), Ok(w), Ok(h)) = (
                                    tokens[1].parse::<f64>(),
                                    tokens[2].parse::<f64>(),
                                    tokens[3].parse::<f64>(),
                                    tokens[4].parse::<f64>(),
                                ) {
                                    let px = Self::board_unit_to_brd_coordinate(x, dimension, dimension_unit);
                                    let py = Self::board_unit_to_brd_coordinate(y, dimension, dimension_unit);
                                    let pw = Self::board_unit_to_brd_coordinate(w, dimension, dimension_unit);
                                    let ph = Self::board_unit_to_brd_coordinate(h, dimension, dimension_unit);

                                    let p1 = Point::new(px, py);
                                    let p2 = Point::new(px, py + ph);
                                    let p3 = Point::new(px + pw, py + ph);
                                    let p4 = Point::new(px + pw, py);

                                    outline_segments.push((p1, p2));
                                    outline_segments.push((p2, p3));
                                    outline_segments.push((p3, p4));
                                    outline_segments.push((p4, p1));
                                }
                            }
                        }
                        "ARC" => {
                            // ARC x_start y_start x_stop y_stop x_center y_center
                            if tokens.len() >= 7 {
                                if let (Ok(x1), Ok(y1), Ok(x2), Ok(y2), Ok(xc), Ok(yc)) = (
                                    tokens[1].parse::<f64>(),
                                    tokens[2].parse::<f64>(),
                                    tokens[3].parse::<f64>(),
                                    tokens[4].parse::<f64>(),
                                    tokens[5].parse::<f64>(),
                                    tokens[6].parse::<f64>(),
                                ) {
                                    let p1 = Point::new(
                                        Self::board_unit_to_brd_coordinate(x1, dimension, dimension_unit),
                                        Self::board_unit_to_brd_coordinate(y1, dimension, dimension_unit),
                                    );
                                    let p2 = Point::new(
                                        Self::board_unit_to_brd_coordinate(x2, dimension, dimension_unit),
                                        Self::board_unit_to_brd_coordinate(y2, dimension, dimension_unit),
                                    );
                                    let pc = Point::new(
                                        Self::board_unit_to_brd_coordinate(xc, dimension, dimension_unit),
                                        Self::board_unit_to_brd_coordinate(yc, dimension, dimension_unit),
                                    );

                                    let arc_segs = Self::arc_to_segments(p1, p2, pc, 0.1);
                                    outline_segments.extend(arc_segs);
                                }
                            }
                        }
                        _ => {}
                    }
                }

                "PADS" => {
                    let tokens: Vec<&str> = line.split_whitespace().collect();
                    let Some(cmd) = tokens.first().map(|t| t.to_ascii_uppercase()) else { continue };
                    if cmd == "PAD" {
                        // PAD <pad_name> <kind> <aperture>; the first definition of a name wins.
                        current_pad_name = None;
                        if tokens.len() >= 3 {
                            let name = tokens[1].trim_matches('"').to_string();
                            if !pad_defs.contains_key(&name) {
                                pad_defs.insert(name.clone(), PadDef { kind: tokens[2].to_ascii_uppercase(), paths: Vec::new() });
                                current_pad_name = Some(name);
                            }
                        }
                    } else if let Some(def) = current_pad_name.as_ref().and_then(|n| pad_defs.get_mut(n))
                        && let Some(path) = parse_geometry(&tokens, |v| Self::board_unit_to_brd_coordinate(v, dimension, dimension_unit))
                    {
                        def.paths.push(path);
                    }
                }

                "PADSTACKS" => {
                    let tokens: Vec<&str> = line.split_whitespace().collect();
                    if tokens.is_empty() {
                        continue;
                    }
                    let cmd = tokens[0].to_ascii_uppercase();
                    if cmd == "PADSTACK" {
                        if let (Some(name), Some(info)) = (current_padstack_name.take(), current_padstack_info.take()) {
                            padstacks.insert(name, info);
                        }
                        if tokens.len() >= 3 {
                            let pad_name = tokens[1].trim_matches('"').to_string();
                            let drill_size = tokens[2].parse::<f64>().unwrap_or(0.0);
                            let is_drilled = drill_size != 0.0;
                            current_padstack_name = Some(pad_name);
                            current_padstack_info = Some(PadStackInfo {
                                is_drilled,
                                side: Side::Both,
                                pads: Vec::new(),
                            });
                        }
                    } else if cmd == "PAD" {
                        // PAD <pad_name> <layer> <rot> <mirror>
                        if tokens.len() >= 3 {
                            let layer_str = tokens[2].to_ascii_uppercase();
                            if let Some(ref mut info) = current_padstack_info {
                                info.pads.push(StackPad {
                                    name: tokens[1].trim_matches('"').to_string(),
                                    layer: layer_str.clone(),
                                    rotation: tokens.get(3).and_then(|t| t.parse::<f64>().ok()).unwrap_or(0.0),
                                    mirror: Mirror::parse(tokens.get(4).copied()),
                                });
                                if layer_str == "TOP" {
                                    info.side = if info.side == Side::Bottom { Side::Both } else { Side::Top };
                                } else if layer_str == "BOTTOM" {
                                    info.side = if info.side == Side::Top { Side::Both } else { Side::Bottom };
                                } else if layer_str == "ALL" {
                                    info.side = Side::Both;
                                }
                            }
                        }
                    }
                }

                "SHAPES" => {
                    let tokens: Vec<&str> = line.split_whitespace().collect();
                    if tokens.is_empty() {
                        continue;
                    }
                    let cmd = tokens[0].to_ascii_uppercase();
                    if cmd == "SHAPE" {
                        if tokens.len() >= 2 {
                            let sname = tokens[1].trim_matches('"').to_string();
                            current_shape_name = Some(sname.clone());
                            shapes.entry(sname).or_default();
                        }
                    } else if cmd == "PIN" {
                        // PIN <pin_name> <pad_name> <x> <y> <layer> <rot> <mirror>
                        if let Some(ref sname) = current_shape_name {
                            if tokens.len() >= 5 {
                                let pin_name = tokens[1].trim_matches('"').to_string();
                                let pad_name = tokens[2].trim_matches('"').to_string();
                                if let (Ok(x), Ok(y)) = (tokens[3].parse::<f64>(), tokens[4].parse::<f64>()) {
                                    let layer = if tokens.len() >= 6 { tokens[5].to_ascii_uppercase() } else { "TOP".to_string() };
                                    let rot = if tokens.len() >= 7 { tokens[6].parse::<f64>().unwrap_or(0.0) } else { 0.0 };
                                    let mirror = Mirror::parse(tokens.get(7).copied());

                                    let px = Self::board_unit_to_brd_coordinate(x, dimension, dimension_unit);
                                    let py = Self::board_unit_to_brd_coordinate(y, dimension, dimension_unit);

                                    if let Some(shape_def) = shapes.get_mut(sname) {
                                        shape_def.pins.push(ShapePin {
                                            name: pin_name,
                                            pad_name,
                                            pos: Point::new(px, py),
                                            layer,
                                            rotation: rot,
                                            mirror,
                                        });
                                    }
                                }
                            }
                        }
                    } else if let Some(shape_def) = current_shape_name.as_ref().and_then(|n| shapes.get_mut(n))
                        && let Some(path) = parse_geometry(&tokens, |v| Self::board_unit_to_brd_coordinate(v, dimension, dimension_unit))
                    {
                        shape_def.paths.push(path);
                    }
                }

                "COMPONENTS" => {
                    let tokens: Vec<&str> = line.split_whitespace().collect();
                    if tokens.is_empty() {
                        continue;
                    }
                    let cmd = tokens[0].to_ascii_uppercase();
                    match cmd.as_str() {
                        "COMPONENT" => {
                            if tokens.len() >= 2 {
                                let cname = tokens[1].trim_matches('"').to_string();
                                components.push(ComponentDef {
                                    name: cname,
                                    device: String::new(),
                                    place: Point::ZERO,
                                    layer: Side::Top,
                                    rotation: 0.0,
                                    shape_name: String::new(),
                                    mirror_x: false,
                                    mirror_y: false,
                                    flip: false,
                                });
                            }
                        }
                        "DEVICE" => {
                            if let Some(comp) = components.last_mut() {
                                if tokens.len() >= 2 {
                                    let mut dev_str = line[tokens[0].len()..].trim().trim_matches('"').to_string();
                                    dev_str = dev_str.replace(' ', "_");
                                    comp.device = dev_str;
                                }
                            }
                        }
                        "PLACE" => {
                            if let Some(comp) = components.last_mut() {
                                if tokens.len() >= 3 {
                                    if let (Ok(x), Ok(y)) = (tokens[1].parse::<f64>(), tokens[2].parse::<f64>()) {
                                        let px = Self::board_unit_to_brd_coordinate(x, dimension, dimension_unit);
                                        let py = Self::board_unit_to_brd_coordinate(y, dimension, dimension_unit);
                                        comp.place = Point::new(px, py);
                                    }
                                }
                            }
                        }
                        "LAYER" => {
                            if let Some(comp) = components.last_mut() {
                                if tokens.len() >= 2 {
                                    let l = tokens[1].to_ascii_uppercase();
                                    comp.layer = if l == "BOTTOM" { Side::Bottom } else { Side::Top };
                                }
                            }
                        }
                        "ROTATION" => {
                            if let Some(comp) = components.last_mut() {
                                if tokens.len() >= 2 {
                                    comp.rotation = tokens[1].parse::<f64>().unwrap_or(0.0);
                                }
                            }
                        }
                        "SHAPE" => {
                            // SHAPE <shape_name> [MIRRORX|MIRRORY|0] [FLIP|0]
                            if let Some(comp) = components.last_mut() {
                                if tokens.len() >= 2 {
                                    comp.shape_name = tokens[1].trim_matches('"').to_string();
                                    for t in &tokens[2..] {
                                        let tu = t.to_ascii_uppercase();
                                        if tu == "MIRRORX" {
                                            comp.mirror_x = true;
                                        } else if tu == "MIRRORY" {
                                            comp.mirror_y = true;
                                        } else if tu == "MIRRORXY" {
                                            comp.mirror_x = true;
                                            comp.mirror_y = true;
                                        } else if tu == "FLIP" {
                                            comp.flip = true;
                                        }
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }

                "SIGNALS" => {
                    let tokens: Vec<&str> = line.split_whitespace().collect();
                    if tokens.is_empty() {
                        continue;
                    }
                    let cmd = tokens[0].to_ascii_uppercase();
                    if cmd == "SIGNAL" {
                        if tokens.len() >= 2 {
                            let signame = line[tokens[0].len()..].trim().trim_matches('"').to_string();
                            current_signal_name = Some(signame);
                        }
                    } else if cmd == "NODE" {
                        // NODE <comp_name> <pin_name>
                        if let Some(ref signame) = current_signal_name {
                            if tokens.len() >= 3 {
                                let cname = tokens[1].trim_matches('"').to_string();
                                let pname = tokens[2].trim_matches('"').to_string();
                                signals_cache.insert((cname, pname), signame.clone());
                            }
                        }
                    }
                }

                "ROUTES" => {
                    let tokens: Vec<&str> = line.split_whitespace().collect();
                    if tokens.is_empty() {
                        continue;
                    }
                    let cmd = tokens[0].to_ascii_uppercase();
                    if cmd == "ROUTE" {
                        if tokens.len() >= 2 {
                            let rnet = line[tokens[0].len()..].trim().trim_matches('"').to_string();
                            current_route_net = Some(rnet);
                        }
                    } else if cmd == "VIA" {
                        // VIA <pad_name> <x> <y> <layer> <drill_size> <via_name>
                        if tokens.len() >= 4 {
                            if let (Ok(x), Ok(y)) = (tokens[2].parse::<f64>(), tokens[3].parse::<f64>()) {
                                let px = Self::board_unit_to_brd_coordinate(x, dimension, dimension_unit);
                                let py = Self::board_unit_to_brd_coordinate(y, dimension, dimension_unit);

                                let net_name = current_route_net.clone().unwrap_or_else(|| "GND".to_string());
                                let via_id = board.vias.len();
                                board.vias.push(Via {
                                    id: via_id,
                                    pos: Point::new(px, py),
                                    side: Side::Both,
                                    net_name,
                                    radius: 8.0,
                                });
                            }
                        }
                    }
                }

                _ => {}
            }
        }

        // Close last padstack if still open
        if let (Some(name), Some(info)) = (current_padstack_name.take(), current_padstack_info.take()) {
            padstacks.insert(name, info);
        }

        // Now process components and their shape pins to create Part and Pin instances.
        let pad_extents: BTreeMap<&str, PadExtent> =
            pad_defs.iter().map(|(name, def)| (name.as_str(), PadExtent::of(Some(def)))).collect();
        let mut parsed_shapes: BTreeSet<(Side, i64, i64, String)> = BTreeSet::new();
        let mut nc_counter = 0usize;

        for comp in components {
            if comp.shape_name.is_empty() {
                continue;
            }

            // Duplicate component check matching OpenBoardView:
            // PositionedNamedShape duplicate_check_id{brd_part.mounting_side, brd_part.p1.x, brd_part.p1.y, shape_name_str};
            let check_id = (
                comp.layer,
                comp.place.x.round() as i64,
                comp.place.y.round() as i64,
                comp.shape_name.clone(),
            );
            if !parsed_shapes.insert(check_id) {
                continue;
            }

            let shape_opt = shapes.get(&comp.shape_name);

            // Determine if SMD or ThroughHole
            let mut is_smd = true;
            if let Some(shape) = shape_opt {
                for spin in &shape.pins {
                    if let Some(ps_info) = padstacks.get(&spin.pad_name) {
                        if ps_info.is_drilled {
                            is_smd = false;
                            break;
                        }
                    }
                }
            }

            let part_type = if is_smd { PartType::Smd } else { PartType::ThroughHole };
            let mounting_side = if part_type == PartType::ThroughHole { Side::Both } else { comp.layer };

            let part_id = board.parts.len();
            let mut mfg_code = comp.device.clone();
            if !mfg_code.is_empty() {
                mfg_code.push_str(" SHAPE ");
            }
            mfg_code.push_str(&comp.shape_name);

            board.parts.push(Part {
                id: part_id,
                name: comp.name.clone(),
                mfg_code,
                side: mounting_side,
                part_type,
                p1: comp.place,
                p2: comp.place,
                pin_indices: Vec::new(),
                bounding_box: None,
                ..Default::default()
            });

            // Placement transform: mirror in shape space, then rotate counter-clockwise, then translate.
            // This is algebraically identical to OpenBoardView's formula (angle' = PI - angle when exactly
            // one mirror flag is set, signs applied after rotation), so pin positions are unchanged.
            let comp_rotation = comp.rotation.rem_euclid(360.0);
            let comp_mirror = Mirror::from_flags(comp.mirror_x, comp.mirror_y);
            let placed = |p: Point| transform(p, comp_rotation, comp_mirror, comp.place);

            let empty_shape = ShapeDef::default();
            let shape = shape_opt.unwrap_or(&empty_shape);
            let mut outline: Vec<Point> = outer_contour(&shape.paths).into_iter().map(placed).collect();
            let mut body_points: Vec<Point> =
                shape.paths.iter().flat_map(|path| path.points.iter().map(|&q| placed(q))).collect();

            for spin in &shape.pins {
                let pin_id = board.pins.len();
                let pin_pos = placed(spin.pos);

                // Determine side
                let mut pin_side = if let Some(ps_info) = padstacks.get(&spin.pad_name) {
                    ps_info.side
                } else {
                    mounting_side
                };

                if comp.flip {
                    pin_side = match pin_side {
                        Side::Top => Side::Bottom,
                        Side::Bottom => Side::Top,
                        Side::Both => Side::Both,
                    };
                }

                let pad = resolve_pad(spin, padstacks.get(&spin.pad_name), &pad_defs, &pad_extents, comp.layer, comp.flip, &placed);
                let reserved = if pad.size.is_some() { pad.bounds } else { pad.bounds.expand(MARKER_RADIUS) };
                body_points.extend(rectangle(&reserved));

                // Net assignment
                let key = (comp.name.clone(), spin.name.clone());
                let net_name = if let Some(n) = signals_cache.get(&key) {
                    n.clone()
                } else {
                    let nc = format!("NC@{nc_counter}");
                    nc_counter += 1;
                    nc
                };

                board.pins.push(Pin {
                    id: pin_id,
                    part_id,
                    name: spin.name.clone(),
                    pos: pin_pos,
                    side: pin_side,
                    net_name,
                    radius: pad.radius,
                    shape: pad.shape,
                    width: pad.size.map(|(w, _)| w),
                    height: pad.size.map(|(_, h)| h),
                    rotation_deg: pad.rotation_deg,
                });
            }

            if body_points.is_empty() {
                body_points.push(comp.place);
            }
            let mut bounds = Rect::from_points(&body_points).expect("body points are not empty");
            if outline.is_empty() {
                // No closed artwork: the body is the pads' extent plus a small margin.
                bounds = bounds.expand(FALLBACK_BODY_MARGIN);
                outline = rectangle(&bounds);
            }
            let part = &mut board.parts[part_id];
            part.bounding_box = Some(bounds);
            part.outline = outline;
            part.rotation_deg = comp_rotation;
        }

        // Outline handling:
        // If outline_segments exist from $BOARD, use their vertices for outline_polygon.
        if !outline_segments.is_empty() {
            // Collect points from outline segments
            let mut pts = Vec::with_capacity(outline_segments.len() * 2);
            for (p1, p2) in &outline_segments {
                pts.push(*p1);
                pts.push(*p2);
            }
            // Also ensure board bounding box covers all outline segments
            if let Some(rect) = crate::model::Rect::from_points(&pts) {
                board.outline_polygon = vec![
                    Point::new(rect.min_x, rect.min_y),
                    Point::new(rect.max_x, rect.min_y),
                    Point::new(rect.max_x, rect.max_y),
                    Point::new(rect.min_x, rect.max_y),
                    Point::new(rect.min_x, rect.min_y),
                ];
            }
        } else if !board.pins.is_empty() {
            // Generate bounding box outline with 20-mil margin (matching OpenBoardView fallback)
            let pin_pts: Vec<Point> = board.pins.iter().map(|p| p.pos).collect();
            if let Some(rect) = crate::model::Rect::from_points(&pin_pts) {
                const OUTLINE_MARGIN: f64 = 20.0;
                let min_x = rect.min_x - OUTLINE_MARGIN;
                let max_x = rect.max_x + OUTLINE_MARGIN;
                let min_y = rect.min_y - OUTLINE_MARGIN;
                let max_y = rect.max_y + OUTLINE_MARGIN;

                board.outline_polygon = vec![
                    Point::new(min_x, min_y),
                    Point::new(max_x, min_y),
                    Point::new(max_x, max_y),
                    Point::new(min_x, max_y),
                    Point::new(min_x, min_y),
                ];
            }
        }

        board.rebuild_indices();
        Ok(board)
    }
}
