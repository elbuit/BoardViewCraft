//! OpenBoardView `.cad` file format parser and umbrella dispatcher.
//!
//! Variants:
//! - FZ-zlib (`0x78` offset 4)
//! - CPD neutral (`# CPD` or `###Board Information`)
//! - BRD2 fallback (`BRDOUT:`)
//! - Test_Link fallback (`var_data:`)
//! - Legacy OpenBoardView CAD (`COMP`, `C_PIN`, `NET`, `N_VIA`) with 1000.0 multiplier.

use std::collections::BTreeMap;
use crate::board::Board;
use crate::model::{Part, PartType, Pin, Point, Side, Via};
use crate::parser::brd2::Brd2Parser;
use crate::parser::cpd::CpdParser;
use crate::parser::fz_zlib::FzZlibParser;
use crate::parser::test_link::{TestLinkEngine, CAD_TEST_LINK_MARKERS};
use crate::parser::{FormatParser, ParseError};

pub struct CadParser;

impl CadParser {
    pub const fn new() -> Self {
        Self
    }

    pub fn parse_openboardview_cad(text: &str) -> Result<Board, ParseError> {
        let mut board = Board::new();
        let mut parts_map: BTreeMap<String, usize> = BTreeMap::new();
        let mut current_nail_net = String::new();

        const MULTIPLIER: f64 = 1000.0;
        const OUTLINE_MARGIN: f64 = 20.0;

        for (line_idx, raw_line) in text.lines().enumerate() {
            let line = raw_line.trim();
            if line.is_empty() {
                continue;
            }

            let tokens: Vec<&str> = line.split_whitespace().collect();
            if tokens.is_empty() {
                continue;
            }

            let block = tokens[0];
            match block {
                "COMP" => {
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
                    if tokens.len() >= 9 {
                        let full_part = tokens[1];
                        let part_clean = match full_part.find('-') {
                            Some(idx) => &full_part[..idx],
                            None => full_part,
                        };

                        let part_id = match parts_map.get(part_clean) {
                            Some(&id) => id,
                            None => {
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
                            radius: 12.0,
                        });
                    }
                }
                "NET" => {
                    if tokens.len() >= 2 {
                        let mut net = tokens[1].to_string();
                        if let Some(stripped) = net.strip_prefix('/') {
                            net = stripped.to_string();
                        }
                        current_nail_net = net;
                    }
                }
                "N_VIA" => {
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
                            radius: 8.0,
                        });
                    }
                }
                _ => {}
            }
        }

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
        if FzZlibParser::looks_like_fz_zlib(data) {
            return true;
        }
        if CpdParser::looks_like_cpd(data) {
            return true;
        }
        let sample = &data[..data.len().min(4096)];
        let s = String::from_utf8_lossy(sample);
        if s.contains("BRDOUT:") {
            return true;
        }
        if s.contains("var_data:") && (s.contains("Format:") || s.contains("Parts:") || s.contains("Pins:")) {
            return true;
        }
        (s.contains("###Panel Added") || s.contains("COMP")) && s.contains("C_PIN") && !s.contains("GENCAD")
    }

    fn parse(&self, data: &[u8]) -> Result<Board, ParseError> {
        // 1. Check FZ-zlib
        if FzZlibParser::looks_like_fz_zlib(data) {
            return FzZlibParser::new().parse(data);
        }

        // 2. Check CPD neutral
        if CpdParser::looks_like_cpd(data) {
            return CpdParser::new().parse(data);
        }

        let s = String::from_utf8_lossy(data);

        // 3. Check BRD2
        if s.contains("BRDOUT:") {
            return Brd2Parser::parse_content(&s);
        }

        // 4. Check Test_Link ASCII
        if s.contains("var_data:") {
            return TestLinkEngine::parse_text(&s, &CAD_TEST_LINK_MARKERS);
        }

        // 5. OpenBoardView legacy CAD format
        Self::parse_openboardview_cad(&s)
    }
}
