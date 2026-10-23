//! BRD2 format parser.
//!
//! Handles OpenBoardView BRD2 format starting with `BRDOUT:` header:
//! `BRDOUT: <n_outline> <width> <height>`
//! `NETS: <n_nets>`
//! `PARTS: <n_parts>`
//! `PINS: <n_pins>`
//! `NAILS: <n_nails>`

use crate::board::Board;
use crate::model::{Part, PartType, Pin, Point, Side, Via};
use crate::parser::{FormatParser, ParseError};

pub struct Brd2Parser;

impl Brd2Parser {
    pub const fn new() -> Self {
        Self
    }

    pub fn parse_content(text: &str) -> Result<Board, ParseError> {
        let lines: Vec<&str> = text.lines().map(|l| l.trim()).filter(|l| !l.is_empty()).collect();
        if lines.is_empty() {
            return Err(ParseError::SyntaxError {
                line: 0,
                message: "Empty BRD2 payload".into(),
            });
        }

        // Outline
        let outline = Self::parse_brdout(&lines)?;

        // Nets
        let nets = Self::parse_nets(&lines)?;

        // Parts raw: (refdes, x1, y1, x2, y2, first_pin_0_based, side)
        let parts_raw = Self::parse_parts(&lines)?;

        // Pins raw: (x, y, net_id_1_based, side)
        let pins_raw = Self::parse_pins(&lines)?;

        // Nails raw: (probe, x, y, net_id_1_based, side)
        let nails_raw = Self::parse_nails(&lines)?;

        let mut board = Board::new();
        board.outline_polygon = outline;

        // Build parts and pins
        let n_pins = pins_raw.len();
        let mut first_pins = Vec::with_capacity(parts_raw.len());
        for p in &parts_raw {
            first_pins.push(p.5);
        }

        for (k, (refdes, _x1, _y1, _x2, _y2, _, side_num)) in parts_raw.iter().enumerate() {
            let start = first_pins[k];
            let end = if k + 1 < parts_raw.len() {
                first_pins[k + 1]
            } else {
                n_pins
            };

            let part_side = if *side_num == 2 {
                Side::Bottom
            } else {
                Side::Top
            };

            let part_id = board.parts.len();
            board.parts.push(Part {
                id: part_id,
                name: refdes.clone(),
                mfg_code: String::new(),
                side: part_side,
                part_type: PartType::Smd,
                p1: Point::ZERO,
                p2: Point::ZERO,
                pin_indices: Vec::new(),
                bounding_box: None,
            });

            // Pins for this part
            if start <= end && end <= n_pins {
                for (local_idx, p_idx) in (start..end).enumerate() {
                    let (px, py, net_id, pin_side_num) = pins_raw[p_idx];
                    let net_name = if net_id > 0 && net_id <= nets.len() {
                        nets[net_id - 1].clone()
                    } else {
                        String::new()
                    };
                    let pin_side = if pin_side_num == 2 {
                        Side::Bottom
                    } else {
                        Side::Top
                    };

                    let pin_id = board.pins.len();
                    board.pins.push(Pin {
                        id: pin_id,
                        part_id,
                        name: (local_idx + 1).to_string(),
                        net_name,
                        pos: Point::new(px as f64, py as f64),
                        side: pin_side,
                        radius: 12.0,
                    });
                }
            }
        }

        // Nails -> Vias
        for (_probe, x, y, net_id, side_num) in nails_raw {
            let net_name = if net_id > 0 && net_id <= nets.len() {
                nets[net_id - 1].clone()
            } else {
                String::new()
            };
            let side = if side_num == 2 {
                Side::Bottom
            } else {
                Side::Top
            };
            let via_id = board.vias.len();
            board.vias.push(Via {
                id: via_id,
                net_name,
                pos: Point::new(x as f64, y as f64),
                side,
                radius: 8.0,
            });
        }

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

    fn find_block(lines: &[&str], marker: &str) -> Result<(usize, usize), ParseError> {
        for (i, &ln) in lines.iter().enumerate() {
            if ln.starts_with(marker) {
                let rest = ln[marker.len()..].trim();
                let first_tok = rest.split_whitespace().next().unwrap_or("0");
                let count = first_tok.parse::<usize>().map_err(|_| ParseError::SyntaxError {
                    line: i + 1,
                    message: format!("Invalid count in {}", marker),
                })?;
                return Ok((i, count));
            }
        }
        Err(ParseError::SyntaxError {
            line: 0,
            message: format!("Marker {} not found", marker),
        })
    }

    fn iter_block_lines<'a>(lines: &'a [&'a str], start_idx: usize, count: usize) -> Vec<&'a str> {
        let mut out = Vec::with_capacity(count);
        for &ln in &lines[start_idx + 1..] {
            if Self::is_block_header(ln) {
                break;
            }
            out.push(ln);
            if out.len() >= count {
                break;
            }
        }
        out
    }

    fn is_block_header(line: &str) -> bool {
        const MARKERS: &[&str] = &["BRDOUT:", "NETS:", "PARTS:", "PINS:", "NAILS:"];
        MARKERS.iter().any(|m| line.starts_with(m))
    }

    fn parse_brdout(lines: &[&str]) -> Result<Vec<Point>, ParseError> {
        let (idx, count) = Self::find_block(lines, "BRDOUT:")?;
        if count == 0 {
            return Ok(Vec::new());
        }
        let block = Self::iter_block_lines(lines, idx, count);
        let mut pts = Vec::with_capacity(block.len());
        for (i, &ln) in block.iter().enumerate() {
            let toks: Vec<&str> = ln.split_whitespace().collect();
            if toks.len() < 2 {
                continue;
            }
            let x: f64 = toks[0].parse().map_err(|_| ParseError::SyntaxError {
                line: idx + 1 + i,
                message: "Invalid BRDOUT X".into(),
            })?;
            let y: f64 = toks[1].parse().map_err(|_| ParseError::SyntaxError {
                line: idx + 1 + i,
                message: "Invalid BRDOUT Y".into(),
            })?;
            pts.push(Point::new(x, y));
        }
        Ok(pts)
    }

    fn parse_nets(lines: &[&str]) -> Result<Vec<String>, ParseError> {
        let (idx, count) = Self::find_block(lines, "NETS:")?;
        if count == 0 {
            return Ok(Vec::new());
        }
        let block = Self::iter_block_lines(lines, idx, count);
        let mut nets = Vec::with_capacity(block.len());
        for &ln in &block {
            let mut parts = ln.split_whitespace();
            let _id = parts.next();
            let name = parts.next().unwrap_or("").to_string();
            nets.push(name);
        }
        Ok(nets)
    }

    fn parse_parts(lines: &[&str]) -> Result<Vec<(String, i32, i32, i32, i32, usize, i32)>, ParseError> {
        let (idx, count) = Self::find_block(lines, "PARTS:")?;
        if count == 0 {
            return Ok(Vec::new());
        }
        let block = Self::iter_block_lines(lines, idx, count);
        let mut parts = Vec::with_capacity(block.len());
        for (i, &ln) in block.iter().enumerate() {
            let toks: Vec<&str> = ln.split_whitespace().collect();
            if toks.len() < 7 {
                return Err(ParseError::SyntaxError {
                    line: idx + 1 + i,
                    message: "Malformed PARTS line".into(),
                });
            }
            let refdes = toks[0].to_string();
            let x1: i32 = toks[1].parse().unwrap_or(0);
            let y1: i32 = toks[2].parse().unwrap_or(0);
            let x2: i32 = toks[3].parse().unwrap_or(0);
            let y2: i32 = toks[4].parse().unwrap_or(0);
            let first_pin: usize = toks[5].parse().unwrap_or(0);
            let side: i32 = toks[6].parse().unwrap_or(1);
            parts.push((refdes, x1, y1, x2, y2, first_pin, side));
        }
        Ok(parts)
    }

    fn parse_pins(lines: &[&str]) -> Result<Vec<(i32, i32, usize, i32)>, ParseError> {
        let (idx, count) = Self::find_block(lines, "PINS:")?;
        if count == 0 {
            return Ok(Vec::new());
        }
        let block = Self::iter_block_lines(lines, idx, count);
        let mut pins = Vec::with_capacity(block.len());
        for (i, &ln) in block.iter().enumerate() {
            let toks: Vec<&str> = ln.split_whitespace().collect();
            if toks.len() < 4 {
                return Err(ParseError::SyntaxError {
                    line: idx + 1 + i,
                    message: "Malformed PINS line".into(),
                });
            }
            let x: i32 = toks[0].parse().unwrap_or(0);
            let y: i32 = toks[1].parse().unwrap_or(0);
            let net_id: usize = toks[2].parse().unwrap_or(0);
            let side: i32 = toks[3].parse().unwrap_or(1);
            pins.push((x, y, net_id, side));
        }
        Ok(pins)
    }

    fn parse_nails(lines: &[&str]) -> Result<Vec<(i32, i32, i32, usize, i32)>, ParseError> {
        let (idx, count) = match Self::find_block(lines, "NAILS:") {
            Ok(res) => res,
            Err(_) => return Ok(Vec::new()),
        };
        if count == 0 {
            return Ok(Vec::new());
        }
        let block = Self::iter_block_lines(lines, idx, count);
        let mut nails = Vec::with_capacity(block.len());
        for (i, &ln) in block.iter().enumerate() {
            let toks: Vec<&str> = ln.split_whitespace().collect();
            if toks.len() < 5 {
                return Err(ParseError::SyntaxError {
                    line: idx + 1 + i,
                    message: "Malformed NAILS line".into(),
                });
            }
            let probe: i32 = toks[0].parse().unwrap_or(0);
            let x: i32 = toks[1].parse().unwrap_or(0);
            let y: i32 = toks[2].parse().unwrap_or(0);
            let net_id: usize = toks[3].parse().unwrap_or(0);
            let side: i32 = toks[4].parse().unwrap_or(1);
            nails.push((probe, x, y, net_id, side));
        }
        Ok(nails)
    }
}

impl Default for Brd2Parser {
    fn default() -> Self {
        Self::new()
    }
}

impl FormatParser for Brd2Parser {
    fn name(&self) -> &'static str {
        "BRD2"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["brd2", "brd"]
    }

    fn verify_format(&self, data: &[u8]) -> bool {
        let s = String::from_utf8_lossy(&data[..data.len().min(1024)]);
        s.contains("BRDOUT:")
    }

    fn parse(&self, data: &[u8]) -> Result<Board, ParseError> {
        let text = String::from_utf8_lossy(data);
        Self::parse_content(&text)
    }
}
