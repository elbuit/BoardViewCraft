//! EasyEDA PCB parser for Standard (`.json`) and Pro (`.epro`, `.epro2`, `.epcb`, `.zip`).
//!
//! Follows EasyEDA Standard and Pro documented formats:
//! - Standard: `head` (docType 3), `shape` array containing `LIB` footprints, `PAD`, `TRACK`, `TEXT` strings.
//! - Pro: JSON-array line records (`COMPONENT`, `PAD_NET`, `LINE`, `PAD`).
//! - Coordinates normalized to millimeters.

use crate::board::Board;
use crate::model::{PadShape, Part, PartType, Pin, Point, Side};
use crate::parser::zip_container::ZipArchiveReader;
use crate::parser::{FormatParser, ParseError};
use serde_json::Value;
use std::collections::HashMap;

// ============================================================================
// EasyEDA Standard Parser (.json)
// ============================================================================

pub struct EasyEdaStdParser;

impl EasyEdaStdParser {
    pub const fn new() -> Self {
        Self
    }

    pub fn looks_like_easyeda_std(data: &[u8]) -> bool {
        let sample = if data.len() > 4096 { &data[..4096] } else { data };
        let text = String::from_utf8_lossy(sample);
        (text.contains("\"docType\":\"3\"") || text.contains("\"docType\": \"3\"")) && text.contains("\"shape\"")
    }
}

impl Default for EasyEdaStdParser {
    fn default() -> Self {
        Self::new()
    }
}

