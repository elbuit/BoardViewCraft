//! Unified Test_Link ASCII boardview engine and dialect definitions.
//!
//! Handles classic OpenBoardView `.brd`, `.cad` (Test_Link shape), `.bv`, `.bvr` (ASCII fallback),
//! `.cst`, `.gr`, `.f2b`, TSICT `.asc` (combined), and post-decode streams (.bdv, .tvw).

use std::collections::BTreeMap;
use crate::board::Board;
use crate::model::{Part, PartType, Pin, Point, Side, Via};
use crate::parser::ParseError;

/// Dialect marker configuration for Test_Link-shape formats.
#[derive(Debug, Clone)]
pub struct DialectMarkers {
    /// Prefix for the 4-integer count header (outline, parts, pins, nails).
    /// If empty, counts are inferred from blocks directly.
    pub header_count_marker: &'static str,
    pub outline_markers: &'static [&'static str],
    pub parts_markers: &'static [&'static str],
    pub pins_markers: &'static [&'static str],
    pub nails_markers: &'static [&'static str],
    pub all_block_markers: &'static [&'static str],
}

impl Default for DialectMarkers {
    fn default() -> Self {
        Self {
            header_count_marker: "var_data:",
            outline_markers: &["Format:", "format:", "FORMAT:"],
            parts_markers: &["Parts:", "Pins1:", "PARTS:", "Components:", "[Components]", "[Parts]"],
            pins_markers: &["Pins:", "Pins2:", "PINS:", "[Pins]"],
            nails_markers: &["Nails:", "NAILS:", "TestPoints:", "[Nails]", "[TestPoints]"],
            all_block_markers: &[
                "Format:", "format:", "FORMAT:", "Outline:", "[Format]", "[Outline]",
                "Parts:", "Pins1:", "PARTS:", "Components:", "[Components]", "[Parts]",
                "Pins:", "Pins2:", "PINS:", "[Pins]",
                "Nails:", "NAILS:", "TestPoints:", "[Nails]", "[TestPoints]",
            ],
        }
    }
}

pub const CST_MARKERS: DialectMarkers = DialectMarkers {
    header_count_marker: "",
    outline_markers: &["[Format]", "[Outline]", "Format:"],
    parts_markers: &["[Components]", "[Parts]", "Components:", "Parts:"],
    pins_markers: &["[Pins]", "Pins:"],
    nails_markers: &["[Nails]", "[TestPoints]", "Nails:"],
    all_block_markers: &[
        "[Format]", "[Outline]", "[Components]", "[Parts]", "[Pins]", "[Nails]", "[TestPoints]",
        "Format:", "Components:", "Parts:", "Pins:", "Nails:",
    ],
};

pub const GR_MARKERS: DialectMarkers = DialectMarkers {
    header_count_marker: "var_data:",
    outline_markers: &["Format:", "format:", "FORMAT:"],
    parts_markers: &["Components:", "Parts:", "PARTS:"],
    pins_markers: &["Pins:", "Pins2:", "PINS:"],
    nails_markers: &["TestPoints:", "Nails:", "NAILS:"],
    all_block_markers: &[
        "Format:", "format:", "FORMAT:", "Components:", "Parts:", "PARTS:", "Pins:", "Pins2:", "PINS:", "TestPoints:", "Nails:", "NAILS:",
    ],
};

pub const F2B_MARKERS: DialectMarkers = DialectMarkers {
    header_count_marker: "var_data:",
    outline_markers: &["Format:", "Outline:", "format:"],
    parts_markers: &["Parts:", "Components:", "PARTS:"],
    pins_markers: &["Pins:", "PINS:"],
    nails_markers: &["Nails:", "TestPoints:", "NAILS:"],
    all_block_markers: &[
        "Format:", "Outline:", "format:", "Parts:", "Components:", "PARTS:", "Pins:", "PINS:", "Nails:", "TestPoints:", "NAILS:", "Annotations:",
    ],
};

