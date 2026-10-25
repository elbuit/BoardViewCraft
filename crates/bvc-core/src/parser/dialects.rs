//! Dialect parser for `.bv` (ATE BoardView), `.bvr` (Binary Board Viewer),
//! `.cst` (Compal Boardview), `.gr` (BoardView R5.0), and `.f2b` (Board save).

use crate::board::Board;
use crate::model::{Part, PartType, Pin, Point, Side, Via};
use crate::parser::brd2::Brd2Parser;
use crate::parser::test_link::{
    looks_like_binary, DialectMarkers, TestLinkEngine, CST_MARKERS, F2B_MARKERS, GR_MARKERS,
};
use crate::parser::{FormatParser, ParseError};

// --- .bv ---
pub struct BvParser;
impl BvParser {
    pub const fn new() -> Self { Self }
}
impl Default for BvParser {
    fn default() -> Self { Self::new() }
}
impl FormatParser for BvParser {
    fn name(&self) -> &'static str { "BV" }
    fn extensions(&self) -> &'static [&'static str] { &["bv"] }
    fn verify_format(&self, data: &[u8]) -> bool {
        if looks_like_binary(data, 0.30) {
            // Check JET4 magic
            return data.len() >= 20 && &data[0..4] == &[0x00, 0x01, 0x00, 0x00] && &data[4..19] == b"Standard Jet DB";
        }
        let sample = &data[..data.len().min(1024)];
        let s = String::from_utf8_lossy(sample);
        s.contains("BoardView") || s.contains("var_data:") || s.contains("Parts:") || s.contains("Pins:")
    }
    fn parse(&self, data: &[u8]) -> Result<Board, ParseError> {
        if looks_like_binary(data, 0.30) {
            return Err(ParseError::Other(
                "Binary JET4 MS Access database container for .bv not yet supported".into(),
            ));
        }
        let text = String::from_utf8_lossy(data);
        TestLinkEngine::parse_text(&text, &DialectMarkers::default())
    }
}

// --- .bvr ---
pub struct BvrParser;
impl BvrParser {
    pub const fn new() -> Self { Self }

    pub fn looks_like_bvr_binary(data: &[u8]) -> bool {
        data.len() >= 4 && &data[0..4] == b"BVR\0"
    }

    #[inline(always)]
    fn rd_u32(buf: &[u8], off: usize) -> Option<u32> {
        let end = off.checked_add(4)?;
        if end > buf.len() {
            return None;
        }
        Some(u32::from_le_bytes([buf[off], buf[off + 1], buf[off + 2], buf[off + 3]]))
    }

    #[inline(always)]
    fn rd_i32(buf: &[u8], off: usize) -> Option<i32> {
        let end = off.checked_add(4)?;
        if end > buf.len() {
            return None;
        }
        Some(i32::from_le_bytes([buf[off], buf[off + 1], buf[off + 2], buf[off + 3]]))
    }