impl FormatParser for EasyEdaStdParser {
    fn name(&self) -> &'static str {
        "EasyEDA-Std"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["json"]
    }

    fn verify_format(&self, data: &[u8]) -> bool {
        Self::looks_like_easyeda_std(data)
    }

    fn parse(&self, data: &[u8]) -> Result<Board, ParseError> {
        let root: Value = serde_json::from_slice(data)
            .map_err(|e| ParseError::Other(format!("EasyEDA Standard JSON syntax error: {}", e)))?;

        let shapes = root
            .get("shape")
            .and_then(|v| v.as_array())
            .ok_or_else(|| ParseError::Other("Missing 'shape' array in EasyEDA Standard document".into()))?;

        let mut board = Board::new();
        // EasyEDA Standard unit: 10 mil = 0.254 mm
        const SCALE: f64 = 0.254;

        for item in shapes {
            if let Some(shape_str) = item.as_str() {
                // Footprint shape: LIB~x~y~attrs~rot~~id~lock#@$PAD~...#@$TEXT~...
                if shape_str.starts_with("LIB~") {
                    let sub_items: Vec<&str> = shape_str.split("#@$").collect();
                    if sub_items.is_empty() {
                        continue;
                    }

                    let lib_header: Vec<&str> = sub_items[0].split('~').collect();
                    let part_x = lib_header.get(1).and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0) * SCALE;
                    let part_y = lib_header.get(2).and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0) * SCALE;

                    let mut refdes = format!("U{}", board.parts.len() + 1);
                    let mut part_side = Side::Top;
                    let mut pads_to_add = Vec::new();

                    for sub in &sub_items[1..] {
                        let tokens: Vec<&str> = sub.split('~').collect();
                        if tokens.is_empty() {
                            continue;
                        }

                        if tokens[0] == "TEXT" {
                            // TEXT~type~x~y~...~text
                            if tokens.len() >= 6 && tokens[1] == "P" {
                                refdes = tokens.last().unwrap_or(&"").to_string();
                            }
                        } else if tokens[0] == "PAD" {
                            // PAD~shape~x~y~w~h~layer~net~number~...
                            if tokens.len() >= 10 {
                                let px = tokens[2].parse::<f64>().unwrap_or(0.0) * SCALE;
                                let py = tokens[3].parse::<f64>().unwrap_or(0.0) * SCALE;
                                let pw = tokens[4].parse::<f64>().unwrap_or(1.0) * SCALE;
                                let ph = tokens[5].parse::<f64>().unwrap_or(1.0) * SCALE;
                                let layer = tokens[6].parse::<u32>().unwrap_or(1);
                                let net = tokens[7].to_string();
                                let pad_num = tokens[8].to_string();

                                let pin_side = match layer {
                                    1 => Side::Top,
                                    2 => {
                                        part_side = Side::Bottom;
                                        Side::Bottom
                                    }
                                    _ => Side::Both,
                                };

                                pads_to_add.push((px, py, pw, ph, pad_num, net, pin_side));
                            }
                        }
                    }

                    let part_id = board.parts.len();
                    board.parts.push(Part {
                        id: part_id,
                        name: refdes,
                        mfg_code: String::new(),
                        side: part_side,
                        part_type: PartType::Smd,
                        p1: Point::new(part_x, part_y),
                        p2: Point::new(part_x, part_y),
                        pin_indices: Vec::new(),
                        bounding_box: None,
                        ..Default::default()
                    });

                    for (px, py, pw, ph, pad_num, net, pin_side) in pads_to_add {
                        let pin_id = board.pins.len();
                        board.pins.push(Pin {
                            id: pin_id,
                            part_id,
                            name: pad_num,
                            pos: Point::new(px, py),
                            side: pin_side,
                            net_name: net,
                            radius: pw.max(ph) * 0.5,
                            shape: PadShape::Rect,
                            width: Some(pw),
                            height: Some(ph),
                            rotation_deg: 0.0,
                        });
                        board.parts[part_id].pin_indices.push(pin_id);
                    }
                } else if shape_str.starts_with("TRACK~") {
                    // TRACK~width~layer~net~point_list...
                    let tokens: Vec<&str> = shape_str.split('~').collect();
                    if tokens.len() >= 5 && tokens[2] == "10" {
                        // Layer 10 is Board Outline
                        let pts: Vec<&str> = tokens[4].split_whitespace().collect();
                        for chunk in pts.chunks(2) {
                            if chunk.len() == 2 {
                                if let (Ok(x), Ok(y)) = (chunk[0].parse::<f64>(), chunk[1].parse::<f64>()) {
                                    board.outline_polygon.push(Point::new(x * SCALE, y * SCALE));
                                }
                            }
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

// ============================================================================
// EasyEDA Pro Parser (.epro, .epro2, .epcb)
// ============================================================================

pub struct EasyEdaProParser;

impl EasyEdaProParser {
    pub const fn new() -> Self {
        Self
    }
}

impl Default for EasyEdaProParser {
    fn default() -> Self {
        Self::new()
    }
}

impl FormatParser for EasyEdaProParser {
    fn name(&self) -> &'static str {
        "EasyEDA-Pro"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["epro", "epro2", "epcb"]
    }

    fn verify_format(&self, data: &[u8]) -> bool {
        if ZipArchiveReader::is_zip(data) {
            if let Ok(archive) = ZipArchiveReader::open(data) {
                return archive.entries.iter().any(|e| {
                    e.name == "project.json" || e.name == "project2.json" || e.name.starts_with("PCB/")
                });
            }
        }
        let sample = if data.len() > 1024 { &data[..1024] } else { data };
        let text = String::from_utf8_lossy(sample);
        text.contains("[\"DOCTYPE\",\"PCB\"") || text.contains("[\"COMPONENT\"")
    }

    fn parse(&self, data: &[u8]) -> Result<Board, ParseError> {
        let mut pcb_data = None;

        if ZipArchiveReader::is_zip(data) {
            let mut archive = ZipArchiveReader::open(data)
                .map_err(|e| ParseError::Other(format!("Failed to open EasyEDA Pro zip container: {}", e)))?;

            for entry in archive.entries.clone() {
                if entry.name.starts_with("PCB/") || entry.name.ends_with(".epcb") || entry.name.ends_with(".epru") {
                    if let Ok(extracted) = archive.extract(&entry) {
                        pcb_data = Some(extracted);
                        break;
                    }
                }
            }
        }

        let raw_bytes = pcb_data.as_deref().unwrap_or(data);
        let text = String::from_utf8_lossy(raw_bytes);

        let mut board = Board::new();
        let mut parts_map: HashMap<String, usize> = HashMap::new();
        let mut pad_nets: HashMap<(String, String), String> = HashMap::new();

        // EasyEDA Pro units are typically in mils (0.0254 mm) or mm. Default is mil (0.0254 mm).
        const MIL_TO_MM: f64 = 0.0254;

        for line in text.lines() {
            let trimmed = line.trim();
            if !trimmed.starts_with('[') || !trimmed.ends_with(']') {
                continue;
            }

            if let Ok(Value::Array(record)) = serde_json::from_str::<Value>(trimmed) {
                if record.is_empty() {
                    continue;
                }
                let rec_type = record[0].as_str().unwrap_or("");
                match rec_type {
                    "COMPONENT" => {
                        // ["COMPONENT", id, group, layer, x, y, rotation, {props}, locked]
                        if record.len() >= 7 {
                            let comp_id = record[1].as_str().unwrap_or("").to_string();
                            let layer_str = record[3].as_str().unwrap_or("TOP").to_uppercase();
                            let x = record[4].as_f64().unwrap_or(0.0) * MIL_TO_MM;
                            let y = record[5].as_f64().unwrap_or(0.0) * MIL_TO_MM;
                            let rot = record[6].as_f64().unwrap_or(0.0);

                            let side = if layer_str.contains("BOT") { Side::Bottom } else { Side::Top };
                            let part_id = board.parts.len();
                            parts_map.insert(comp_id.clone(), part_id);

                            board.parts.push(Part {
                                id: part_id,
                                name: comp_id,
                                mfg_code: String::new(),
                                side,
                                part_type: PartType::Smd,
                                p1: Point::new(x, y),
                                p2: Point::new(x, y),
                                pin_indices: Vec::new(),
                                bounding_box: None,
                                rotation_deg: rot,
                                ..Default::default()
                            });
                        }
                    }
                    "ATTR" => {
                        // ["ATTR", id, group, parentId, layer, x, y, key, value]
                        if record.len() >= 9 {
                            let parent_id = record[3].as_str().unwrap_or("");
                            let key = record[7].as_str().unwrap_or("");
                            let val = record[8].as_str().unwrap_or("");
                            if key.eq_ignore_ascii_case("designator") || key.eq_ignore_ascii_case("name") {
                                if let Some(&part_idx) = parts_map.get(parent_id) {
                                    board.parts[part_idx].name = val.to_string();
                                }
                            }
                        }
                    }
                    "PAD_NET" => {
                        // ["PAD_NET", componentId, padNumber, net]
                        if record.len() >= 4 {
                            let comp_id = record[1].as_str().unwrap_or("").to_string();
                            let pad_num = record[2].as_str().unwrap_or("").to_string();
                            let net = record[3].as_str().unwrap_or("").to_string();
                            pad_nets.insert((comp_id, pad_num), net);
                        }
                    }
                    "PAD" => {
                        // ["PAD", id, group, net, layer, number, x, y, rotation, hole, shape, width, height]
                        if record.len() >= 9 {
                            let net = record[3].as_str().unwrap_or("").to_string();
                            let pad_num = record[5].as_str().unwrap_or("1").to_string();
                            let x = record[6].as_f64().unwrap_or(0.0) * MIL_TO_MM;
                            let y = record[7].as_f64().unwrap_or(0.0) * MIL_TO_MM;
                            let rot = record[8].as_f64().unwrap_or(0.0);
                            let width = record.get(11).and_then(|v| v.as_f64()).unwrap_or(40.0) * MIL_TO_MM;
                            let height = record.get(12).and_then(|v| v.as_f64()).unwrap_or(40.0) * MIL_TO_MM;

                            let pin_id = board.pins.len();
                            board.pins.push(Pin {
                                id: pin_id,
                                part_id: 0,
                                name: pad_num,
                                pos: Point::new(x, y),
                                side: Side::Top,
                                net_name: net,
                                radius: width.max(height) * 0.5,
                                shape: PadShape::Rect,
                                width: Some(width),
                                height: Some(height),
                                rotation_deg: rot,
                            });
                        }
                    }
                    "LINE" => {
                        // ["LINE", id, group, net, layer, x1, y1, x2, y2]
                        if record.len() >= 9 && record[4].as_str() == Some("OUTLINE") {
                            let x1 = record[5].as_f64().unwrap_or(0.0) * MIL_TO_MM;
                            let y1 = record[6].as_f64().unwrap_or(0.0) * MIL_TO_MM;
                            let x2 = record[7].as_f64().unwrap_or(0.0) * MIL_TO_MM;
                            let y2 = record[8].as_f64().unwrap_or(0.0) * MIL_TO_MM;
                            board.outline_polygon.push(Point::new(x1, y1));
                            board.outline_polygon.push(Point::new(x2, y2));
                        }
                    }
                    _ => {}
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
