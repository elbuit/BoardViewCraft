//! FZ-zlib parser: pipe-delimited (`!`) boardview format.
//!
//! Real-world `.fz` and `.cad` files with 4-byte LE decompressed length + zlib stream.
//! After inflation, text contains section schemas (`A!col1!col2!...`) and data (`S!val1!...`).

use std::collections::BTreeMap;
use std::io::Read;
use flate2::read::ZlibDecoder;
use crate::board::Board;
use crate::model::{Part, PartType, Pin, Point, Side, Via};
use crate::parser::{FormatParser, ParseError};

pub struct FzZlibParser;

impl FzZlibParser {
    pub const fn new() -> Self {
        Self
    }

    pub fn looks_like_fz_zlib(data: &[u8]) -> bool {
        if data.len() < 8 {
            return false;
        }
        // zlib magics at offset 4: 78 9c, 78 da, 78 01
        data[4] == 0x78 && (data[5] == 0x9c || data[5] == 0xda || data[5] == 0x01)
    }

    pub fn decompress_stream(data: &[u8]) -> Result<String, ParseError> {
        if data.len() < 6 {
            return Err(ParseError::SyntaxError {
                line: 0,
                message: "Data too short for fz_zlib stream".into(),
            });
        }
        let stream = &data[4..];
        let mut decoder = ZlibDecoder::new(stream);
        let mut decompressed = Vec::new();
        decoder.read_to_end(&mut decompressed).map_err(|e| ParseError::Other(format!("Zlib decompression failed: {}", e)))?;
        Ok(String::from_utf8_lossy(&decompressed).into_owned())
    }

    pub fn parse_content(text: &str) -> Result<Board, ParseError> {
        let mut board = Board::new();
        let mut scale = 1.0; // mils

        // Detect UNIT
        for line in text.lines().take(50) {
            let l = line.trim();
            if l.starts_with("UNIT:") {
                let unit = l["UNIT:".len()..].trim().to_ascii_lowercase();
                if unit.starts_with("millimeter") || unit.starts_with("mm") {
                    scale = 39.3700787;
                } else if unit.starts_with("inch") {
                    scale = 1000.0;
                }
            }
        }

        let mut current_section = String::new();
        let mut parts_map: BTreeMap<String, usize> = BTreeMap::new();

        for line in text.lines() {
            let l = line.trim();
            if l.is_empty() {
                continue;
            }

            if l.starts_with("F!") || l.starts_with("SECTION!") {
                let parts: Vec<&str> = l.split('!').collect();
                if parts.len() >= 2 {
                    current_section = parts[1].trim().to_string();
                }
                continue;
            }

            if l.starts_with("A!") {
                // Header row for current section
                let parts: Vec<&str> = l.split('!').collect();
                if parts.len() >= 2 {
                    current_section = parts[1].trim().to_string();
                }
                continue;
            }

            if l.starts_with("S!") {
                let vals: Vec<&str> = l[2..].split('!').map(|s| s.trim()).collect();
                match current_section.as_str() {
                    "REFDES" => {
                        // refdes, ins_code, sym_name, mirror, rotate
                        if !vals.is_empty() && !vals[0].is_empty() {
                            let refdes = vals[0].to_string();
                            let mirror = if vals.len() >= 4 { vals[3] } else { "" };
                            let side = if mirror.eq_ignore_ascii_case("YES") {
                                Side::Bottom
                            } else {
                                Side::Top
                            };

                            if !parts_map.contains_key(&refdes) {
                                let part_id = board.parts.len();
                                board.parts.push(Part {
                                    id: part_id,
                                    name: refdes.clone(),
                                    mfg_code: if vals.len() >= 3 { vals[2].to_string() } else { String::new() },
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
                    "NET_NAME" => {
                        // net_name, refdes, pin_num, pin_name, x, y, test_point, radius
                        if vals.len() >= 6 {
                            let net_name = vals[0].to_string();
                            let refdes = vals[1].to_string();
                            let pin_name = if vals.len() >= 4 { vals[3] } else { "" };
                            let x_raw: f64 = vals[4].parse().unwrap_or(0.0);
                            let y_raw: f64 = vals[5].parse().unwrap_or(0.0);
                            let radius_raw: f64 = if vals.len() >= 8 { vals[7].parse().unwrap_or(12.0) } else { 12.0 };

                            let x = x_raw * scale;
                            let y = y_raw * scale;
                            let radius = if radius_raw > 0.0 { radius_raw * scale } else { 12.0 };

                            let part_id = if let Some(&id) = parts_map.get(&refdes) {
                                id
                            } else {
                                let id = board.parts.len();
                                board.parts.push(Part {
                                    id,
                                    name: refdes.clone(),
                                    mfg_code: String::new(),
                                    side: Side::Top,
                                    part_type: PartType::Smd,
                                    p1: Point::ZERO,
                                    p2: Point::ZERO,
                                    pin_indices: Vec::new(),
                                    bounding_box: None,
                                });
                                parts_map.insert(refdes, id);
                                id
                            };

                            let side = board.parts[part_id].side;
                            let pin_id = board.pins.len();
                            board.pins.push(Pin {
                                id: pin_id,
                                part_id,
                                name: pin_name.to_string(),
                                net_name,
                                pos: Point::new(x, y),
                                side,
                                radius,
                            });
                        }
                    }
                    "TESTVIA" => {
                        // TESTVIA, net_name, refdes, pin_num, pin_name, x, y, test_point, radius
                        if vals.len() >= 7 {
                            let net_name = vals[1].to_string();
                            let x_raw: f64 = vals[5].parse().unwrap_or(0.0);
                            let y_raw: f64 = vals[6].parse().unwrap_or(0.0);
                            let via_id = board.vias.len();
                            board.vias.push(Via {
                                id: via_id,
                                net_name,
                                pos: Point::new(x_raw * scale, y_raw * scale),
                                side: Side::Top,
                                radius: 8.0,
                            });
                        }
                    }
                    _ => {}
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

impl Default for FzZlibParser {
    fn default() -> Self {
        Self::new()
    }
}

impl FormatParser for FzZlibParser {
    fn name(&self) -> &'static str {
        "FZ_ZLIB"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["fz", "cad"]
    }

    fn verify_format(&self, data: &[u8]) -> bool {
        Self::looks_like_fz_zlib(data)
    }

    fn parse(&self, data: &[u8]) -> Result<Board, ParseError> {
        let text = Self::decompress_stream(data)?;
        Self::parse_content(&text)
    }
}