    pub fn parse_bvr_binary(data: &[u8]) -> Result<Board, ParseError> {
        if data.len() < 12 {
            return Err(ParseError::SyntaxError { line: 0, message: "BVR binary too short".into() });
        }
        let mut cur = 4; // skip magic
        let _version = Self::rd_u32(data, cur).unwrap_or(0);
        cur += 4;

        let n_format = Self::rd_u32(data, cur).unwrap_or(0) as usize;
        cur += 4;

        let mut board = Board::new();

        // Format points
        for _ in 0..n_format {
            if cur + 8 > data.len() { break; }
            let x = Self::rd_i32(data, cur).unwrap_or(0);
            let y = Self::rd_i32(data, cur + 4).unwrap_or(0);
            cur += 8;
            board.outline_polygon.push(Point::new(x as f64, y as f64));
        }

        // Parts
        if cur + 4 > data.len() {
            board.rebuild_indices();
            return Ok(board);
        }
        let n_parts = Self::rd_u32(data, cur).unwrap_or(0) as usize;
        cur += 4;

        for _ in 0..n_parts {
            if cur >= data.len() { break; }
            let name_len = data[cur] as usize;
            cur += 1;
            if cur + name_len > data.len() { break; }
            let name = String::from_utf8_lossy(&data[cur..cur + name_len]).into_owned();
            cur += name_len;

            if cur + 17 > data.len() { break; }
            let _px = Self::rd_i32(data, cur).unwrap_or(0);
            let _py = Self::rd_i32(data, cur + 4).unwrap_or(0);
            // rotation (f32)
            cur += 12;
            let flags = data[cur];
            cur += 1;
            // width (f32), height (f32)
            cur += 8;

            let side = if (flags & 1) != 0 { Side::Top } else { Side::Bottom };
            let part_type = if (flags & 2) != 0 { PartType::Smd } else { PartType::ThroughHole };

            let part_id = board.parts.len();
            board.parts.push(Part {
                id: part_id,
                name,
                mfg_code: String::new(),
                side,
                part_type,
                p1: Point::ZERO,
                p2: Point::ZERO,
                pin_indices: Vec::new(),
                bounding_box: None,
            });
        }

        // Pins
        if cur + 4 <= data.len() {
            let n_pins = Self::rd_u32(data, cur).unwrap_or(0) as usize;
            cur += 4;

            for _ in 0..n_pins {
                if cur + 17 > data.len() { break; }
                let x = Self::rd_i32(data, cur).unwrap_or(0);
                let y = Self::rd_i32(data, cur + 4).unwrap_or(0);
                let _probe = Self::rd_i32(data, cur + 8).unwrap_or(0);
                let part_idx = Self::rd_u32(data, cur + 12).unwrap_or(0) as usize;
                let side_b = data[cur + 16];
                cur += 17;

                let side = match side_b {
                    2 => Side::Bottom,
                    3 => Side::Top,
                    _ => Side::Both,
                };

                // net string
                if cur >= data.len() { break; }
                let net_len = data[cur] as usize;
                cur += 1;
                if cur + net_len > data.len() { break; }
                let net = String::from_utf8_lossy(&data[cur..cur + net_len]).into_owned();
                cur += net_len;

                // number string
                if cur >= data.len() { break; }
                let num_len = data[cur] as usize;
                cur += 1;
                if cur + num_len > data.len() { break; }
                let pin_number = String::from_utf8_lossy(&data[cur..cur + num_len]).into_owned();
                cur += num_len;

                // name string
                if cur >= data.len() { break; }
                let pin_name_len = data[cur] as usize;
                cur += 1;
                if cur + pin_name_len > data.len() { break; }
                cur += pin_name_len;

                let part_id = if part_idx < board.parts.len() { part_idx } else { 0 };
                let pin_id = board.pins.len();
                board.pins.push(Pin {
                    id: pin_id,
                    part_id,
                    name: if !pin_number.is_empty() { pin_number } else { (board.pins.len() + 1).to_string() },
                    net_name: net,
                    pos: Point::new(x as f64, y as f64),
                    side,
                    radius: 12.0,
                });
            }
        }

        // Nails
        if cur + 4 <= data.len() {
            let n_nails = Self::rd_u32(data, cur).unwrap_or(0) as usize;
            cur += 4;
            for _ in 0..n_nails {
                if cur + 13 > data.len() { break; }
                let _probe = Self::rd_i32(data, cur).unwrap_or(0);
                let x = Self::rd_i32(data, cur + 4).unwrap_or(0);
                let y = Self::rd_i32(data, cur + 8).unwrap_or(0);
                let side_b = data[cur + 12];
                cur += 13;

                let side = match side_b {
                    1 => Side::Bottom,
                    _ => Side::Top,
                };

                if cur >= data.len() { break; }
                let net_len = data[cur] as usize;
                cur += 1;
                if cur + net_len > data.len() { break; }
                let net = String::from_utf8_lossy(&data[cur..cur + net_len]).into_owned();
                cur += net_len;

                let via_id = board.vias.len();
                board.vias.push(Via {
                    id: via_id,
                    net_name: net,
                    pos: Point::new(x as f64, y as f64),
                    side,
                    radius: 8.0,
                });
            }
        }

        board.rebuild_indices();

        // Synthesize outline bounding box with 20-mil margin
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

    /// Parses BVR Layout T1 (`BVRAW_FORMAT_1` text layout).
    pub fn parse_bvr_t1(text: &str) -> Result<Board, ParseError> {
        let mut board = Board::new();
        let mut current_section: Option<&str> = None;
        let mut skip_header_line = false;

        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            if trimmed == "<<Layout>>" || trimmed == "<<Pin>>" || trimmed == "<<Nail>>" {
                current_section = Some(trimmed);
                skip_header_line = true;
                continue;
            }

            if skip_header_line {
                skip_header_line = false;
                continue;
            }

            match current_section {
                Some("<<Layout>>") => {
                    // Coordinates X, Y multiplied by 1000 to convert to mils
                    let norm = trimmed.replacen(',', " ", 1);
                    let mut tokens = norm.split_whitespace();
                    if let (Some(x_str), Some(y_str)) = (tokens.next(), tokens.next()) {
                        if let (Ok(x), Ok(y)) = (x_str.parse::<f64>(), y_str.parse::<f64>()) {
                            let x_mils = (x * 1000.0).trunc();
                            let y_mils = (y * 1000.0).trunc();
                            board.outline_polygon.push(Point::new(x_mils, y_mils));
                        }
                    }
                }
                Some("<<Pin>>") => {
                    // Fields: Part, TB, Pin, Name, X, Y, Layer, Net
                    // Can be tab-separated or whitespace-separated
                    let fields: Vec<&str> = if line.contains('\t') {
                        line.split('\t').map(|s| s.trim()).collect()
                    } else {
                        trimmed.split_whitespace().collect()
                    };

                    if fields.len() >= 6 {
                        let part_name = fields[0];
                        let tb = fields[1];
                        let _pin_num = fields.get(2).copied().unwrap_or("");
                        let pin_name = fields.get(3).copied().unwrap_or("");
                        let x_str = fields[4];
                        let y_str = fields[5];
                        let net_name = fields.get(7).copied().unwrap_or("");

                        let side = if tb.contains("(T)") || tb.eq_ignore_ascii_case("T") {
                            Side::Top
                        } else {
                            Side::Bottom
                        };

                        // Group into parts by consecutive part name
                        let part_id = if let Some(last) = board.parts.last() {
                            if last.name == part_name {
                                board.parts.len() - 1
                            } else {
                                let new_id = board.parts.len();
                                board.parts.push(Part {
                                    id: new_id,
                                    name: part_name.to_string(),
                                    mfg_code: String::new(),
                                    side,
                                    part_type: PartType::Smd,
                                    p1: Point::ZERO,
                                    p2: Point::ZERO,
                                    pin_indices: Vec::new(),
                                    bounding_box: None,
                                });
                                new_id
                            }
                        } else {
                            let new_id = 0;
                            board.parts.push(Part {
                                id: new_id,
                                name: part_name.to_string(),
                                mfg_code: String::new(),
                                side,
                                part_type: PartType::Smd,
                                p1: Point::ZERO,
                                p2: Point::ZERO,
                                pin_indices: Vec::new(),
                                bounding_box: None,
                            });
                            new_id
                        };

                        if let (Ok(x), Ok(y)) = (x_str.parse::<f64>(), y_str.parse::<f64>()) {
                            let x_mils = (x * 1000.0).trunc();
                            let y_mils = (y * 1000.0).trunc();
                            let pin_id = board.pins.len();
                            board.pins.push(Pin {
                                id: pin_id,
                                part_id,
                                name: if !pin_name.is_empty() {
                                    pin_name.to_string()
                                } else {
                                    (board.pins.len() + 1).to_string()
                                },
                                pos: Point::new(x_mils, y_mils),
                                side,
                                net_name: net_name.to_string(),
                                radius: 12.0,
                            });
                        }
                    }
                }
                Some("<<Nail>>") => {
                    // Fields: Nail, X, Y, Type, Grid, TB, NetID, NetName
                    let fields: Vec<&str> = if line.contains('\t') {
                        line.split('\t').map(|s| s.trim()).collect()
                    } else {
                        trimmed.split_whitespace().collect()
                    };

                    if fields.len() >= 3 {
                        let x_str = fields[1];
                        let y_str = fields[2];
                        let tb = fields.get(5).copied().unwrap_or("");
                        let net_name = fields.get(7).copied().unwrap_or("");

                        let side = if tb.contains("(T)") || tb.eq_ignore_ascii_case("T") {
                            Side::Top
                        } else {
                            Side::Bottom
                        };

                        if let (Ok(x), Ok(y)) = (x_str.parse::<f64>(), y_str.parse::<f64>()) {
                            let x_mils = (x * 1000.0).trunc();
                            let y_mils = (y * 1000.0).trunc();
                            let via_id = board.vias.len();
                            board.vias.push(Via {
                                id: via_id,
                                pos: Point::new(x_mils, y_mils),
                                side,
                                net_name: net_name.to_string(),
                                radius: 8.0,
                            });
                        }
                    }
                }
                _ => {}
            }
        }

        board.rebuild_indices();

        // Synthesize outline bounding box with 20-mil margin if empty
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

    /// Chains outline segments as specified in BVR_SPECS.md §5.5 (Manhattan distance / endpoint matching).
    fn chain_segments(mut segments: Vec<(Point, Point)>) -> Vec<Point> {
        if segments.is_empty() {
            return Vec::new();
        }

        let mut polyline = Vec::new();
        let (s_start, first_end) = segments.remove(0);
        polyline.push(s_start);
        polyline.push(first_end);
        let mut cur_end = first_end;

        while !segments.is_empty() && cur_end != s_start {
            // Check exact endpoint match first
            let mut exact_idx = None;
            let mut exact_reversed = false;
            for (idx, &(p1, p2)) in segments.iter().enumerate() {
                if p1 == cur_end {
                    exact_idx = Some(idx);
                    exact_reversed = false;
                    break;
                } else if p2 == cur_end {
                    exact_idx = Some(idx);
                    exact_reversed = true;
                    break;
                }
            }

            if let Some(idx) = exact_idx {
                let (p1, p2) = segments.remove(idx);
                let next_end = if exact_reversed { p1 } else { p2 };
                polyline.push(next_end);
                cur_end = next_end;
                continue;
            }

            // Find nearest segment by Manhattan distance to cur_end
            let mut best_idx = 0;
            let mut best_dist = f64::INFINITY;
            let mut best_is_p1_nearer = true;

            for (idx, &(p1, p2)) in segments.iter().enumerate() {
                let d1 = (cur_end.x - p1.x).abs() + (cur_end.y - p1.y).abs();
                let d2 = (cur_end.x - p2.x).abs() + (cur_end.y - p2.y).abs();
                let (min_d, p1_nearer) = if d1 <= d2 { (d1, true) } else { (d2, false) };
                if min_d < best_dist {
                    best_dist = min_d;
                    best_idx = idx;
                    best_is_p1_nearer = p1_nearer;
                }
            }

            let dist_to_start = (cur_end.x - s_start.x).abs() + (cur_end.y - s_start.y).abs();
            if dist_to_start <= best_dist {
                polyline.push(s_start);
                break;
            }

            let (p1, p2) = segments.remove(best_idx);
            let (near_pt, far_pt) = if best_is_p1_nearer { (p1, p2) } else { (p2, p1) };
            polyline.push(near_pt);
            polyline.push(far_pt);
            cur_end = far_pt;
        }

        polyline
    }

    /// Parses BVR Layout T3 (`BVRAW_FORMAT_3` text layout).
    pub fn parse_bvr_t3(text: &str) -> Result<Board, ParseError> {
        let mut board = Board::new();

        // Part building state
        let mut part_name = String::new();
        let mut part_side = Side::Top;
        let mut part_origin = Point::ZERO;
        let mut part_smd = true;

        // Pin building state for pins of current part
        struct PinRecord {
            number: String,
            name: String,
            side: Side,
            pos: Point,
            radius: f64,
            net: String,
        }
        let mut current_part_pins: Vec<PinRecord> = Vec::new();

        let mut pin_number = String::new();
        let mut pin_name = String::new();
        let mut pin_side = Side::Top;
        let mut pin_pos = Point::ZERO;
        let mut pin_radius = 12.0;
        let mut pin_net = String::new();

        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            if trimmed == "PIN_END" {
                current_part_pins.push(PinRecord {
                    number: std::mem::take(&mut pin_number),
                    name: std::mem::take(&mut pin_name),
                    side: pin_side,
                    pos: pin_pos,
                    radius: pin_radius,
                    net: std::mem::take(&mut pin_net),
                });
                pin_side = part_side;
                pin_pos = part_origin;
                pin_radius = 12.0;
                continue;
            }

            if trimmed == "PART_END" {
                let part_id = board.parts.len();
                board.parts.push(Part {
                    id: part_id,
                    name: if !part_name.is_empty() {
                        std::mem::take(&mut part_name)
                    } else {
                        format!("U{}", part_id + 1)
                    },
                    mfg_code: String::new(),
                    side: part_side,
                    part_type: if part_smd { PartType::Smd } else { PartType::ThroughHole },
                    p1: Point::ZERO,
                    p2: Point::ZERO,
                    pin_indices: Vec::new(),
                    bounding_box: None,
                });

                for prec in current_part_pins.drain(..) {
                    let pin_id = board.pins.len();
                    board.pins.push(Pin {
                        id: pin_id,
                        part_id,
                        name: if !prec.number.is_empty() {
                            prec.number
                        } else if !prec.name.is_empty() {
                            prec.name
                        } else {
                            (board.pins.len() + 1).to_string()
                        },
                        pos: prec.pos,
                        side: prec.side,
                        net_name: prec.net,
                        radius: prec.radius,
                    });
                }

                // Reset part state
                part_name.clear();
                part_side = Side::Top;
                part_origin = Point::ZERO;
                part_smd = true;
                continue;
            }

            let (key, val) = match trimmed.split_once(' ') {
                Some((k, v)) => (k, v.trim()),
                None => (trimmed, ""),
            };

            match key {
                "PART_NAME" => {
                    let token = val.split_whitespace().next().unwrap_or("");
                    part_name = token.to_string();
                }
                "PART_SIDE" => {
                    let token = val.split_whitespace().next().unwrap_or("");
                    part_side = match token {
                        "B" => Side::Bottom,
                        "O" => Side::Both,
                        _ => Side::Top,
                    };
                }
                "PART_ORIGIN" => {
                    let mut tokens = val.split_whitespace();
                    if let (Some(x_str), Some(y_str)) = (tokens.next(), tokens.next()) {
                        if let (Ok(x), Ok(y)) = (x_str.parse::<f64>(), y_str.parse::<f64>()) {
                            part_origin = Point::new(x.trunc(), y.trunc());
                        }
                    }
                }
                "PART_MOUNT" => {
                    let token = val.split_whitespace().next().unwrap_or("");
                    part_smd = token == "SMD";
                }
                "PIN_NUMBER" => {
                    let token = val.split_whitespace().next().unwrap_or("");
                    pin_number = token.to_string();
                }
                "PIN_NAME" => {
                    let token = val.split_whitespace().next().unwrap_or("");
                    pin_name = token.to_string();
                }
                "PIN_SIDE" => {
                    let token = val.split_whitespace().next().unwrap_or("");
                    pin_side = match token {
                        "B" => Side::Bottom,
                        "O" => Side::Both,
                        _ => Side::Top,
                    };
                }
                "PIN_ORIGIN" => {
                    let mut tokens = val.split_whitespace();
                    if let (Some(x_str), Some(y_str)) = (tokens.next(), tokens.next()) {
                        if let (Ok(x), Ok(y)) = (x_str.parse::<f64>(), y_str.parse::<f64>()) {
                            pin_pos = Point::new(x.trunc() + part_origin.x, y.trunc() + part_origin.y);
                        }
                    }
                }
                "PIN_RADIUS" => {
                    let token = val.split_whitespace().next().unwrap_or("");
                    if let Ok(r) = token.parse::<f64>() {
                        pin_radius = r;
                    }
                }
                "PIN_NET" => {
                    let token = val.split_whitespace().next().unwrap_or("");
                    pin_net = token.to_string();
                }
                "OUTLINE_POINTS" => {
                    let nums: Vec<f64> = val
                        .split_whitespace()
                        .filter_map(|t| t.parse::<f64>().ok())
                        .collect();
                    for chunk in nums.chunks_exact(2) {
                        board.outline_polygon.push(Point::new(chunk[0].trunc(), chunk[1].trunc()));
                    }
                }
                "OUTLINE_SEGMENTED" => {
                    let nums: Vec<f64> = val
                        .split_whitespace()
                        .filter_map(|t| t.parse::<f64>().ok())
                        .collect();
                    let mut segs = Vec::new();
                    for chunk in nums.chunks_exact(4) {
                        segs.push((
                            Point::new(chunk[0].trunc(), chunk[1].trunc()),
                            Point::new(chunk[2].trunc(), chunk[3].trunc()),
                        ));
                    }
                    let chained = Self::chain_segments(segs);
                    board.outline_polygon.extend(chained);
                }
                _ => {}
            }
        }

        board.rebuild_indices();

        // Synthesize outline bounding box with 20-mil margin if empty
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
impl Default for BvrParser {
    fn default() -> Self { Self::new() }
}
impl FormatParser for BvrParser {
    fn name(&self) -> &'static str { "BVR" }
    fn extensions(&self) -> &'static [&'static str] { &["bvr"] }
    fn verify_format(&self, data: &[u8]) -> bool {
        if Self::looks_like_bvr_binary(data) {
            return true;
        }
        let sample = &data[..data.len().min(1024)];
        let s = String::from_utf8_lossy(sample);
        s.contains("BVRAW_FORMAT_1") || s.contains("BVRAW_FORMAT_3") || s.contains("var_data:") || s.contains("Format:")
    }
    fn parse(&self, data: &[u8]) -> Result<Board, ParseError> {
        if Self::looks_like_bvr_binary(data) {
            return Self::parse_bvr_binary(data);
        }
        let text = String::from_utf8_lossy(data);
        if text.contains("BVRAW_FORMAT_1") {
            return Self::parse_bvr_t1(&text);
        }
        if text.contains("BVRAW_FORMAT_3") {
            return Self::parse_bvr_t3(&text);
        }
        TestLinkEngine::parse_text(&text, &DialectMarkers::default())
    }
}

// --- .cst ---
pub struct CstParser;
impl CstParser {
    pub const fn new() -> Self { Self }
}
impl Default for CstParser {
    fn default() -> Self { Self::new() }
}
impl FormatParser for CstParser {
    fn name(&self) -> &'static str { "CST" }
    fn extensions(&self) -> &'static [&'static str] { &["cst"] }
    fn verify_format(&self, data: &[u8]) -> bool {
        if looks_like_binary(data, 0.30) {
            return false;
        }
        let sample = &data[..data.len().min(1024)];
        let s = String::from_utf8_lossy(sample);
        s.contains("[Format]") || s.contains("[Components]") || s.contains("[Parts]") || s.contains("[Pins]")
    }
    fn parse(&self, data: &[u8]) -> Result<Board, ParseError> {
        if looks_like_binary(data, 0.30) {
            return Err(ParseError::Other("Binary .cst containers are not supported".into()));
        }
        let text = String::from_utf8_lossy(data);
        TestLinkEngine::parse_text(&text, &CST_MARKERS)
    }
}

