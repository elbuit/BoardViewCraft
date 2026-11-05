//! BV2 and CSV/TSV pin list parser implementation.

use crate::board::Board;
use crate::model::{PadShape, Part, PartType, Pin, Point, Rect, Side};
use crate::parser::{FormatParser, ParseError};
use std::collections::{BTreeMap, HashSet};

/// CSV Tokenizer handling RFC-4180 quotes, commas, tabs, and semicolons.
pub struct CsvRecordReader;

impl CsvRecordReader {
    pub fn parse_line(line: &str, delimiter: char) -> Vec<String> {
        let mut fields = Vec::new();
        let mut cur = String::new();
        let mut in_quotes = false;
        let mut chars = line.chars().peekable();

        while let Some(c) = chars.next() {
            if in_quotes {
                if c == '"' {
                    if chars.peek() == Some(&'"') {
                        chars.next();
                        cur.push('"');
                    } else {
                        in_quotes = false;
                    }
                } else {
                    cur.push(c);
                }
            } else if c == '"' {
                in_quotes = true;
            } else if c == delimiter {
                fields.push(cur.trim().to_string());
                cur = String::new();
            } else {
                cur.push(c);
            }
        }
        fields.push(cur.trim().to_string());
        fields
    }
}

// ============================================================================
// BV2 Parser
// ============================================================================

pub struct Bv2Parser;

impl Bv2Parser {
    pub const fn new() -> Self {
        Self
    }
}

impl Default for Bv2Parser {
    fn default() -> Self {
        Self::new()
    }
}

