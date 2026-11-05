//! Samsung CAD, Mentor Neutral, and Tebo ICT parser implementations.

use crate::board::Board;
use crate::model::{PadShape, Part, PartType, Pin, Point, Side, Via};
use crate::parser::{FormatParser, ParseError};
use std::collections::BTreeMap;

// ============================================================================
// Samsung CAD (.cad)
// ============================================================================

pub struct SamsungCadParser;

impl SamsungCadParser {
    pub const fn new() -> Self {
        Self
    }
}

impl Default for SamsungCadParser {
    fn default() -> Self {
        Self::new()
    }
}

impl FormatParser for SamsungCadParser {
    fn name(&self) -> &'static str {
        "SamsungCAD"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["cad"]
    }

    fn verify_format(&self, data: &[u8]) -> bool {
        let sample = if data.len() > 4096 { &data[..4096] } else { data };
        let text = String::from_utf8_lossy(sample);
        text.contains("###Panel Added") && text.contains("C_PIN")
    }

    fn parse(&self, data: &[u8]) -> Result<Board, ParseError> {
        let text = String::from_utf8_lossy(data);
        let mut board = Board::new();
        let mut parts_map: BTreeMap<String, usize> = BTreeMap::new();
        let mut current_via_net = String::new();

        const UNIT_SCALE: f64 = 1000.0;

        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            let tokens: Vec<&str> = trimmed.split_whitespace().collect();
            if tokens.is_empty() {
                continue;
            }

            match tokens[0] {
                "COMP" => {
                    // COMP <name> <part no.> <?> <?> <X> <Y> <side> [?]
                    if tokens.len() >= 8 {
                        let name = tokens[1].to_string();
                        let side = if tokens[7] == "1" { Side::Top } else { Side::Bottom };
                        let part_id = board.parts.len();
                        parts_map.insert(name.clone(), part_id);
                        board.parts.push(Part {
                            id: part_id,
                            name,
                            mfg_code: tokens[2].to_string(),
                            side,
                            part_type: PartType::Smd,
                            p1: Point::ZERO,
                            p2: Point::ZERO,
                            pin_indices: Vec::new(),
                            bounding_box: None,
                            ..Default::default()
                        });
                    }
                }
                "C_PIN" => {
                    // C_PIN <REF-PIN> <X> <Y> <?> <?> <?> <?> [net]
                    if tokens.len() >= 4 {
                        let token = tokens[1];
                        let mut owner_name = token;
                        let mut pin_number = "";

                        if let Some(dash) = token.rfind('-') {
                            owner_name = &token[..dash];
                            pin_number = &token[dash + 1..];
                        }

                        let x = tokens[2].parse::<f64>().unwrap_or(0.0) * UNIT_SCALE;
                        let y = tokens[3].parse::<f64>().unwrap_or(0.0) * UNIT_SCALE;
                        let net = if tokens.len() >= 9 {
                            tokens[8].trim_start_matches('/').to_string()
                        } else {
                            String::new()
                        };

                        let part_id = if let Some(&id) = parts_map.get(owner_name) {
                            id
                        } else {
                            let id = board.parts.len();
                            parts_map.insert(owner_name.to_string(), id);
                            board.parts.push(Part {
                                id,
                                name: owner_name.to_string(),
                                mfg_code: String::new(),
                                side: Side::Top,
                                part_type: PartType::Smd,
                                p1: Point::new(x, y),
                                p2: Point::new(x, y),
                                pin_indices: Vec::new(),
                                bounding_box: None,
                                ..Default::default()
                            });
                            id
                        };

                        let side = board.parts[part_id].side;
                        let pin_idx = board.pins.len();
                        board.pins.push(Pin {
                            id: pin_idx,
                            part_id,
                            name: pin_number.to_string(),
                            pos: Point::new(x, y),
                            side,
                            net_name: net,
                            radius: 15.0,
                            shape: PadShape::Round,
                            width: None,
                            height: None,
                            rotation_deg: 0.0,
                        });
                        board.parts[part_id].pin_indices.push(pin_idx);
                    }
                }
                "NET" => {
                    if tokens.len() >= 2 {
                        current_via_net = tokens[1].trim_start_matches('/').to_string();
                    }
                }
                "N_VIA" => {
                    if tokens.len() >= 4 {
                        let x = tokens[1].parse::<f64>().unwrap_or(0.0) * UNIT_SCALE;
                        let y = tokens[2].parse::<f64>().unwrap_or(0.0) * UNIT_SCALE;
                        let side = if tokens.len() >= 5 && tokens[4] == "1" { Side::Top } else { Side::Bottom };
                        let via_id = board.vias.len();
                        board.vias.push(Via {
                            id: via_id,
                            pos: Point::new(x, y),
                            side,
                            net_name: current_via_net.clone(),
                            radius: 12.0,
                        });
                    }
                }
                _ => {}
            }
        }

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

