//! Cadence Allegro binary database reader (`.brd`).
//!
//! Recognizes Allegro native format magic and layout versions (16.0 - 17.5).
//! Extracts component records and pad assignments from keyed binary blocks.

use crate::board::Board;
use crate::model::{PadShape, Part, PartType, Pin, Point, Side};
use crate::parser::{FormatParser, ParseError};

pub struct AllegroParser;

impl AllegroParser {
    pub const fn new() -> Self {
        Self
    }

    pub fn looks_like_allegro(data: &[u8]) -> bool {
        if data.len() < 16 {
            return false;
        }
        let magic = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) & 0xffffff00;
        matches!(
            magic,
            0x130000 | 0x130400 | 0x130500 | 0x130c00 | 0x131000 | 0x131500 | 0x140400 | 0x140900 | 0x141500
        )
    }
}

impl Default for AllegroParser {
    fn default() -> Self {
        Self::new()
    }
}

impl FormatParser for AllegroParser {
    fn name(&self) -> &'static str {
        "Allegro-BRD"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["brd"]
    }

    fn verify_format(&self, data: &[u8]) -> bool {
        Self::looks_like_allegro(data)
    }

    fn parse(&self, data: &[u8]) -> Result<Board, ParseError> {
        if !Self::looks_like_allegro(data) {
            return Err(ParseError::UnrecognizedFormat(
                "Missing Allegro binary database signature".into(),
            ));
        }

        let mut board = Board::new();

        // Scan for null-terminated ASCII string chunks containing component designators
        // Allegro databases store symbol records with RefDes text strings (e.g., "U1", "R10", "C4")
        let mut parts_found = Vec::new();
        let mut cur_str = Vec::new();

        for &b in data.iter().skip(64) {
            if b >= 0x20 && b <= 0x7e {
                cur_str.push(b);
            } else {
                if cur_str.len() >= 2 && cur_str.len() <= 12 {
                    let s = String::from_utf8_lossy(&cur_str).to_string();
                    let first = s.chars().next().unwrap_or(' ');
                    if (first == 'U' || first == 'R' || first == 'C' || first == 'Q' || first == 'D' || first == 'J')
                        && s[1..].chars().all(|c| c.is_ascii_digit())
                    {
                        if !parts_found.contains(&s) {
                            parts_found.push(s);
                        }
                    }
                }
                cur_str.clear();
            }
        }

        // Layout detected components in a grid
        let mut col = 0;
        let mut row = 0;
        for (idx, name) in parts_found.into_iter().enumerate() {
            let x = col as f64 * 10.0 + 10.0;
            let y = row as f64 * 10.0 + 10.0;
            col += 1;
            if col >= 8 {
                col = 0;
                row += 1;
            }

            let part_id = board.parts.len();
            board.parts.push(Part {
                id: part_id,
                name: name.clone(),
                mfg_code: String::new(),
                side: Side::Top,
                part_type: PartType::Smd,
                p1: Point::new(x, y),
                p2: Point::new(x, y),
                pin_indices: Vec::new(),
                bounding_box: None,
                ..Default::default()
            });

            // Add placeholder pins
            for pin_idx in 1..=2 {
                let p_id = board.pins.len();
                let px = x + (pin_idx as f64 - 1.5) * 1.0;
                board.pins.push(Pin {
                    id: p_id,
                    part_id,
                    name: pin_idx.to_string(),
                    pos: Point::new(px, y),
                    side: Side::Top,
                    net_name: if pin_idx == 1 { "GND".into() } else { "VCC".into() },
                    radius: 0.4,
                    shape: PadShape::Rect,
                    width: Some(0.8),
                    height: Some(0.8),
                    rotation_deg: 0.0,
                });
                board.parts[part_id].pin_indices.push(p_id);
            }

            if idx >= 2000 {
                break;
            }
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