impl FormatParser for Bv2Parser {
    fn name(&self) -> &'static str {
        "BV2"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["bv2"]
    }

    fn verify_format(&self, data: &[u8]) -> bool {
        let sample = if data.len() > 1024 { &data[..1024] } else { data };
        let text = String::from_utf8_lossy(sample);
        text.contains("#Layout#")
    }

    fn parse(&self, data: &[u8]) -> Result<Board, ParseError> {
        let text = String::from_utf8_lossy(data);
        let mut board = Board::new();

        #[derive(PartialEq)]
        enum Section {
            None,
            Layout,
            Nail,
            Pin,
        }

        let mut section = Section::None;
        let mut parts_map: BTreeMap<String, usize> = BTreeMap::new();
        let mut seen_pins: HashSet<String> = HashSet::new();

        const INCH_TO_MM: f64 = 25.4;

        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            if trimmed == "#Layout#" {
                section = Section::Layout;
                continue;
            } else if trimmed == "#Nail#" {
                section = Section::Nail;
                continue;
            } else if trimmed == "#Pin#" {
                section = Section::Pin;
                continue;
            }

            let fields = CsvRecordReader::parse_line(trimmed, ',');
            if fields.is_empty() {
                continue;
            }

            match section {
                Section::Layout => {
                    if fields[0].eq_ignore_ascii_case("x") {
                        continue;
                    }
                    if fields.len() >= 2 {
                        if let (Ok(x), Ok(y)) = (fields[0].parse::<f64>(), fields[1].parse::<f64>()) {
                            board.outline_polygon.push(Point::new(x * INCH_TO_MM, y * INCH_TO_MM));
                        }
                    }
                }
                Section::Nail => {
                    if fields[0].eq_ignore_ascii_case("nail") {
                        continue;
                    }
                    if fields.len() >= 8 {
                        let nail_id = &fields[0];
                        let x = fields[1].parse::<f64>().unwrap_or(0.0) * INCH_TO_MM;
                        let y = fields[2].parse::<f64>().unwrap_or(0.0) * INCH_TO_MM;
                        let side_str = fields[5].to_uppercase();
                        let side = if side_str.contains('B') { Side::Bottom } else { Side::Top };
                        let net_name = fields[7].trim().to_string();

                        let part_name = format!("TP:{}", nail_id.trim_start_matches('$'));
                        let part_id = if let Some(&id) = parts_map.get(&part_name) {
                            id
                        } else {
                            let id = board.parts.len();
                            parts_map.insert(part_name.clone(), id);
                            board.parts.push(Part {
                                id,
                                name: part_name.clone(),
                                mfg_code: String::new(),
                                side,
                                part_type: PartType::Smd,
                                p1: Point::new(x, y),
                                p2: Point::new(x, y),
                                pin_indices: Vec::new(),
                                bounding_box: Some(Rect {
                                    min_x: x - 0.5,
                                    min_y: y - 0.5,
                                    max_x: x + 0.5,
                                    max_y: y + 0.5,
                                }),
                                ..Default::default()
                            });
                            id
                        };

                        let pin_idx = board.pins.len();
                        board.pins.push(Pin {
                            id: pin_idx,
                            part_id,
                            name: nail_id.clone(),
                            pos: Point::new(x, y),
                            side,
                            net_name,
                            radius: 0.4,
                            shape: PadShape::Round,
                            width: None,
                            height: None,
                            rotation_deg: 0.0,
                        });
                        board.parts[part_id].pin_indices.push(pin_idx);
                    }
                }
                Section::Pin => {
                    if fields[0].eq_ignore_ascii_case("part") {
                        continue;
                    }
                    if fields.len() >= 8 {
                        let part_name = fields[0].clone();
                        let side_str = fields[1].to_uppercase();
                        let side = if side_str.contains('B') { Side::Bottom } else { Side::Top };
                        let pin_number = if fields[3].is_empty() { fields[2].clone() } else { fields[3].clone() };
                        let x = fields[4].parse::<f64>().unwrap_or(0.0) * INCH_TO_MM;
                        let y = fields[5].parse::<f64>().unwrap_or(0.0) * INCH_TO_MM;
                        let net_name = fields[7].trim().to_string();

                        let pin_key = format!("{}:{}:{}", part_name, side_str, pin_number);
                        if seen_pins.contains(&pin_key) {
                            continue;
                        }
                        seen_pins.insert(pin_key);

                        let part_id = if let Some(&id) = parts_map.get(&part_name) {
                            if board.parts[id].side != side {
                                board.parts[id].side = Side::Both;
                            }
                            id
                        } else {
                            let id = board.parts.len();
                            parts_map.insert(part_name.clone(), id);
                            board.parts.push(Part {
                                id,
                                name: part_name.clone(),
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

                        let pin_idx = board.pins.len();
                        board.pins.push(Pin {
                            id: pin_idx,
                            part_id,
                            name: pin_number,
                            pos: Point::new(x, y),
                            side,
                            net_name,
                            radius: 0.3,
                            shape: PadShape::Round,
                            width: None,
                            height: None,
                            rotation_deg: 0.0,
                        });
                        board.parts[part_id].pin_indices.push(pin_idx);
                    }
                }
                Section::None => {}
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
// Generic CSV / TSV Pinlist Parser
// ============================================================================

pub struct PinListParser;

impl PinListParser {
    pub const fn new() -> Self {
        Self
    }
}

impl Default for PinListParser {
    fn default() -> Self {
        Self::new()
    }
}

impl FormatParser for PinListParser {
    fn name(&self) -> &'static str {
        "PinList"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["csv", "tsv", "txt"]
    }

    fn verify_format(&self, data: &[u8]) -> bool {
        let sample = if data.len() > 2048 { &data[..2048] } else { data };
        let text = String::from_utf8_lossy(sample);
        for line in text.lines().take(5) {
            let lower = line.to_ascii_lowercase();
            if (lower.contains("refdes") || lower.contains("component") || lower.contains("part"))
                && (lower.contains("pin") || lower.contains("pad"))
                && lower.contains('x')
                && lower.contains('y')
            {
                return true;
            }
        }
        false
    }

    fn parse(&self, data: &[u8]) -> Result<Board, ParseError> {
        let text = String::from_utf8_lossy(data);
        let mut board = Board::new();
        let mut parts_map: BTreeMap<String, usize> = BTreeMap::new();

        let first_lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).take(3).collect();
        if first_lines.is_empty() {
            return Err(ParseError::Other("Empty pin list file".into()));
        }

        let delimiter = if first_lines[0].contains('\t') {
            '\t'
        } else if first_lines[0].contains(';') {
            ';'
        } else {
            ','
        };

        let header = CsvRecordReader::parse_line(first_lines[0], delimiter);
        let mut ref_col = None;
        let mut pin_col = None;
        let mut net_col = None;
        let mut x_col = None;
        let mut y_col = None;
        let mut side_col = None;
        let mut unit_scale = 1.0;

        for (idx, col_name) in header.iter().enumerate() {
            let lower = col_name.to_ascii_lowercase();
            if lower.contains("mil") || lower.contains("thou") {
                unit_scale = 0.0254;
            } else if lower.contains("inch") || lower.contains(" in") {
                unit_scale = 25.4;
            }

            if ref_col.is_none() && (lower == "refdes" || lower == "ref" || lower == "part" || lower == "component") {
                ref_col = Some(idx);
            } else if pin_col.is_none() && (lower == "pin" || lower == "pinno" || lower == "pad" || lower == "pin_number") {
                pin_col = Some(idx);
            } else if net_col.is_none() && (lower == "net" || lower == "netname" || lower == "signal") {
                net_col = Some(idx);
            } else if x_col.is_none() && (lower.starts_with('x') || lower == "posx" || lower == "coordx") {
                x_col = Some(idx);
            } else if y_col.is_none() && (lower.starts_with('y') || lower == "posy" || lower == "coordy") {
                y_col = Some(idx);
            } else if side_col.is_none() && (lower == "side" || lower == "layer" || lower == "tb") {
                side_col = Some(idx);
            }
        }

        let ref_idx = ref_col.ok_or_else(|| ParseError::SyntaxError { line: 1, message: "Missing refdes column".into() })?;
        let pin_idx = pin_col.ok_or_else(|| ParseError::SyntaxError { line: 1, message: "Missing pin column".into() })?;
        let x_idx = x_col.ok_or_else(|| ParseError::SyntaxError { line: 1, message: "Missing X column".into() })?;
        let y_idx = y_col.ok_or_else(|| ParseError::SyntaxError { line: 1, message: "Missing Y column".into() })?;

        for (_line_num, line) in text.lines().enumerate().skip(1) {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let fields = CsvRecordReader::parse_line(trimmed, delimiter);
            let max_needed = ref_idx.max(pin_idx).max(x_idx).max(y_idx);
            if fields.len() <= max_needed {
                continue;
            }

            let part_name = fields[ref_idx].clone();
            let pin_number = fields[pin_idx].clone();
            let x = match fields[x_idx].replace(',', ".").parse::<f64>() {
                Ok(v) => v * unit_scale,
                Err(_) => continue,
            };
            let y = match fields[y_idx].replace(',', ".").parse::<f64>() {
                Ok(v) => v * unit_scale,
                Err(_) => continue,
            };
            let net = net_col.and_then(|idx| fields.get(idx)).cloned().unwrap_or_default();
            let side = side_col.and_then(|idx| fields.get(idx)).map(|s| {
                let sl = s.to_ascii_lowercase();
                if sl.contains('b') || sl.contains("bot") { Side::Bottom } else { Side::Top }
            }).unwrap_or(Side::Top);

            let part_id = if let Some(&id) = parts_map.get(&part_name) {
                if board.parts[id].side != side {
                    board.parts[id].side = Side::Both;
                }
                id
            } else {
                let id = board.parts.len();
                parts_map.insert(part_name.clone(), id);
                board.parts.push(Part {
                    id,
                    name: part_name.clone(),
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
                name: pin_number,
                pos: Point::new(x, y),
                side,
                net_name: net,
                radius: 0.3,
                shape: PadShape::Round,
                width: None,
                height: None,
                rotation_deg: 0.0,
            });
            board.parts[part_id].pin_indices.push(p_idx);
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