// ============================================================================
// Mentor Neutral (.neu, .cad)
// ============================================================================

pub struct MentorNeutralParser;

impl MentorNeutralParser {
    pub const fn new() -> Self {
        Self
    }
}

impl Default for MentorNeutralParser {
    fn default() -> Self {
        Self::new()
    }
}

impl FormatParser for MentorNeutralParser {
    fn name(&self) -> &'static str {
        "MentorNeutral"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["neu", "cad"]
    }

    fn verify_format(&self, data: &[u8]) -> bool {
        let sample = if data.len() > 4096 { &data[..4096] } else { data };
        let text = String::from_utf8_lossy(sample);
        text.contains("B_UNITS") || (text.contains("BOARD") && text.contains("C_PIN") && text.contains("COMP"))
    }

    fn parse(&self, data: &[u8]) -> Result<Board, ParseError> {
        let text = String::from_utf8_lossy(data);
        let mut board = Board::new();
        let mut parts_map: BTreeMap<String, usize> = BTreeMap::new();
        let mut unit_scale = 1.0;

        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            let tokens: Vec<&str> = trimmed.split_whitespace().collect();
            if tokens.is_empty() {
                continue;
            }

            match tokens[0] {
                "B_UNITS" => {
                    if tokens.len() >= 2 {
                        let unit = tokens[1].to_ascii_lowercase();
                        if unit.starts_with("inch") || unit.starts_with("in") {
                            unit_scale = 25.4;
                        } else if unit.starts_with("mil") || unit.starts_with("thou") {
                            unit_scale = 0.0254;
                        } else if unit.starts_with("mm") {
                            unit_scale = 1.0;
                        }
                    }
                }
                "COMP" => {
                    if tokens.len() >= 5 {
                        let refdes = tokens[1].to_string();
                        let side = if tokens.len() >= 8 {
                            let s = tokens[7].to_ascii_lowercase();
                            if s.contains('b') { Side::Bottom } else { Side::Top }
                        } else {
                            Side::Top
                        };

                        let part_id = board.parts.len();
                        parts_map.insert(refdes.clone(), part_id);
                        board.parts.push(Part {
                            id: part_id,
                            name: refdes,
                            mfg_code: tokens[2].to_string(),
                            side,
                            part_type: PartType::Smd,
                            p1: Point::ZERO,
                            p2: Point::ZERO,
                            pin_indices: Vec::new(),
                            bounding_box: None,
                            ..Default::default()
                        });
                    }
                }
                "C_PIN" => {
                    if tokens.len() >= 9 {
                        let ref_pin = tokens[1];
                        let (refdes, pin_no) = if let Some(dash) = ref_pin.find('-') {
                            (&ref_pin[..dash], &ref_pin[dash + 1..])
                        } else {
                            (ref_pin, "")
                        };

                        let x = tokens[2].parse::<f64>().unwrap_or(0.0) * unit_scale;
                        let y = tokens[3].parse::<f64>().unwrap_or(0.0) * unit_scale;
                        let side_str = tokens[5].to_ascii_lowercase();
                        let side = if side_str.contains('b') { Side::Bottom } else { Side::Top };
                        let net = if tokens[8] == "$NONE$" { String::new() } else { tokens[8].to_string() };

                        let part_id = if let Some(&id) = parts_map.get(refdes) {
                            id
                        } else {
                            let id = board.parts.len();
                            parts_map.insert(refdes.to_string(), id);
                            board.parts.push(Part {
                                id,
                                name: refdes.to_string(),
                                mfg_code: String::new(),
                                side,
                                part_type: PartType::Smd,
                                p1: Point::new(x, y),
                                p2: Point::new(x, y),
                                pin_indices: Vec::new(),
                                bounding_box: None,
                                ..Default::default()
                            });
                            id
                        };

                        let p_idx = board.pins.len();
                        board.pins.push(Pin {
                            id: p_idx,
                            part_id,
                            name: pin_no.to_string(),
                            pos: Point::new(x, y),
                            side,
                            net_name: net,
                            radius: 0.35,
                            shape: PadShape::Round,
                            width: None,
                            height: None,
                            rotation_deg: 0.0,
                        });
                        board.parts[part_id].pin_indices.push(p_idx);
                    }
                }
                _ => {}
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

// ============================================================================
// Tebo ICT (.ict)
// ============================================================================

pub struct TeboIctParser;

impl TeboIctParser {
    pub const fn new() -> Self {
        Self
    }
}

impl Default for TeboIctParser {
    fn default() -> Self {
        Self::new()
    }
}

impl FormatParser for TeboIctParser {
    fn name(&self) -> &'static str {
        "TeboICT"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["ict"]
    }

    fn verify_format(&self, data: &[u8]) -> bool {
        let sample = if data.len() > 1024 { &data[..1024] } else { data };
        let text = String::from_utf8_lossy(sample);
        text.contains("!Tebo-ict")
    }

    fn parse(&self, data: &[u8]) -> Result<Board, ParseError> {
        let text = String::from_utf8_lossy(data);
        let mut board = Board::new();
        let mut parts_map: BTreeMap<String, usize> = BTreeMap::new();

        let mut in_outline = false;
        const INCH_TO_MM: f64 = 25.4;

        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('!') {
                continue;
            }

            if trimmed == "OUTLINE" {
                in_outline = true;
                continue;
            }

            if in_outline {
                let clean = trimmed.trim_end_matches(';');
                let coords: Vec<&str> = clean.split(',').map(|s| s.trim()).collect();
                if coords.len() >= 2 {
                    if let (Ok(x), Ok(y)) = (coords[0].parse::<f64>(), coords[1].parse::<f64>()) {
                        board.outline_polygon.push(Point::new(x * INCH_TO_MM, y * INCH_TO_MM));
                    }
                }
                if trimmed.ends_with(';') {
                    in_outline = false;
                }
                continue;
            }

            if trimmed.ends_with(';') && trimmed.contains(',') {
                let clean = trimmed.trim_end_matches(';').trim();
                let parts: Vec<&str> = clean.split_whitespace().collect();
                if parts.len() >= 3 {
                    let xy_str = parts[0];
                    let xy: Vec<&str> = xy_str.split(',').collect();
                    if xy.len() == 2 {
                        if let (Ok(x), Ok(y)) = (xy[0].parse::<f64>(), xy[1].parse::<f64>()) {
                            let ref_pin = parts[1];
                            let (refdes, pin_no) = if let Some(dot) = ref_pin.find('.') {
                                (&ref_pin[..dot], &ref_pin[dot + 1..])
                            } else {
                                (ref_pin, "")
                            };

                            let side = if clean.contains("TOP") { Side::Top } else { Side::Bottom };

                            let part_id = if let Some(&id) = parts_map.get(refdes) {
                                id
                            } else {
                                let id = board.parts.len();
                                parts_map.insert(refdes.to_string(), id);
                                board.parts.push(Part {
                                    id,
                                    name: refdes.to_string(),
                                    mfg_code: String::new(),
                                    side,
                                    part_type: PartType::Smd,
                                    p1: Point::new(x * INCH_TO_MM, y * INCH_TO_MM),
                                    p2: Point::new(x * INCH_TO_MM, y * INCH_TO_MM),
                                    pin_indices: Vec::new(),
                                    bounding_box: None,
                                    ..Default::default()
                                });
                                id
                            };

                            let p_idx = board.pins.len();
                            board.pins.push(Pin {
                                id: p_idx,
                                part_id,
                                name: pin_no.to_string(),
                                pos: Point::new(x * INCH_TO_MM, y * INCH_TO_MM),
                                side,
                                net_name: String::new(),
                                radius: 0.35,
                                shape: PadShape::Round,
                                width: None,
                                height: None,
                                rotation_deg: 0.0,
                            });
                            board.parts[part_id].pin_indices.push(p_idx);
                        }
                    }
                }
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
