//! Native S-Expression `.kicad_pcb` parser according to KICAD_SPECS.md.

use std::borrow::Cow;
use std::collections::HashMap;
use crate::board::Board;
use crate::model::{Part, PartType, Pin, Point, Side, Via};
use crate::parser::{FormatParser, ParseError};

/// Representation of an S-Expression node.
#[derive(Debug, PartialEq, Clone)]
pub enum SExpr<'a> {
    Atom(Cow<'a, str>),
    List(Vec<SExpr<'a>>),
}

impl<'a> SExpr<'a> {
    /// Returns the atom string if this node is an atom.
    pub fn as_atom(&self) -> Option<&str> {
        match self {
            SExpr::Atom(s) => Some(s.as_ref()),
            _ => None,
        }
    }

    /// Returns the list slice if this node is a list.
    pub fn as_list(&self) -> Option<&[SExpr<'a>]> {
        match self {
            SExpr::List(l) => Some(l.as_slice()),
            _ => None,
        }
    }

    /// If this node is a list and starts with an atom matching `tag`, returns the slice of children.
    pub fn list_head(&self) -> Option<&str> {
        match self {
            SExpr::List(l) => l.first().and_then(|h| h.as_atom()),
            _ => None,
        }
    }
}

/// Helper methods to query children of a list node.
fn find_child<'a, 'b>(nodes: &'b [SExpr<'a>], key: &str) -> Option<&'b [SExpr<'a>]> {
    for node in nodes {
        if let SExpr::List(list) = node {
            if let Some(head) = list.first().and_then(|h| h.as_atom()) {
                if head == key {
                    return Some(list.as_slice());
                }
            }
        }
    }
    None
}

fn find_all_children<'a, 'b>(nodes: &'b [SExpr<'a>], key: &str) -> Vec<&'b [SExpr<'a>]> {
    let mut out = Vec::new();
    for node in nodes {
        if let SExpr::List(list) = node {
            if let Some(head) = list.first().and_then(|h| h.as_atom()) {
                if head == key {
                    out.push(list.as_slice());
                }
            }
        }
    }
    out
}

/// Parses an S-Expression string with support for nested lists, quoted strings, and bare atoms.
pub fn parse_sexpr(input: &str) -> Result<Vec<SExpr<'_>>, ParseError> {
    // Strip leading UTF-8 BOM if present
    let trimmed_input = input.strip_prefix('\u{feff}').unwrap_or(input);
    let bytes = trimmed_input.as_bytes();
    let len = bytes.len();
    let mut stack: Vec<Vec<SExpr<'_>>> = vec![Vec::new()];
    let mut i = 0;

    while i < len {
        let b = bytes[i];
        match b {
            b' ' | b'\t' | b'\r' | b'\n' => {
                i += 1;
            }
            b'(' => {
                stack.push(Vec::new());
                i += 1;
            }
            b')' => {
                if stack.len() <= 1 {
                    return Err(ParseError::SyntaxError {
                        line: 0,
                        message: format!("Unexpected ')' at byte offset {}", i),
                    });
                }
                let node = stack.pop().unwrap();
                stack.last_mut().unwrap().push(SExpr::List(node));
                i += 1;
            }
            b'"' => {
                i += 1;
                let start = i;
                let mut has_escapes = false;
                while i < len {
                    if bytes[i] == b'\\' {
                        has_escapes = true;
                        i += 2;
                    } else if bytes[i] == b'"' {
                        break;
                    } else {
                        i += 1;
                    }
                }
                let raw_str = &trimmed_input[start..i.min(len)];
                if i < len {
                    i += 1; // skip closing quote
                }
                let atom_str = if has_escapes {
                    let mut unescaped = String::with_capacity(raw_str.len());
                    let mut chars = raw_str.chars();
                    while let Some(c) = chars.next() {
                        if c == '\\' {
                            if let Some(next_c) = chars.next() {
                                unescaped.push(next_c);
                            }
                        } else {
                            unescaped.push(c);
                        }
                    }
                    Cow::Owned(unescaped)
                } else {
                    Cow::Borrowed(raw_str)
                };
                stack.last_mut().unwrap().push(SExpr::Atom(atom_str));
            }
            _ => {
                let start = i;
                while i < len
                    && !bytes[i].is_ascii_whitespace()
                    && bytes[i] != b'('
                    && bytes[i] != b')'
                    && bytes[i] != b'"'
                {
                    i += 1;
                }
                let raw_atom = &trimmed_input[start..i];
                stack.last_mut().unwrap().push(SExpr::Atom(Cow::Borrowed(raw_atom)));
            }
        }
    }

    if stack.len() != 1 {
        return Err(ParseError::SyntaxError {
            line: 0,
            message: "Unclosed '(' in S-Expression input".to_string(),
        });
    }

    Ok(stack.pop().unwrap())
}

