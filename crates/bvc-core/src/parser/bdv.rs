//! Arithmetic deobfuscation `.bdv` parser.
//!
//! Two arithmetic schedules:
//! 1. Historical per-byte schedule: `key` starts at 160, +1 per non-newline, wraps 285 -> 159. Decodes to Test_Link shape.
//! 2. Field per-line schedule: header `dd:`, `key` increments once per `\n`, wraps 285 -> 159. Decodes to fixture dialect (`<<format.asc>>`, `<<pins.asc>>`, `Part <ref> (T/B)`).

use crate::board::Board;
use crate::model::{Part, PartType, Pin, Point, Side, Via};
use crate::parser::test_link::{DialectMarkers, TestLinkEngine};
use crate::parser::{FormatParser, ParseError};

pub struct BdvParser;

impl BdvParser {
    pub const fn new() -> Self {
        Self
    }

    pub fn is_field_variant(data: &[u8]) -> bool {
        data.starts_with(b"dd:")
    }

    /// Decodes per-byte schedule (historical synthetic shape)
    pub fn decode_per_byte(data: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(data.len());
        let mut key: i32 = 160;
        for &b in data {
            if b == 10 || b == 13 {
                out.push(b);
                continue;
            }
            out.push(((key - (b as i32)) & 0xFF) as u8);
            if key >= 285 {
                key = 159;
            } else {
                key += 1;
            }
        }
        out
    }

    /// Decodes per-line schedule (field shape)
    pub fn decode_per_line(data: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(data.len());
        let mut key: i32 = 160;
        for &b in data {
            if b == 10 || b == 13 {
                out.push(b);
                if b == 10 {
                    key += 1;
                    if key > 285 {
                        key = 159;
                    }
                }
                continue;
            }
            out.push(((key - (b as i32)) & 0xFF) as u8);
        }
        out
    }

    /// Parses test-fixture dialect resulting from field per-line decode
    pub fn parse_fixture_dialect(text: &str) -> Result<Board, ParseError> {
        let mut board = Board::new();
        let mut in_outline = false;
        let mut cur_part: Option<String> = None;
        let mut cur_side = Side::Top;
        let mut cur_pin_idx = 0;

        for raw_line in text.lines() {
            let line = raw_line.trim();
            if line.is_empty() {
                continue;
            }

            if line.starts_with("<<") {
                in_outline = line == "<<format.asc>>";
                cur_part = None;
                continue;
            }

            if line.contains("Board Outline Contour") {
                in_outline = true;
                cur_part = None;
                continue;
            }

            // Part header: Part <refdes> (T) or (B)
            if line.starts_with("Part ") || line.starts_with("Part\t") {
                in_outline = false;
                let toks: Vec<&str> = line.split_whitespace().collect();
                if toks.len() >= 2 {
                    let refdes = toks[1].to_string();
                    let side = if toks.len() >= 3 && toks[2].contains('B') {
                        Side::Bottom
                    } else {
                        Side::Top
                    };
                    cur_part = Some(refdes.clone());
                    cur_side = side;
                    cur_pin_idx = 0;

                    let part_id = board.parts.len();
                    board.parts.push(Part {
                        id: part_id,
                        name: refdes,
                        mfg_code: String::new(),
                        side,
                        part_type: PartType::Smd,
                        p1: Point::ZERO,
                        p2: Point::ZERO,
                        pin_indices: Vec::new(),
                        bounding_box: None,
                    });
                }
                continue;
            }

            // Nail row: starts with '$'
            if line.starts_with('$') {
                in_outline = false;
                let toks: Vec<&str> = line.split_whitespace().collect();
                // $1  4.7725  1.5550  1  D2  (T)  #3  GND
                if toks.len() >= 4 {
                    let x_raw: f64 = toks[1].parse().unwrap_or(0.0);
                    let y_raw: f64 = toks[2].parse().unwrap_or(0.0);
                    let side = if line.contains("(B)") { Side::Bottom } else { Side::Top };
                    let net = toks.last().unwrap_or(&"").to_string();
                    let via_id = board.vias.len();
                    board.vias.push(Via {
                        id: via_id,
                        net_name: net,
                        pos: Point::new(x_raw * 1000.0, y_raw * 1000.0),
                        side,
                        radius: 8.0,
                    });
                }
                continue;
            }

            // Pin row inside a Part block
            if let Some(ref _refdes) = cur_part {
                let toks: Vec<&str> = line.split_whitespace().collect();
                // <id> <name> <x> <y> <layer> <net> [nail]
                if toks.len() >= 6 {
                    cur_pin_idx += 1;
                    let pin_name = toks[1].to_string();
                    let x_raw: f64 = toks[2].parse().unwrap_or(0.0);
                    let y_raw: f64 = toks[3].parse().unwrap_or(0.0);
                    let net = if toks[5] == "(NC)" { String::new() } else { toks[5].to_string() };

                    let part_id = board.parts.len().saturating_sub(1);
                    let pin_id = board.pins.len();
                    board.pins.push(Pin {
                        id: pin_id,
                        part_id,
                        name: if !pin_name.is_empty() { pin_name } else { cur_pin_idx.to_string() },
                        net_name: net,
                        pos: Point::new(x_raw * 1000.0, y_raw * 1000.0),
                        side: cur_side,
                        radius: 12.0,
                    });
                    continue;
                }
            }

            // Outline vertex: X Y Radius
            if in_outline {
                let toks: Vec<&str> = line.split_whitespace().collect();
                if toks.len() >= 2 {
                    if let (Ok(x), Ok(y)) = (toks[0].parse::<f64>(), toks[1].parse::<f64>()) {
                        board.outline_polygon.push(Point::new(x * 1000.0, y * 1000.0));
                    }
                }
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

impl Default for BdvParser {
    fn default() -> Self {
        Self::new()
    }
}

impl FormatParser for BdvParser {
    fn name(&self) -> &'static str {
        "BDV"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["bdv"]
    }

    fn verify_format(&self, data: &[u8]) -> bool {
        if Self::is_field_variant(data) {
            return true;
        }
        let decoded = Self::decode_per_byte(&data[..data.len().min(1024)]);
        let s = String::from_utf8_lossy(&decoded);
        s.contains("var_data:") || s.contains("Format:") || s.contains("Parts:")
    }

    fn parse(&self, data: &[u8]) -> Result<Board, ParseError> {
        if Self::is_field_variant(data) {
            let decoded = Self::decode_per_line(data);
            let text = String::from_utf8_lossy(&decoded);
            return Self::parse_fixture_dialect(&text);
        }

        let decoded = Self::decode_per_byte(data);
        let text = String::from_utf8_lossy(&decoded);
        TestLinkEngine::parse_text(&text, &DialectMarkers::default())
    }
}
