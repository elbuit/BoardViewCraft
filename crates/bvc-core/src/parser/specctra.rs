//! Native Specctra DSN (`.dsn`) format parser.
//!
//! Specctra Design Format is an S-expression based interchange format widely used
//! for autorouting and board definitions (Cadence Allegro, Specctra, KiCad, Altium).
//!
//! Key sections parsed:
//! - Preprocessing: strip backslash-newline string continuations and drop `(string_quote ")` declarations.
//! - Root element: `(pcb ...)` or `(design ...)`.
//! - `library`: image definitions with pin coordinates `(pin <padstack> <pinNumber> <x> <y> [rot])`.
//! - `placement`: component instances `(component <image> (place <ref> <x> <y> <side> [rotation]))`.
//! - `network`: net definitions with assigned pins `(net <name> (pins <ref>-<pin> ...))`.
//! - `structure`: boundary definitions `(boundary (path ...))`, `(polygon ...)`, or `(rect ...)`.

use std::collections::HashMap;
use crate::board::Board;
use crate::model::{PadShape, Part, PartType, Pin, Point, Side};
use crate::parser::kicad::{parse_sexpr, SExpr};
use crate::parser::{FormatParser, ParseError};

pub struct SpecctraParser;

impl SpecctraParser {
    pub const fn new() -> Self {
        Self
    }

    /// Preprocesses Specctra text to normalize line continuations and drop quote redefinitions
    pub fn preprocess(input: &str) -> String {
        // 1. Remove backslash line continuations (\r\n or \n)
        let s = input.replace("\\\r\n", "").replace("\\\n", "");

        // 2. Remove (string_quote ...) patterns which cause bare quotes to confuse S-expression parsers
        let mut result = String::with_capacity(s.len());
        let mut chars = s.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '(' {
                // Peek ahead to see if it starts with "string_quote"
                let remaining: String = chars.clone().take(20).collect();
                let trimmed = remaining.trim_start();
                if trimmed.starts_with("string_quote") {
                    // Consume until matching ')'
                    for next_c in chars.by_ref() {
                        if next_c == ')' {
                            break;
                        }
                    }
                    continue;
                }
            }
            result.push(c);
        }
        result
    }
}

impl Default for SpecctraParser {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Default, Clone)]
struct ImagePin {
    num: String,
    dx: f64,
    dy: f64,
}

fn atom_to_str<'a>(node: &'a SExpr<'a>) -> Option<&'a str> {
    node.as_atom()
}

fn atom_to_f64(node: &SExpr<'_>) -> f64 {
    node.as_atom().and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0)
}

fn kid<'a, 'b>(list: &'b [SExpr<'a>], key: &str) -> Option<&'b [SExpr<'a>]> {
    for node in list {
        if let SExpr::List(children) = node {
            if let Some(head) = children.first().and_then(|h| h.as_atom()) {
                if head.eq_ignore_ascii_case(key) {
                    return Some(children.as_slice());
                }
            }
        }
    }
    None
}

fn kids<'a, 'b>(list: &'b [SExpr<'a>], key: &str) -> Vec<&'b [SExpr<'a>]> {
    let mut out = Vec::new();
    for node in list {
        if let SExpr::List(children) = node {
            if let Some(head) = children.first().and_then(|h| h.as_atom()) {
                if head.eq_ignore_ascii_case(key) {
                    out.push(children.as_slice());
                }
            }
        }
    }
    out
}