/// Converts millimeters to mils (1 mm = 1.0 / 0.0254 mils = ~39.3700787 mils).
#[inline]
pub fn mm_to_mils(mm: f64) -> f64 {
    mm / 0.0254
}

/// Extracts (x, y, [angle]) from an `(at X Y [ANGLE])` list.
fn parse_at(node: &[SExpr<'_>]) -> Option<(f64, f64, f64)> {
    if node.len() < 3 {
        return None;
    }
    let x = node[1].as_atom()?.parse::<f64>().ok()?;
    let y = node[2].as_atom()?.parse::<f64>().ok()?;
    let angle = if node.len() > 3 {
        node[3].as_atom().and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0)
    } else {
        0.0
    };
    Some((x, y, angle))
}

/// KiCad format parser.
pub struct KicadParser;

impl KicadParser {
    pub const fn new() -> Self {
        Self
    }
}

impl Default for KicadParser {
    fn default() -> Self {
        Self::new()
    }
}

impl FormatParser for KicadParser {
    fn name(&self) -> &'static str {
        "KiCad"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["kicad_pcb"]
    }

    fn verify_format(&self, data: &[u8]) -> bool {
        let sample = &data[..data.len().min(1024)];
        let text = String::from_utf8_lossy(sample);
        text.contains("(kicad_pcb")
    }

    fn parse(&self, data: &[u8]) -> Result<Board, ParseError> {
        let text = std::str::from_utf8(data)
            .map_err(|e| ParseError::SyntaxError { line: 0, message: format!("Invalid UTF-8: {}", e) })?;

        let nodes = parse_sexpr(text)?;
        let root = nodes.iter().find(|n| n.list_head() == Some("kicad_pcb"))
            .ok_or_else(|| ParseError::UnrecognizedFormat("Missing (kicad_pcb root node".into()))?;

        let pcb_children = root.as_list().unwrap();

        let mut board = Board::new();

        // 1. Net table: (net CODE NAME)
        let mut net_code_to_name: HashMap<i64, String> = HashMap::new();
        for child in pcb_children {
            if let Some(list) = child.as_list() {
                if list.first().and_then(|h| h.as_atom()) == Some("net") && list.len() >= 3 {
                    if let (Some(code_str), Some(name_str)) = (list[1].as_atom(), list[2].as_atom()) {
                        if let Ok(code) = code_str.parse::<i64>() {
                            if code != 0 && !name_str.is_empty() {
                                net_code_to_name.insert(code, name_str.to_string());
                            }
                        }
                    }
                }
            }
        }

        // 2. Outlines on "Edge.Cuts": gr_line, gr_rect, gr_arc, gr_poly
        let mut raw_segments: Vec<(Point, Point)> = Vec::new();

        for child in pcb_children {
            let Some(list) = child.as_list() else { continue; };
            let Some(head) = list.first().and_then(|h| h.as_atom()) else { continue; };

            let is_edge_cuts = find_child(list, "layer")
                .and_then(|l| l.get(1))
                .and_then(|n| n.as_atom())
                == Some("Edge.Cuts");

            if !is_edge_cuts {
                continue;
            }

            match head {
                "gr_line" => {
                    let start = find_child(list, "start").and_then(|s| {
                        let x = s.get(1)?.as_atom()?.parse::<f64>().ok()?;
                        let y = s.get(2)?.as_atom()?.parse::<f64>().ok()?;
                        Some(Point::new(mm_to_mils(x), mm_to_mils(y)))
                    });
                    let end = find_child(list, "end").and_then(|e| {
                        let x = e.get(1)?.as_atom()?.parse::<f64>().ok()?;
                        let y = e.get(2)?.as_atom()?.parse::<f64>().ok()?;
                        Some(Point::new(mm_to_mils(x), mm_to_mils(y)))
                    });
                    if let (Some(p1), Some(p2)) = (start, end) {
                        raw_segments.push((p1, p2));
                    }
                }
                "gr_rect" => {
                    let start = find_child(list, "start").and_then(|s| {
                        let x = s.get(1)?.as_atom()?.parse::<f64>().ok()?;
                        let y = s.get(2)?.as_atom()?.parse::<f64>().ok()?;
                        Some(Point::new(mm_to_mils(x), mm_to_mils(y)))
                    });
                    let end = find_child(list, "end").and_then(|e| {
                        let x = e.get(1)?.as_atom()?.parse::<f64>().ok()?;
                        let y = e.get(2)?.as_atom()?.parse::<f64>().ok()?;
                        Some(Point::new(mm_to_mils(x), mm_to_mils(y)))
                    });
                    if let (Some(p1), Some(p2)) = (start, end) {
                        let p1_x = p1.x;
                        let p1_y = p1.y;
                        let p2_x = p2.x;
                        let p2_y = p2.y;
                        let a = Point::new(p1_x, p1_y);
                        let b = Point::new(p2_x, p1_y);
                        let c = Point::new(p2_x, p2_y);
                        let d = Point::new(p1_x, p2_y);
                        raw_segments.push((a, b));
                        raw_segments.push((b, c));
                        raw_segments.push((c, d));
                        raw_segments.push((d, a));
                    }
                }
                "gr_arc" => {
                    let start = find_child(list, "start").and_then(|s| {
                        let x = s.get(1)?.as_atom()?.parse::<f64>().ok()?;
                        let y = s.get(2)?.as_atom()?.parse::<f64>().ok()?;
                        Some(Point::new(mm_to_mils(x), mm_to_mils(y)))
                    });
                    let mid = find_child(list, "mid").and_then(|s| {
                        let x = s.get(1)?.as_atom()?.parse::<f64>().ok()?;
                        let y = s.get(2)?.as_atom()?.parse::<f64>().ok()?;
                        Some(Point::new(mm_to_mils(x), mm_to_mils(y)))
                    });
                    let end = find_child(list, "end").and_then(|e| {
                        let x = e.get(1)?.as_atom()?.parse::<f64>().ok()?;
                        let y = e.get(2)?.as_atom()?.parse::<f64>().ok()?;
                        Some(Point::new(mm_to_mils(x), mm_to_mils(y)))
                    });
                    if let (Some(p1), Some(pm), Some(p2)) = (start, mid, end) {
                        raw_segments.push((p1, pm));
                        raw_segments.push((pm, p2));
                    } else if let (Some(p1), Some(p2)) = (start, end) {
                        raw_segments.push((p1, p2));
                    }
                }
                "gr_poly" => {
                    if let Some(pts) = find_child(list, "pts") {
                        let mut poly_pts = Vec::new();
                        for pt_node in pts.iter().skip(1) {
                            if let Some(pt_list) = pt_node.as_list() {
                                if pt_list.first().and_then(|h| h.as_atom()) == Some("xy") && pt_list.len() >= 3 {
                                    if let (Some(x_str), Some(y_str)) = (pt_list[1].as_atom(), pt_list[2].as_atom()) {
                                        if let (Ok(x), Ok(y)) = (x_str.parse::<f64>(), y_str.parse::<f64>()) {
                                            poly_pts.push(Point::new(mm_to_mils(x), mm_to_mils(y)));
                                        }
                                    }
                                }
                            }
                        }
                        for i in 0..poly_pts.len() {
                            let p1 = poly_pts[i];
                            let p2 = poly_pts[(i + 1) % poly_pts.len()];
                            raw_segments.push((p1, p2));
                        }
                    }
                }
                _ => {}
            }
        }

        // Chain outline segments into polygon
        if !raw_segments.is_empty() {
            let mut chain = Vec::new();
            let first = raw_segments.remove(0);
            chain.push(first.0);
            chain.push(first.1);

            let tol = 1.0; // 1 mil tolerance
            while !raw_segments.is_empty() {
                let curr = *chain.last().unwrap();
                let mut found_idx = None;
                let mut reversed = false;

                for (idx, (p1, p2)) in raw_segments.iter().enumerate() {
                    if (curr.x - p1.x).abs() <= tol && (curr.y - p1.y).abs() <= tol {
                        found_idx = Some(idx);
                        reversed = false;
                        break;
                    } else if (curr.x - p2.x).abs() <= tol && (curr.y - p2.y).abs() <= tol {
                        found_idx = Some(idx);
                        reversed = true;
                        break;
                    }
                }

                if let Some(idx) = found_idx {
                    let seg = raw_segments.remove(idx);
                    if reversed {
                        chain.push(seg.0);
                    } else {
                        chain.push(seg.1);
                    }
                } else {
                    // Try to connect to start of chain or finish this loop
                    break;
                }
            }

            // If the last point equals the first point, remove duplicate closing point
            if chain.len() > 1 {
                let first_pt = chain[0];
                let last_pt = *chain.last().unwrap();
                if (first_pt.x - last_pt.x).abs() <= tol && (first_pt.y - last_pt.y).abs() <= tol {
                    chain.pop();
                }
            }
            board.outline_polygon = chain;
        }

        // 3. Footprints: (footprint ...) or (module ...)
        for child in pcb_children {
            let Some(list) = child.as_list() else { continue; };
            let Some(head) = list.first().and_then(|h| h.as_atom()) else { continue; };
            if head != "footprint" && head != "module" {
                continue;
            }

            // Layer: F.Cu => Side::Top, B.Cu => Side::Bottom
            let layer_side = find_child(list, "layer")
                .and_then(|l| l.get(1))
                .and_then(|n| n.as_atom())
                .map(|layer_name| {
                    if layer_name == "B.Cu" {
                        Side::Bottom
                    } else {
                        Side::Top
                    }
                })
                .unwrap_or(Side::Top);

            // Refdes from fp_text reference or property "Reference"
            let mut refdes = String::new();
            for item in list {
                if let Some(item_list) = item.as_list() {
                    let item_head = item_list.first().and_then(|h| h.as_atom()).unwrap_or("");
                    if item_head == "fp_text" && item_list.len() >= 3 {
                        if item_list[1].as_atom() == Some("reference") {
                            if let Some(name) = item_list[2].as_atom() {
                                refdes = name.to_string();
                            }
                        }
                    } else if item_head == "property" && item_list.len() >= 3 {
                        if item_list[1].as_atom() == Some("Reference") {
                            if let Some(name) = item_list[2].as_atom() {
                                refdes = name.to_string();
                            }
                        }
                    }
                }
            }

            // Footprint at (at FX FY [FA])
            let (fx_mm, fy_mm, fa_deg) = find_child(list, "at")
                .and_then(parse_at)
                .unwrap_or((0.0, 0.0, 0.0));

            // Part type
            let is_smd = find_child(list, "attr").map(|attr| {
                attr.iter().any(|arg| arg.as_atom() == Some("smd"))
            }).unwrap_or(true);

            let part_type = if is_smd { PartType::Smd } else { PartType::ThroughHole };

            let part_id = board.parts.len();
            board.parts.push(Part {
                id: part_id,
                name: if refdes.is_empty() { format!("FP_{}", part_id) } else { refdes },
                mfg_code: String::new(),
                side: layer_side,
                part_type,
                p1: Point::ZERO,
                p2: Point::ZERO,
                pin_indices: Vec::new(),
                bounding_box: None,
            });

            // Angle rad
            let theta = fa_deg.to_radians();
            let cos_theta = theta.cos();
            let sin_theta = theta.sin();

            // Pads: (pad NUMBER TYPE SHAPE (at PX PY [PA]) (size W H) (layers ...) (net CODE ...))
            for pad_slice in find_all_children(list, "pad") {
                if pad_slice.len() < 4 {
                    continue;
                }
                let pad_number = pad_slice[1].as_atom().unwrap_or("").to_string();
                let pad_type = pad_slice[2].as_atom().unwrap_or("");
                let _pad_shape = pad_slice[3].as_atom().unwrap_or("");

                let (px_mm, py_mm, _pa_deg) = find_child(pad_slice, "at")
                    .and_then(parse_at)
                    .unwrap_or((0.0, 0.0, 0.0));

                // Formula in KICAD_SPECS.md §6.3:
                // AX = FX + PX * cos(θ) + PY * sin(θ)
                // AY = FY - PX * sin(θ) + PY * cos(θ)
                let ax_mm = fx_mm + px_mm * cos_theta + py_mm * sin_theta;
                let ay_mm = fy_mm - px_mm * sin_theta + py_mm * cos_theta;

                let ax_mils = mm_to_mils(ax_mm);
                let ay_mils = mm_to_mils(ay_mm);

                // Size W H
                let (w_mm, h_mm) = find_child(pad_slice, "size").and_then(|s| {
                    if s.len() >= 3 {
                        let w = s[1].as_atom()?.parse::<f64>().ok()?;
                        let h = s[2].as_atom()?.parse::<f64>().ok()?;
                        Some((w, h))
                    } else {
                        None
                    }
                }).unwrap_or((0.5, 0.5));

                let radius_mils = (mm_to_mils(w_mm).min(mm_to_mils(h_mm)) * 0.5).max(1.0);

                // Layers & side
                let mut on_both = pad_type == "thru_hole" || pad_type == "np_thru_hole";
                if let Some(layers_child) = find_child(pad_slice, "layers") {
                    for l in layers_child.iter().skip(1) {
                        if let Some(lname) = l.as_atom() {
                            if lname == "*.Cu" || lname == "F&B.Cu" {
                                on_both = true;
                            }
                        }
                    }
                }

                let pin_side = if on_both {
                    Side::Both
                } else {
                    layer_side
                };

                // Net
                let mut net_name = String::new();
                if let Some(net_child) = find_child(pad_slice, "net") {
                    if net_child.len() >= 3 {
                        if let Some(name) = net_child[2].as_atom() {
                            net_name = name.to_string();
                        }
                    } else if net_child.len() >= 2 {
                        if let Some(code_or_name) = net_child[1].as_atom() {
                            if let Ok(code) = code_or_name.parse::<i64>() {
                                if let Some(n) = net_code_to_name.get(&code) {
                                    net_name = n.clone();
                                }
                            } else {
                                net_name = code_or_name.to_string();
                            }
                        }
                    }
                }

                let pin_id = board.pins.len();
                board.pins.push(Pin {
                    id: pin_id,
                    part_id,
                    name: if pad_number.is_empty() { (board.pins.len() + 1).to_string() } else { pad_number },
                    pos: Point::new(ax_mils, ay_mils),
                    side: pin_side,
                    net_name,
                    radius: radius_mils,
                });
            }
        }

        // 4. Vias: (via (at X Y) (size D) (net CODE) ...)
        for child in pcb_children {
            let Some(list) = child.as_list() else { continue; };
            let Some(head) = list.first().and_then(|h| h.as_atom()) else { continue; };
            if head != "via" {
                continue;
            }

            let (vx_mm, vy_mm, _) = find_child(list, "at")
                .and_then(parse_at)
                .unwrap_or((0.0, 0.0, 0.0));

            let size_mm = find_child(list, "size").and_then(|s| {
                if s.len() >= 2 {
                    s[1].as_atom()?.parse::<f64>().ok()
                } else {
                    None
                }
            }).unwrap_or(0.8);

            let mut via_net_name = String::new();
            if let Some(net_child) = find_child(list, "net") {
                if net_child.len() >= 3 {
                    if let Some(name) = net_child[2].as_atom() {
                        via_net_name = name.to_string();
                    }
                } else if net_child.len() >= 2 {
                    if let Some(code_or_name) = net_child[1].as_atom() {
                        if let Ok(code) = code_or_name.parse::<i64>() {
                            if let Some(n) = net_code_to_name.get(&code) {
                                via_net_name = n.clone();
                            }
                        } else {
                            via_net_name = code_or_name.to_string();
                        }
                    }
                }
            }

            let via_id = board.vias.len();
            board.vias.push(Via {
                id: via_id,
                pos: Point::new(mm_to_mils(vx_mm), mm_to_mils(vy_mm)),
                side: Side::Both,
                net_name: via_net_name,
                radius: (mm_to_mils(size_mm) * 0.5).max(1.0),
            });
        }

        board.rebuild_indices();

        // Synthesize outline bounding box if empty
        if board.outline_polygon.is_empty() && (!board.pins.is_empty() || !board.vias.is_empty()) {
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
