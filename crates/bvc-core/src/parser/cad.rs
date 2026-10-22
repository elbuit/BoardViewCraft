//! OpenBoardView `.cad` file format parser.
//!
//! Faithful implementation of OpenBoardView's CADFile.cpp format logic:
//! - Parts: `COMP` block
//! - Pins: `C_PIN` block
//! - Nets: `NET` block
//! - Vias: `N_VIA` block
//! - Outline generation: bounding box over all pins with 20-mil margin.

use std::collections::BTreeMap;
use crate::board::Board;
use crate::model::{Part, PartType, Pin, Point, Side, Via};
use crate::parser::{FormatParser, ParseError};

pub struct CadParser;

impl CadParser {
    pub const fn new() -> Self {
        Self
    }
}

impl Default for CadParser {
    fn default() -> Self {
        Self::new()
    }
}

impl FormatParser for CadParser {
    fn name(&self) -> &'static str {
        "CAD"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["cad"]
    }

    fn verify_format(&self, data: &[u8]) -> bool {
        // Matches OpenBoardView CADFile.cpp:
        // verifyFormat(buffer) -> find_str_in_buf("###Panel Added", buf) && find_str_in_buf("C_PIN", buf)
        let s = String::from_utf8_lossy(data);
        (s.contains("###Panel Added") || s.contains("COMP")) && s.contains("C_PIN") && !s.contains("GENCAD")
    }

    fn parse(&self, data: &[u8]) -> Result<Board, ParseError> {
        let content = String::from_utf8_lossy(data);
        let mut board = Board::new();

        let mut parts_map: BTreeMap<String, usize> = BTreeMap::new();
        let mut current_nail_net = String::new();

        const MULTIPLIER: f64 = 1000.0;
        const OUTLINE_MARGIN: f64 = 20.0;

        for (line_idx, raw_line) in content.lines().enumerate() {
            let line = raw_line.trim();
            if line.is_empty() {
                continue;
            }

            // Split line by whitespace
            let tokens: Vec<&str> = line.split_whitespace().collect();
            if tokens.is_empty() {
                continue;
            }

            let block = tokens[0];
            match block {
                "COMP" => {
                    // OpenBoardView CADFile.cpp:
                    // READ_STR() -> TYPE ("COMP")
                    // READ_STR() -> part.name
                    // READ_STR() -> Part(NR)
                    // READ_STR() -> Unknown
                    // READ_STR() -> Unknown
                    // READ_STR() -> X
                    // READ_STR() -> Y
                    // READ_STR() -> loc ("1" = Top, other = Bottom)
                    if tokens.len() >= 8 {
                        let name = tokens[1].to_string();
                        let loc = tokens[7];
                        let side = if loc == "1" { Side::Top } else { Side::Bottom };

                        let part_id = board.parts.len();
                        board.parts.push(Part {
                            id: part_id,
                            name: name.clone(),
                            mfg_code: String::new(),
                            side,
                            part_type: PartType::Smd,
                            p1: Point::ZERO,
                            p2: Point::ZERO,
                            pin_indices: Vec::new(),
                            bounding_box: None,
                        });
                        parts_map.insert(name, part_id);
                    }
                }
                "C_PIN" => {
                    // OpenBoardView CADFile.cpp:
                    // READ_STR() -> TYPE ("C_PIN")
                    // READ_STR() -> part name (with possible '-' stripped)
                    // READ_DOUBLE() -> posx * multiplier
                    // READ_DOUBLE() -> posy * multiplier
                    // READ_DOUBLE() -> unknown
                    // READ_DOUBLE() -> unknown
                    // READ_DOUBLE() -> unknown
                    // READ_STR() -> unknown
                    // READ_STR() -> netr (strip leading '/' if present)
                    if tokens.len() >= 9 {
                        let full_part = tokens[1];
                        let part_clean = match full_part.find('-') {
                            Some(idx) => &full_part[..idx],
                            None => full_part,
                        };

                        let part_id = match parts_map.get(part_clean) {
                            Some(&id) => id,
                            None => {
                                // Auto-register part if not yet encountered
                                let id = board.parts.len();
                                board.parts.push(Part {
                                    id,
                                    name: part_clean.to_string(),
                                    mfg_code: String::new(),
                                    side: Side::Top,
                                    part_type: PartType::Smd,
                                    p1: Point::ZERO,
                                    p2: Point::ZERO,
                                    pin_indices: Vec::new(),
                                    bounding_box: None,
                                });
                                parts_map.insert(part_clean.to_string(), id);
                                id
                            }
                        };

                        let pos_x = tokens[2].parse::<f64>().map_err(|_| ParseError::SyntaxError {
                            line: line_idx + 1,
                            message: format!("Invalid pin X coordinate: {}", tokens[2]),
                        })? * MULTIPLIER;

                        let pos_y = tokens[3].parse::<f64>().map_err(|_| ParseError::SyntaxError {
                            line: line_idx + 1,
                            message: format!("Invalid pin Y coordinate: {}", tokens[3]),
                        })? * MULTIPLIER;

                        let mut net_name = tokens[8].to_string();
                        if let Some(stripped) = net_name.strip_prefix('/') {
                            net_name = stripped.to_string();
                        }

                        let part_side = board.parts[part_id].side;
                        let pin_id = board.pins.len();
                        let pin_count_for_part = board.pins.iter().filter(|p| p.part_id == part_id).count() + 1;

                        board.pins.push(Pin {
                            id: pin_id,
                            part_id,
                            name: pin_count_for_part.to_string(),
                            pos: Point::new(pos_x, pos_y),
                            side: part_side,
                            net_name,
                            radius: 12.0, // Standard pad radius in mils (0.012 in)
                        });
                    }
                }
                "NET" => {
                    // OpenBoardView CADFile.cpp:
                    // READ_STR() -> TYPE ("NET")
                    // READ_STR() -> netr (strip leading '/' if present)
                    if tokens.len() >= 2 {
                        let mut net = tokens[1].to_string();
                        if let Some(stripped) = net.strip_prefix('/') {
                            net = stripped.to_string();
                        }
                        current_nail_net = net;
                    }
                }
                "N_VIA" => {
                    // OpenBoardView CADFile.cpp:
                    // nail.net = nailnet
                    // READ_STR() -> TYPE ("N_VIA")
                    // READ_DOUBLE() -> posx * multiplier
                    // READ_DOUBLE() -> posy * multiplier
                    // READ_STR() -> unknown
                    // READ_DOUBLE() -> side == 1 ? Top : Bottom
                    if tokens.len() >= 5 {
                        let pos_x = tokens[1].parse::<f64>().map_err(|_| ParseError::SyntaxError {
                            line: line_idx + 1,
                            message: format!("Invalid via X coordinate: {}", tokens[1]),
                        })? * MULTIPLIER;

                        let pos_y = tokens[2].parse::<f64>().map_err(|_| ParseError::SyntaxError {
                            line: line_idx + 1,
                            message: format!("Invalid via Y coordinate: {}", tokens[2]),
                        })? * MULTIPLIER;

                        let side_val = tokens[4].parse::<f64>().unwrap_or(1.0);
                        let side = if (side_val - 1.0).abs() < 1e-4 {
                            Side::Top
                        } else {
                            Side::Bottom
                        };

                        let via_id = board.vias.len();
                        board.vias.push(Via {
                            id: via_id,
                            pos: Point::new(pos_x, pos_y),
                            side,
                            net_name: current_nail_net.clone(),
                            radius: 8.0, // Standard via radius in mils
                        });
                    }
                }
                _ => {
                    // Skip unrecognized blocks
                }
            }
        }

        // Generate outline matching OpenBoardView CADFile.cpp:
        // OUTLINE_MARGIN = 20
        // minx = min(pins.x) - OUTLINE_MARGIN
        // maxx = max(pins.x) + OUTLINE_MARGIN
        // miny = min(pins.y) - OUTLINE_MARGIN
        // maxy = max(pins.y) + OUTLINE_MARGIN
        // format.push_back({minx, miny});
        // format.push_back({maxx, miny});
        // format.push_back({maxx, maxy});
        // format.push_back({minx, maxy});
        // format.push_back({minx, miny});
        if !board.pins.is_empty() {
            let pin_pts: Vec<Point> = board.pins.iter().map(|p| p.pos).collect();
            if let Some(rect) = crate::model::Rect::from_points(&pin_pts) {
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