impl FormatParser for SpecctraParser {
    fn name(&self) -> &'static str {
        "Specctra DSN"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["dsn"]
    }

    fn verify_format(&self, data: &[u8]) -> bool {
        let sample_len = data.len().min(4096);
        let sample = String::from_utf8_lossy(&data[..sample_len]);
        let lower = sample.to_ascii_lowercase();
        // Look for (pcb or (design at top level
        lower.contains("(pcb") || lower.contains("(design")
    }

    fn parse(&self, data: &[u8]) -> Result<Board, ParseError> {
        let text = String::from_utf8_lossy(data);
        let preprocessed = Self::preprocess(&text);

        let top_nodes = parse_sexpr(&preprocessed)?;

        // Find (pcb ...) or (design ...)
        let pcb_node = top_nodes
            .iter()
            .find(|n| {
                if let SExpr::List(list) = n {
                    if let Some(head) = list.first().and_then(|h| h.as_atom()) {
                        return head.eq_ignore_ascii_case("pcb") || head.eq_ignore_ascii_case("design");
                    }
                }
                false
            })
            .and_then(|n| n.as_list())
            .ok_or_else(|| {
                ParseError::UnrecognizedFormat("Not a valid Specctra DSN file: missing (pcb) or (design) root".into())
            })?;

        let mut board = Board::new();

        // 1. Library: parse images and their pins
        let mut images: HashMap<String, Vec<ImagePin>> = HashMap::new();
        if let Some(lib_slice) = kid(pcb_node, "library") {
            for img_slice in kids(lib_slice, "image") {
                if img_slice.len() >= 2 {
                    let img_name = atom_to_str(&img_slice[1]).unwrap_or("").to_string();
                    let mut pins = Vec::new();
                    for pin_slice in kids(img_slice, "pin") {
                        // (pin <padstack> <pinNumber> <x> <y> [rot])
                        if pin_slice.len() >= 5 {
                            let pin_num = atom_to_str(&pin_slice[2]).unwrap_or("").to_string();
                            let dx = atom_to_f64(&pin_slice[3]);
                            let dy = atom_to_f64(&pin_slice[4]);
                            pins.push(ImagePin { num: pin_num, dx, dy });
                        }
                    }
                    images.insert(img_name, pins);
                }
            }
        }

        // 2. Placement: parse components and pin instances
        if let Some(place_slice) = kid(pcb_node, "placement") {
            for comp_slice in kids(place_slice, "component") {
                if comp_slice.len() >= 2 {
                    let img_name = atom_to_str(&comp_slice[1]).unwrap_or("");
                    for pl_slice in kids(comp_slice, "place") {
                        // (place <ref> <x> <y> <side> [rotation])
                        if pl_slice.len() >= 4 {
                            let ref_des = atom_to_str(&pl_slice[1]).unwrap_or("").to_string();
                            let x = atom_to_f64(&pl_slice[2]);
                            let y = atom_to_f64(&pl_slice[3]);
                            let side_str = if pl_slice.len() >= 5 {
                                atom_to_str(&pl_slice[4]).unwrap_or("front")
                            } else {
                                "front"
                            };
                            let side = if side_str.eq_ignore_ascii_case("back")
                                || side_str.eq_ignore_ascii_case("bottom")
                                || side_str.eq_ignore_ascii_case("b")
                            {
                                Side::Bottom
                            } else {
                                Side::Top
                            };
                            let rot_deg = if pl_slice.len() >= 6 {
                                atom_to_f64(&pl_slice[5])
                            } else {
                                0.0
                            };

                            let part_id = board.parts.len();
                            board.parts.push(Part {
                                id: part_id,
                                name: ref_des.clone(),
                                mfg_code: img_name.to_string(),
                                side,
                                part_type: PartType::Smd,
                                p1: Point::new(x, y),
                                p2: Point::new(x, y),
                                pin_indices: Vec::new(),
                                bounding_box: None,
                                rotation_deg: rot_deg,
                                ..Default::default()
                            });

                            if let Some(pins) = images.get(img_name) {
                                let rot_rad = (-rot_deg).to_radians();
                                let cos_a = rot_rad.cos();
                                let sin_a = rot_rad.sin();

                                for p in pins {
                                    let px = if side == Side::Bottom { -p.dx } else { p.dx };
                                    let py = p.dy;
                                    let wx = x + px * cos_a - py * sin_a;
                                    let wy = y + px * sin_a + py * cos_a;

                                    let pin_id = board.pins.len();
                                    board.pins.push(Pin {
                                        id: pin_id,
                                        part_id,
                                        name: p.num.clone(),
                                        pos: Point::new(wx, wy),
                                        side,
                                        net_name: String::new(),
                                        radius: 3.0,
                                        shape: PadShape::Rect,
                                        width: Some(6.0),
                                        height: Some(6.0),
                                        rotation_deg: rot_deg,
                                    });
                                    board.parts[part_id].pin_indices.push(pin_id);
                                }
                            }
                        }
                    }
                }
            }
        }

        // 3. Network: parse nets and map to pins (REF-PIN or REF PIN)
        let mut net_by_pin: HashMap<String, String> = HashMap::new();
        // Either in (network (net ...)) or directly (net ...) under pcb
        let net_sources = if let Some(net_slice) = kid(pcb_node, "network") {
            kids(net_slice, "net")
        } else {
            kids(pcb_node, "net")
        };

        for net_slice in net_sources {
            if net_slice.len() >= 2 {
                let net_name = atom_to_str(&net_slice[1]).unwrap_or("").to_string();
                if let Some(pins_slice) = kid(net_slice, "pins") {
                    for pin_node in &pins_slice[1..] {
                        if let Some(s) = atom_to_str(pin_node) {
                            net_by_pin.insert(s.to_string(), net_name.clone());
                        }
                    }
                }
                for single_pin in kids(net_slice, "pin") {
                    if single_pin.len() >= 2 {
                        if let Some(s) = atom_to_str(&single_pin[1]) {
                            net_by_pin.insert(s.to_string(), net_name.clone());
                        }
                    }
                }
            }
        }

        for comp in &board.parts {
            for &pin_id in &comp.pin_indices {
                let p = &mut board.pins[pin_id];
                let key_hyphen = format!("{}-{}", comp.name, p.name);
                let key_dot = format!("{}.{}", comp.name, p.name);
                let key_space = format!("{} {}", comp.name, p.name);
                if let Some(net) = net_by_pin.get(&key_hyphen).or_else(|| net_by_pin.get(&key_dot)).or_else(|| net_by_pin.get(&key_space)) {
                    p.net_name = net.clone();
                }
            }
        }

        // 4. Boundary outline
        let mut outline_points = Vec::new();

        fn extract_boundary_points(list: &[SExpr<'_>], pts: &mut Vec<Point>) {
            for node in list {
                if let SExpr::List(children) = node {
                    if let Some(head) = children.first().and_then(|h| h.as_atom()) {
                        if head.eq_ignore_ascii_case("boundary") {
                            if let Some(path) = kid(children, "path") {
                                // (path <layer> <width> (x y) (x y)...) or flat coordinates
                                for child in path.iter().skip(3) {
                                    if let SExpr::List(pt_slice) = child {
                                        if pt_slice.len() >= 2 {
                                            pts.push(Point::new(atom_to_f64(&pt_slice[0]), atom_to_f64(&pt_slice[1])));
                                        }
                                    }
                                }
                            } else if let Some(poly) = kid(children, "polygon") {
                                for child in poly.iter().skip(2) {
                                    if let SExpr::List(pt_slice) = child {
                                        if pt_slice.len() >= 2 {
                                            pts.push(Point::new(atom_to_f64(&pt_slice[0]), atom_to_f64(&pt_slice[1])));
                                        }
                                    }
                                }
                            } else if let Some(rect) = kid(children, "rect") {
                                // (rect <layer> <x1> <y1> <w> <h>) or (rect <layer> <x1> <y1> <x2> <y2>)
                                if rect.len() >= 6 {
                                    let x1 = atom_to_f64(&rect[2]);
                                    let y1 = atom_to_f64(&rect[3]);
                                    let w_or_x2 = atom_to_f64(&rect[4]);
                                    let h_or_y2 = atom_to_f64(&rect[5]);
                                    let (x2, y2) = if w_or_x2 > x1 && h_or_y2 > y1 {
                                        (w_or_x2, h_or_y2)
                                    } else {
                                        (x1 + w_or_x2, y1 + h_or_y2)
                                    };
                                    pts.push(Point::new(x1, y1));
                                    pts.push(Point::new(x2, y1));
                                    pts.push(Point::new(x2, y2));
                                    pts.push(Point::new(x1, y2));
                                    pts.push(Point::new(x1, y1));
                                }
                            }
                        } else if !head.eq_ignore_ascii_case("library")
                            && !head.eq_ignore_ascii_case("placement")
                            && !head.eq_ignore_ascii_case("network")
                            && !head.eq_ignore_ascii_case("net")
                        {
                            extract_boundary_points(children, pts);
                        }
                    }
                }
            }
        }

        if let Some(struct_slice) = kid(pcb_node, "structure") {
            extract_boundary_points(struct_slice, &mut outline_points);
        }
        if outline_points.is_empty() {
            extract_boundary_points(pcb_node, &mut outline_points);
        }

        board.outline_polygon = outline_points;
        board.rebuild_indices();

        if board.outline_polygon.is_empty() {
            if let Some(bbox) = board.bounding_box {
                const MARGIN: f64 = 20.0;
                board.outline_polygon = vec![
                    Point::new(bbox.min_x - MARGIN, bbox.min_y - MARGIN),
                    Point::new(bbox.max_x + MARGIN, bbox.min_y - MARGIN),
                    Point::new(bbox.max_x + MARGIN, bbox.max_y + MARGIN),
                    Point::new(bbox.min_x - MARGIN, bbox.max_y + MARGIN),
                ];
                board.rebuild_indices();
            }
        }

        Ok(board)
    }
}