// --- .gr ---
pub struct GrParser;
impl GrParser {
    pub const fn new() -> Self { Self }
}
impl Default for GrParser {
    fn default() -> Self { Self::new() }
}
impl FormatParser for GrParser {
    fn name(&self) -> &'static str { "GR" }
    fn extensions(&self) -> &'static [&'static str] { &["gr"] }
    fn verify_format(&self, data: &[u8]) -> bool {
        if looks_like_binary(data, 0.30) {
            return false;
        }
        let sample = &data[..data.len().min(1024)];
        let s = String::from_utf8_lossy(sample);
        s.contains("BRDOUT:") || s.contains("Format:") || s.contains("Components:") || s.contains("Parts:")
    }
    fn parse(&self, data: &[u8]) -> Result<Board, ParseError> {
        if looks_like_binary(data, 0.30) {
            return Err(ParseError::Other("Binary .gr containers are not supported".into()));
        }
        let text = String::from_utf8_lossy(data);
        if text.contains("BRDOUT:") {
            return Brd2Parser::parse_content(&text);
        }
        TestLinkEngine::parse_text(&text, &GR_MARKERS)
    }
}

// --- .f2b ---
pub struct F2bParser;
impl F2bParser {
    pub const fn new() -> Self { Self }
}
impl Default for F2bParser {
    fn default() -> Self { Self::new() }
}
impl FormatParser for F2bParser {
    fn name(&self) -> &'static str { "F2B" }
    fn extensions(&self) -> &'static [&'static str] { &["f2b"] }
    fn verify_format(&self, data: &[u8]) -> bool {
        if looks_like_binary(data, 0.30) {
            return false;
        }
        let sample = &data[..data.len().min(1024)];
        let s = String::from_utf8_lossy(sample);
        s.contains("Outline:") || s.contains("Components:") || s.contains("Format:") || s.contains("var_data:")
    }
    fn parse(&self, data: &[u8]) -> Result<Board, ParseError> {
        if looks_like_binary(data, 0.30) {
            return Err(ParseError::Other("Binary .f2b containers are not supported".into()));
        }
        let text = String::from_utf8_lossy(data);
        TestLinkEngine::parse_text(&text, &F2B_MARKERS)
    }
}
