//! IPC-D-356 and IPC-D-356A test netlist parser.
//!
//! Follows standard IPC-D-356 fixed-width record layout:
//! - Operation codes: 317 (through-hole), 327 (surface-mount), 367 (tooling), 017/027/067 (continuation).
//! - Units: CUST 0 (0.0001 inch = 0.00254 mm), CUST 1 / SI (0.001 mm).
//! - Columns:
//!   - 4..17: Net name
//!   - 21..26: Component designator (RefDes)
//!   - 28..31: Pin number
//!   - 39..41: Access (A00=both, A01=top, A02/higher=bottom)
//!   - 42..49: X coordinate (+/-XXXXXX)
//!   - 50..57: Y coordinate (+/-YYYYYY)
//!   - 58..62: X size (pad width)
//!   - 63..67: Y size (pad height, 0 = round pad)
//!   - 68..71: Rotation degrees

use crate::board::Board;
use crate::model::{PadShape, Part, PartType, Pin, Point, Side};
use crate::parser::{FormatParser, ParseError};
use std::collections::HashMap;

pub struct Ipc356Parser;

impl Ipc356Parser {
    pub const fn new() -> Self {
        Self
    }
}

impl Default for Ipc356Parser {
    fn default() -> Self {
        Self::new()
    }
}

impl FormatParser for Ipc356Parser {
    fn name(&self) -> &'static str {
        "IPC-D-356"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["ipc", "356", "d356", "ipc356"]
    }

    fn verify_format(&self, data: &[u8]) -> bool {
        let sample = if data.len() > 4096 { &data[..4096] } else { data };
        let text = String::from_utf8_lossy(sample);
        for line in text.lines().take(50) {
            let trimmed = line.trim();
            if trimmed.starts_with("P  UNITS") || trimmed.starts_with("317 ") || trimmed.starts_with("327 ") {
                return true;
            }
        }
        false
    }

    fn parse(&self, data: &[u8]) -> Result<Board, ParseError> {
        let text = String::from_utf8_lossy(data);
        let mut board = Board::new();
        let mut parts_map: HashMap<String, usize> = HashMap::new();

        // Default scale: CUST 0 (0.0001 inch = 0.00254 mm)
        let mut scale = 0.00254;

        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('C') {
                continue; // Comment line
            }

            if line.starts_with("P  UNITS") {
                let upper = line.to_uppercase();
                if upper.contains("CUST 1") || upper.contains("SI") || upper.contains("MM") {
                    scale = 0.001; // 0.001 mm
                } else if upper.contains("CUST 0") || upper.contains("INCH") {
                    scale = 0.00254; // 0.0001 inch
                }
                continue;
            }

            if line.len() < 58 {
                continue;
            }

            let op = &line[0..line.len().min(3)];
            let is_th = op == "317" || op == "017";
            let is_smd = op == "327" || op == "027";

            if !is_th && !is_smd {
                continue;
            }

            // Slice fixed width fields safely
            let net = line.get(3..17).unwrap_or("").trim().replace('?', " ");
            let net = if net == "N/C" { String::new() } else { net };
            let refdes = line.get(20..26).unwrap_or("").trim().to_string();
            let pin_no = line.get(27..31).unwrap_or("").trim().to_string();

            if refdes.is_empty() {
                continue;
            }

            // Access side: A01 = Top, A02 / higher = Bottom, A00 = Both
            let side_str = line.get(38..41).unwrap_or("").trim();
            let side = if is_th || side_str == "A00" {
                Side::Both
            } else if side_str == "A01" {
                Side::Top
            } else {
                Side::Bottom
            };

            // Parse coordinates: X sign + 6 digits, Y sign + 6 digits
            let x_str = line.get(41..49).unwrap_or("").trim().trim_start_matches('X');
            let y_str = line.get(49..57).unwrap_or("").trim().trim_start_matches('Y');

            let x = x_str.parse::<f64>().unwrap_or(0.0) * scale;
            let y = y_str.parse::<f64>().unwrap_or(0.0) * scale;

            // Optional pad size
            let pad_w = line.get(57..62).and_then(|s| s.trim().trim_start_matches('X').parse::<f64>().ok()).unwrap_or(40.0) * scale;
            let pad_h = line.get(62..67).and_then(|s| s.trim().trim_start_matches('Y').parse::<f64>().ok()).unwrap_or(0.0) * scale;
            let is_round = pad_h == 0.0;
            let pad_height = if is_round { pad_w } else { pad_h };

            let rot = line.get(67..71).and_then(|s| s.trim().trim_start_matches('R').parse::<f64>().ok()).unwrap_or(0.0);

            let part_id = if let Some(&id) = parts_map.get(&refdes) {
                if board.parts[id].side != side && board.parts[id].side != Side::Both {
                    board.parts[id].side = Side::Both;
                }
                id
            } else {
                let id = board.parts.len();
                parts_map.insert(refdes.clone(), id);
                board.parts.push(Part {
                    id,
                    name: refdes.clone(),
                    mfg_code: String::new(),
                    side,
                    part_type: if is_th { PartType::ThroughHole } else { PartType::Smd },
                    p1: Point::new(x, y),
                    p2: Point::new(x, y),
                    pin_indices: Vec::new(),
                    bounding_box: None,
                    ..Default::default()
                });
                id
            };

            let pin_id = board.pins.len();
            board.pins.push(Pin {
                id: pin_id,
                part_id,
                name: pin_no,
                pos: Point::new(x, y),
                side,
                net_name: net,
                radius: pad_w.max(pad_height) * 0.5,
                shape: if is_round { PadShape::Round } else { PadShape::Rect },
                width: Some(pad_w),
                height: Some(pad_height),
                rotation_deg: rot,
            });
            board.parts[part_id].pin_indices.push(pin_id);
        }

        board.rebuild_indices();
        if board.outline_polygon.is_empty() {
            if let Some(bbox) = board.bounding_box {
                const MARGIN: f64 = 2.0;
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