pub const ASC_MARKERS: DialectMarkers = DialectMarkers {
    header_count_marker: "var_data:",
    outline_markers: &["Format:", "Outline:", "format:"],
    parts_markers: &["Parts:", "Components:", "PARTS:"],
    pins_markers: &["Pins:", "PINS:"],
    nails_markers: &["Nails:", "TestPoints:", "NAILS:"],
    all_block_markers: &[
        "Format:", "Outline:", "format:", "Parts:", "Components:", "PARTS:", "Pins:", "PINS:", "Nails:", "TestPoints:", "NAILS:",
    ],
};

pub const CAD_TEST_LINK_MARKERS: DialectMarkers = DialectMarkers {
    header_count_marker: "var_data:",
    outline_markers: &["Format:", "FORMAT:", "format:"],
    parts_markers: &["Parts:", "PARTS:", "Pins1:", "Components:"],
    pins_markers: &["Pins:", "PINS:", "Pins2:"],
    nails_markers: &["Nails:", "NAILS:"],
    all_block_markers: &[
        "Format:", "FORMAT:", "format:", "Parts:", "PARTS:", "Pins1:", "Components:", "Pins:", "PINS:", "Pins2:", "Nails:", "NAILS:",
    ],
};

/// Heuristic check if raw bytes are mostly non-printable binary data (>30%).
pub fn looks_like_binary(raw: &[u8], threshold: f64) -> bool {
    if raw.is_empty() {
        return false;
    }
    let sample = &raw[..raw.len().min(2048)];
    let printable = sample.iter().filter(|&&b| (32..=126).contains(&b) || b == 9 || b == 10 || b == 13).count();
    (1.0 - (printable as f64 / sample.len() as f64)) > threshold
}

pub struct TestLinkEngine;

