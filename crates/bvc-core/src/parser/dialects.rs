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
                board.recompute_bounds();
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
        s.contains("var_data:") || s.contains("Format:")
    }
    fn parse(&self, data: &[u8]) -> Result<Board, ParseError> {
        if Self::looks_like_bvr_binary(data) {
            return Self::parse_bvr_binary(data);
        }
        let text = String::from_utf8_lossy(data);
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
