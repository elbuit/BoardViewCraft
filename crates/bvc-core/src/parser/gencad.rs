//! GenCAD 1.4 file format parser.
//!
//! Faithful implementation of OpenBoardView's `GenCADFile.cpp` logic:
//! - $HEADER: Reads UNITS (INCH, THOU, MM, MM100, USER, USERCM, USERMM).
//! - $BOARD: Reads LINE, ARC, RECTANGLE outlines into outline segments / polygon.
//! - $PADSTACKS: Reads drill sizes (drill_size > 0 => ThroughHole, else SMD) and layer sides (TOP, BOTTOM, ALL).
//! - $SHAPES: Reads pin definitions (pin_name, pad_name, x, y, layer, rot, mirror).
//! - $COMPONENTS: Reads PLACE, LAYER, ROTATION, SHAPE (with MIRRORX, MIRRORY, FLIP). Computes transformed pin coordinates.
//! - $SIGNALS: Reads SIGNAL name and NODE component/pin bindings to map nets to pins.
//! - $ROUTES: Reads ROUTE net name and VIA definitions to populate board vias.

use std::collections::{BTreeMap, BTreeSet};
use std::f64::consts::PI;
use crate::board::Board;
use crate::model::{Part, PartType, Pin, Point, Side, Via};
use crate::parser::{FormatParser, ParseError};

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
}

#[derive(Debug, Clone)]
struct ShapePin {
    name: String,
    pad_name: String,
    pos: Point,
    _layer: String,
    _rotation: f64,
    _mirror: String,
}

#[derive(Debug, Clone, Default)]
struct ShapeDef {
    pins: Vec<ShapePin>,
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
                            });
                        }
                    } else if cmd == "PAD" {
                        // PAD <pad_name> <layer> <rot> <mirror>
                        if tokens.len() >= 3 {
                            let layer_str = tokens[2].to_ascii_uppercase();
                            if let Some(ref mut info) = current_padstack_info {
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
                                    let layer = if tokens.len() >= 6 { tokens[5].to_string() } else { "TOP".to_string() };
                                    let rot = if tokens.len() >= 7 { tokens[6].parse::<f64>().unwrap_or(0.0) } else { 0.0 };
                                    let mirror = if tokens.len() >= 8 { tokens[7].to_string() } else { "0".to_string() };

                                    let px = Self::board_unit_to_brd_coordinate(x, dimension, dimension_unit);
                                    let py = Self::board_unit_to_brd_coordinate(y, dimension, dimension_unit);

                                    if let Some(shape_def) = shapes.get_mut(sname) {
                                        shape_def.pins.push(ShapePin {
                                            name: pin_name,
                                            pad_name,
                                            pos: Point::new(px, py),
                                            _layer: layer,
                                            _rotation: rot,
                                            _mirror: mirror,
                                        });
                                    }
                                }
                            }
                        }
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
            });

            // Calculate pins
            if let Some(shape) = shape_opt {
                // OpenBoardView rotation and mirror transformation:
                // double rotation_in_rads = (rotation_in_degrees * (M_PI / 180.0));
                // int mirror_x_sign = mirror_x ? (-1) : 1;
                // int mirror_y_sign = mirror_y ? (-1) : 1;
                // if (mirror_x_sign * mirror_y_sign == -1) {
                //     rotation_in_rads = M_PI - rotation_in_rads;
                // }
                // double cos_ = cos(rotation_in_rads);
                // double sin_ = sin(rotation_in_rads);
                // pin.pos.x = part->p1.x + mirror_x_sign * (tmpPos.x * cos_ - tmpPos.y * sin_);
                // pin.pos.y = part->p1.y + mirror_y_sign * (tmpPos.x * sin_ + tmpPos.y * cos_);
                let mut rot_rads = comp.rotation * (PI / 180.0);
                let mirror_x_sign: f64 = if comp.mirror_x { -1.0 } else { 1.0 };
                let mirror_y_sign: f64 = if comp.mirror_y { -1.0 } else { 1.0 };
                if (mirror_x_sign * mirror_y_sign) < 0.0 {
                    rot_rads = PI - rot_rads;
                }
                let cos_val = rot_rads.cos();
                let sin_val = rot_rads.sin();

                for spin in &shape.pins {
                    let pin_id = board.pins.len();
                    let pin_x = comp.place.x + mirror_x_sign * (spin.pos.x * cos_val - spin.pos.y * sin_val);
                    let pin_y = comp.place.y + mirror_y_sign * (spin.pos.x * sin_val + spin.pos.y * cos_val);

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
                        pos: Point::new(pin_x, pin_y),
                        side: pin_side,
                        net_name,
                        radius: 5.0,
                    });
                }
            }
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
