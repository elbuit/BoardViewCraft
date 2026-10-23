//! CPD neutral file parser (`.cad` dialect).
//!
//! Handles CPD3-era neutral CAD files (`# CPD`, `###Board Information`, `###Component Information`, etc.)

use std::collections::BTreeMap;
use crate::board::Board;
use crate::model::{Part, PartType, Pin, Point, Side, Via};
use crate::parser::{FormatParser, ParseError};

pub struct CpdParser;

impl CpdParser {
    pub const fn new() -> Self {
        Self
    }

    pub fn looks_like_cpd(data: &[u8]) -> bool {
        let sample = &data[..data.len().min(4096)];
        let s = String::from_utf8_lossy(sample);
        s.contains("# CPD") || s.contains("###Board Information") || s.contains("###Component Information")
    }

    pub fn parse_content(text: &str) -> Result<Board, ParseError> {
        let mut board = Board::new();

        let mut unit_scale = 1.0; // mils by default
        let mut parts_map: BTreeMap<String, usize> = BTreeMap::new();
        let mut current_net = String::new();

        for (_line_idx, raw_line) in text.lines().enumerate() {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            let toks: Vec<&str> = line.split_whitespace().collect();
            if toks.is_empty() {
                continue;
            }

            match toks[0] {
                "B_UNITS" => {
                    if toks.len() >= 2 {
                        let unit = toks[1].to_ascii_lowercase();
                        if unit.starts_with("inch") {
                            unit_scale = 1000.0;
                        } else if unit.starts_with("mm") {
                            unit_scale = 39.3700787;
                        } else {
                            unit_scale = 1.0;
                        }
                    }
                }
                "COMP" => {
                    // COMP <refdes> <partnum> <geom> <pkg> <x> <y> <side> <rot>
                    if toks.len() >= 8 {
                        let refdes = toks[1].to_string();
                        let side_str = toks[7];
                        let side = if side_str == "1" || side_str.eq_ignore_ascii_case("top") {
                            Side::Top
                        } else {
                            Side::Bottom
                        };

                        if !parts_map.contains_key(&refdes) {
                            let part_id = board.parts.len();
                            board.parts.push(Part {
                                id: part_id,
                                name: refdes.clone(),
                                mfg_code: if toks.len() >= 3 { toks[2].to_string() } else { String::new() },
                                side,
                                part_type: PartType::Smd,
                                p1: Point::ZERO,
                                p2: Point::ZERO,
                                pin_indices: Vec::new(),
                                bounding_box: None,
                            });
                            parts_map.insert(refdes, part_id);
                        }
                    }
                }
                "C_PIN" => {
                    // C_PIN <refdes>-<pin> <x> <y> <code> <side> <rot> <padstack> <net>
                    // or C_PIN <refdes> <pin> ...
                    if toks.len() >= 4 {
                        let pin_ref = toks[1];
                        let (refdes, pin_num) = if let Some(dash) = pin_ref.find('-') {
                            (&pin_ref[..dash], &pin_ref[dash + 1..])
                        } else {
                            (pin_ref, "")
                        };

                        let x_raw: f64 = toks[2].parse().unwrap_or(0.0);
                        let y_raw: f64 = toks[3].parse().unwrap_or(0.0);
                        let x = x_raw * unit_scale;
                        let y = y_raw * unit_scale;

                        let mut net = if toks.len() >= 9 {
                            toks[8].to_string()
                        } else if toks.len() >= 8 {
                            toks[7].to_string()
                        } else {
                            current_net.clone()
                        };
                        if let Some(stripped) = net.strip_prefix('/') {
                            net = stripped.to_string();
                        }

                        let part_id = if let Some(&id) = parts_map.get(refdes) {
                            id
                        } else {
                            let id = board.parts.len();
                            board.parts.push(Part {
                                id,
                                name: refdes.to_string(),
                                mfg_code: String::new(),
                                side: Side::Top,
                                part_type: PartType::Smd,
                                p1: Point::ZERO,
                                p2: Point::ZERO,
                                pin_indices: Vec::new(),
                                bounding_box: None,
                            });
                            parts_map.insert(refdes.to_string(), id);
                            id
                        };

                        let side = board.parts[part_id].side;
                        let pin_id = board.pins.len();
                        board.pins.push(Pin {
                            id: pin_id,
                            part_id,
                            name: pin_num.to_string(),
                            net_name: net,
                            pos: Point::new(x, y),
                            side,
                            radius: 12.0,
                        });
                    }
                }
                "NET" => {
                    // NET <netname>
                    if toks.len() >= 2 {
                        let mut net = toks[1].to_string();
                        if let Some(stripped) = net.strip_prefix('/') {
                            net = stripped.to_string();
                        }
                        current_net = net;
                    }
                }
                "N_VIA" => {
                    // N_VIA <x> <y> ...
                    if toks.len() >= 3 {
                        let x_raw: f64 = toks[1].parse().unwrap_or(0.0);
                        let y_raw: f64 = toks[2].parse().unwrap_or(0.0);
                        let via_id = board.vias.len();
                        board.vias.push(Via {
                            id: via_id,
                            net_name: current_net.clone(),
                            pos: Point::new(x_raw * unit_scale, y_raw * unit_scale),
                            side: Side::Top,
                            radius: 8.0,
                        });
                    }
                }
                _ => {}
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

impl Default for CpdParser {
    fn default() -> Self {
        Self::new()
    }
}

impl FormatParser for CpdParser {
    fn name(&self) -> &'static str {
        "CPD"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["cad", "cpd"]
    }

    fn verify_format(&self, data: &[u8]) -> bool {
        Self::looks_like_cpd(data)
    }

    fn parse(&self, data: &[u8]) -> Result<Board, ParseError> {
        let text = String::from_utf8_lossy(data);
        Self::parse_content(&text)
    }
}