impl TestLinkEngine {
    pub fn parse_text(text: &str, markers: &DialectMarkers) -> Result<Board, ParseError> {
        let lines: Vec<&str> = text.lines().map(|l| l.trim()).filter(|l| !l.is_empty()).collect();
        if lines.is_empty() {
            return Err(ParseError::SyntaxError {
                line: 0,
                message: "Empty payload in TestLinkEngine".to_string(),
            });
        }

        // Header counts if present
        let (n_format, n_parts, n_pins, n_nails) = if !markers.header_count_marker.is_empty() {
            if let Some(pos) = lines.iter().position(|l| l.starts_with(markers.header_count_marker)) {
                let rest = lines[pos][markers.header_count_marker.len()..].trim();
                let counts: Vec<&str> = rest.split_whitespace().collect();
                if counts.len() < 4 {
                    return Err(ParseError::SyntaxError {
                        line: pos + 1,
                        message: "Malformed header counts line in var_data".to_string(),
                    });
                }
                let f = counts[0].parse::<usize>().map_err(|_| ParseError::SyntaxError { line: pos + 1, message: "Invalid format count".into() })?;
                let pa = counts[1].parse::<usize>().map_err(|_| ParseError::SyntaxError { line: pos + 1, message: "Invalid parts count".into() })?;
                let pi = counts[2].parse::<usize>().map_err(|_| ParseError::SyntaxError { line: pos + 1, message: "Invalid pins count".into() })?;
                let na = counts[3].parse::<usize>().map_err(|_| ParseError::SyntaxError { line: pos + 1, message: "Invalid nails count".into() })?;
                (Some(f), Some(pa), Some(pi), Some(na))
            } else {
                (None, None, None, None)
            }
        } else {
            (None, None, None, None)
        };

        // Stop markers = all recognized block markers
        let mut stop_markers: Vec<&str> = Vec::new();
        stop_markers.extend_from_slice(markers.outline_markers);
        stop_markers.extend_from_slice(markers.parts_markers);
        stop_markers.extend_from_slice(markers.pins_markers);
        stop_markers.extend_from_slice(markers.nails_markers);
        stop_markers.extend_from_slice(markers.all_block_markers);

        // Outline
        let outline = Self::read_outline(&lines, markers.outline_markers, n_format, &stop_markers)?;

        // Parts raw: (name, type_layer, end_of_pins)
        let parts_raw = Self::read_parts(&lines, markers.parts_markers, n_parts, &stop_markers)?;

        let mut board = Board::new();
        board.outline_polygon = outline;

        for (refdes, type_layer, _) in &parts_raw {
            let side = if (type_layer & 0x2) != 0 {
                Side::Bottom
            } else {
                Side::Top
            };
            let part_type = if (type_layer & 0x4) != 0 {
                PartType::Smd
            } else {
                PartType::ThroughHole
            };

            let part_id = board.parts.len();
            board.parts.push(Part {
                id: part_id,
                name: refdes.clone(),
                mfg_code: String::new(),
                side,
                part_type,
                p1: Point::ZERO,
                p2: Point::ZERO,
                pin_indices: Vec::new(),
                bounding_box: None,
            });
        }

        // Nails raw -> (probe, via)
        let nails_raw = Self::read_nails(&lines, markers.nails_markers, n_nails, &stop_markers)?;
        let mut nail_map: BTreeMap<i32, String> = BTreeMap::new();
        for (probe, via) in nails_raw {
            if probe != 0 && !via.net_name.is_empty() {
                nail_map.insert(probe, via.net_name.clone());
            }
            board.vias.push(via);
        }

        // Pins: (x, y, probe, part_idx, net)
        let pins = Self::read_pins(
            &lines,
            markers.pins_markers,
            n_pins,
            &stop_markers,
            &parts_raw,
            &board.parts,
            &nail_map,
        )?;
        board.pins = pins;

        // Build spatial index, link pins to parts, bounding boxes
        board.rebuild_indices();

        // If outline was empty, synthesize bounding box outline with 20-mil margin
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

    fn find_first_marker(lines: &[&str], markers: &[&str]) -> Option<usize> {
        lines.iter().position(|l| markers.iter().any(|m| l.starts_with(m)))
    }

    fn read_block<'a>(
        lines: &'a [&'a str],
        start_idx: usize,
        count: Option<usize>,
        stop_markers: &[&str],
    ) -> Vec<&'a str> {
        let mut out = Vec::new();
        for &ln in &lines[start_idx + 1..] {
            if stop_markers.iter().any(|m| ln.starts_with(m)) {
                break;
            }
            out.push(ln);
            if let Some(cnt) = count {
                if out.len() >= cnt {
                    break;
                }
            }
        }
        out
    }

    fn read_outline(
        lines: &[&str],
        markers: &[&str],
        count: Option<usize>,
        stop_markers: &[&str],
    ) -> Result<Vec<Point>, ParseError> {
        let start_idx = match Self::find_first_marker(lines, markers) {
            Some(idx) => idx,
            None => {
                if count == Some(0) || count.is_none() {
                    return Ok(Vec::new());
                }
                return Err(ParseError::SyntaxError {
                    line: 0,
                    message: format!("Missing outline block {}", markers[0]),
                });
            }
        };

        let block = Self::read_block(lines, start_idx, count, stop_markers);
        let mut pts = Vec::with_capacity(block.len());
        for (i, line) in block.iter().enumerate() {
            let toks: Vec<&str> = line.split_whitespace().collect();
            if toks.len() < 2 {
                continue;
            }
            let x: f64 = toks[0].parse().map_err(|_| ParseError::SyntaxError {
                line: start_idx + 1 + i,
                message: "Invalid outline X coordinate".into(),
            })?;
            let y: f64 = toks[1].parse().map_err(|_| ParseError::SyntaxError {
                line: start_idx + 1 + i,
                message: "Invalid outline Y coordinate".into(),
            })?;
            pts.push(Point::new(x, y));
        }
        Ok(pts)
    }

    fn read_parts(
        lines: &[&str],
        markers: &[&str],
        count: Option<usize>,
        stop_markers: &[&str],
    ) -> Result<Vec<(String, u32, usize)>, ParseError> {
        let start_idx = match Self::find_first_marker(lines, markers) {
            Some(idx) => idx,
            None => {
                if count == Some(0) || count.is_none() {
                    return Ok(Vec::new());
                }
                return Err(ParseError::SyntaxError {
                    line: 0,
                    message: format!("Missing parts block {}", markers[0]),
                });
            }
        };

        let block = Self::read_block(lines, start_idx, count, stop_markers);
        let mut parts = Vec::with_capacity(block.len());
        for (_i, line) in block.iter().enumerate() {
            let toks: Vec<&str> = line.split_whitespace().collect();
            if toks.len() < 3 {
                continue;
            }
            let name = toks[0].to_string();
            let type_layer: u32 = toks[1].parse().unwrap_or(0);
            let end_of_pins: usize = toks[2].parse().unwrap_or(0);
            parts.push((name, type_layer, end_of_pins));
        }
        Ok(parts)
    }

    fn read_pins(
        lines: &[&str],
        markers: &[&str],
        count: Option<usize>,
        stop_markers: &[&str],
        _parts_raw: &[(String, u32, usize)],
        parts: &[Part],
        nail_map: &BTreeMap<i32, String>,
    ) -> Result<Vec<Pin>, ParseError> {
        let start_idx = match Self::find_first_marker(lines, markers) {
            Some(idx) => idx,
            None => {
                if count == Some(0) || count.is_none() {
                    return Ok(Vec::new());
                }
                return Err(ParseError::SyntaxError {
                    line: 0,
                    message: format!("Missing pins block {}", markers[0]),
                });
            }
        };

        let block = Self::read_block(lines, start_idx, count, stop_markers);
        let mut pins = Vec::with_capacity(block.len());
        let mut part_pin_counters = vec![0usize; parts.len()];

        for (i, line) in block.iter().enumerate() {
            let toks: Vec<&str> = line.split_whitespace().collect();
            if toks.len() < 4 {
                continue;
            }
            let x: f64 = toks[0].parse().map_err(|_| ParseError::SyntaxError {
                line: start_idx + 1 + i,
                message: "Invalid pin X coordinate".into(),
            })?;
            let y: f64 = toks[1].parse().map_err(|_| ParseError::SyntaxError {
                line: start_idx + 1 + i,
                message: "Invalid pin Y coordinate".into(),
            })?;
            let probe: i32 = toks[2].parse().unwrap_or(0);
            let part_1_based: usize = toks[3].parse().unwrap_or(0);
            let mut net = if toks.len() >= 5 {
                toks[4].to_string()
            } else {
                String::new()
            };

            if net.is_empty() && probe != 0 && probe != -99 {
                if let Some(nail_net) = nail_map.get(&probe) {
                    net = nail_net.clone();
                }
            }

            let part_id = if part_1_based >= 1 && part_1_based <= parts.len() {
                part_1_based - 1
            } else {
                0
            };

            let pin_num = if part_id < parts.len() {
                part_pin_counters[part_id] += 1;
                part_pin_counters[part_id].to_string()
            } else {
                (pins.len() + 1).to_string()
            };

            let side = if part_id < parts.len() {
                parts[part_id].side
            } else {
                Side::Top
            };

            let pin_id = pins.len();
            pins.push(Pin {
                id: pin_id,
                part_id,
                name: pin_num,
                net_name: net,
                pos: Point::new(x, y),
                side,
                radius: 12.0,
            });
        }

        Ok(pins)
    }

    fn read_nails(
        lines: &[&str],
        markers: &[&str],
        count: Option<usize>,
        stop_markers: &[&str],
    ) -> Result<Vec<(i32, Via)>, ParseError> {
        let start_idx = match Self::find_first_marker(lines, markers) {
            Some(idx) => idx,
            None => {
                return Ok(Vec::new());
            }
        };

        let block = Self::read_block(lines, start_idx, count, stop_markers);
        let mut vias = Vec::with_capacity(block.len());
        for (i, line) in block.iter().enumerate() {
            let toks: Vec<&str> = line.split_whitespace().collect();
            if toks.len() < 5 {
                continue;
            }
            let probe: i32 = toks[0].parse().unwrap_or(0);
            let x: f64 = toks[1].parse().map_err(|_| ParseError::SyntaxError {
                line: start_idx + 1 + i,
                message: "Invalid nail X coordinate".into(),
            })?;
            let y: f64 = toks[2].parse().map_err(|_| ParseError::SyntaxError {
                line: start_idx + 1 + i,
                message: "Invalid nail Y coordinate".into(),
            })?;
            let side_num: u32 = toks[3].parse().unwrap_or(1);
            let net = toks[4].to_string();

            let side = if side_num == 1 {
                Side::Top
            } else {
                Side::Bottom
            };

            let via_id = vias.len();
            vias.push((probe, Via {
                id: via_id,
                net_name: net,
                pos: Point::new(x, y),
                side,
                radius: 8.0,
            }));
        }
        Ok(vias)
    }
}
