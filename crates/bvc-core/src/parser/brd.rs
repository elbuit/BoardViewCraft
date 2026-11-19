//! Auto-sniffing `.brd` format suite.
//!
//! Variants:
//! 1. Substitution-encoded BRD (`\x23\xe2\x63\x28`)
//! 2. Packed binary BRD (`0a0a1200`, `030c1300`, `03101300`, `060b1200`, `0c0f1200`, `04151300`)
//! 3. BRD2 (`BRDOUT:`)
//! 4. Test_Link standard ASCII (`str_length:`, `var_data:`)

use std::collections::BTreeMap;
use crate::board::Board;
use crate::model::{Part, PartType, Pin, Point, Side, Via};
use crate::parser::brd2::Brd2Parser;
use crate::parser::test_link::{DialectMarkers, TestLinkEngine};
use crate::parser::{FormatParser, ParseError};

pub const SUBST_BRD_MAGIC: [u8; 4] = [0x23, 0xe2, 0x63, 0x28];

pub const PACKED_BRD_MAGICS: &[[u8; 4]] = &[
    [0x0a, 0x0a, 0x12, 0x00],
    [0x03, 0x0c, 0x13, 0x00],
    [0x03, 0x10, 0x13, 0x00],
    [0x06, 0x0b, 0x12, 0x00],
    [0x0c, 0x0f, 0x12, 0x00],
    [0x04, 0x15, 0x13, 0x00],
];

pub struct BrdParser;

impl BrdParser {
    pub const fn new() -> Self {
        Self
    }

    /// Bitwise closed-form deobfuscation for substitution-encoded BRD
    pub fn decode_subst(data: &[u8]) -> Vec<u8> {
        data.iter()
            .map(|&b| {
                if b == b'\r' || b == b'\n' || b == 0 {
                    b
                } else {
                    !(((b >> 6) & 3) | (b << 2))
                }
            })
            .collect()
    }

    /// Parses substitution-encoded BRD payload (which can be either Test_Link var_data or parts/nails table)
    pub fn parse_subst_brd(data: &[u8]) -> Result<Board, ParseError> {
        let decoded = Self::decode_subst(data);
        let text = String::from_utf8_lossy(&decoded);

        // If it decodes to canonical var_data Test_Link
        if text.contains("var_data:") {
            return TestLinkEngine::parse_text(&text, &DialectMarkers::default());
        }

        // Otherwise parse parts and nail/pin rows
        let mut board = Board::new();
        let mut parts_map: BTreeMap<String, usize> = BTreeMap::new();
        let mut in_parts = false;
        let mut in_nails = false;

        let is_refdes = |s: &str| -> bool {
            let b = s.as_bytes();
            if b.len() < 2 || b.len() > 8 { return false; }
            let mut alpha = 0;
            while alpha < b.len() && b[alpha].is_ascii_uppercase() { alpha += 1; }
            alpha >= 1 && alpha <= 3 && alpha < b.len() && b[alpha..].iter().all(|c| c.is_ascii_digit())
        };

        for line in text.lines() {
            let l = line.trim();
            if l.is_empty() {
                continue;
            }
            if l.starts_with("PSRBE:") || l.starts_with("PARTS:") || l.starts_with("Parts:") {
                in_parts = true;
                in_nails = false;
                continue;
            }
            if l.starts_with("NSALE:") || l.starts_with("NAILS:") || l.starts_with("Nails:") {
                in_nails = true;
                in_parts = false;
                continue;
            }

            let toks: Vec<&str> = l.split_whitespace().collect();
            if in_parts && toks.len() >= 3 && is_refdes(toks[0]) {
                let name = toks[0].to_string();
                let code: u32 = toks[1].parse().unwrap_or(1);
                let side = if (code & 1) != 0 { Side::Top } else { Side::Bottom };

                if !parts_map.contains_key(&name) {
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
                        ..Default::default()
                    });
                    parts_map.insert(name, part_id);
                }
                continue;
            }

            if (in_nails || toks.len() == 5) && toks.len() >= 5 {
                if let (Ok(x), Ok(y), Ok(side_code)) = (toks[0].parse::<f64>(), toks[1].parse::<f64>(), toks[3].parse::<u32>()) {
                    let net = toks[4].to_string();
                    let side = if side_code == 1 { Side::Top } else { Side::Bottom };
                    let via_id = board.vias.len();
                    board.vias.push(Via {
                        id: via_id,
                        net_name: net.clone(),
                        pos: Point::new(x, y),
                        side,
                        radius: 8.0,
                    });

                    // Also synthesize pin for the part if net prefixes with refdes or attach to parts
                    if !board.parts.is_empty() {
                        let mut owner_id = 0;
                        for (pname, &pid) in &parts_map {
                            if net.starts_with(pname) {
                                owner_id = pid;
                                break;
                            }
                        }
                        let pin_id = board.pins.len();
                        board.pins.push(Pin {
                            id: pin_id,
                            part_id: owner_id,
                            name: (board.pins.len() + 1).to_string(),
                            net_name: net,
                            pos: Point::new(x, y),
                            side,
                            radius: 12.0,
                            ..Default::default()
                        });
                    }
                }
            }
        }

        board.rebuild_indices();

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

    pub fn looks_like_packed_brd(data: &[u8]) -> bool {
        if data.len() < 4 {
            return false;
        }
        let head: [u8; 4] = [data[0], data[1], data[2], data[3]];
        PACKED_BRD_MAGICS.contains(&head)
    }

    pub fn parse_packed_brd(data: &[u8]) -> Result<Board, ParseError> {
        let mut board = Board::new();
        const HEAP_START: usize = 0x1200;
        if data.len() <= HEAP_START + 8 {
            return Err(ParseError::SyntaxError {
                line: 0,
                message: "Packed BRD file too short for heap".into(),
            });
        }

        let mut i = HEAP_START;
        let mut entries = Vec::new();
        while i + 5 < data.len() {
            if let Some(nul_pos) = data[i + 4..].iter().position(|&b| b == 0) {
                let end = i + 4 + nul_pos;
                let len = end - (i + 4);
                if (1..=48).contains(&len) {
                    let s_bytes = &data[i + 4..end];
                    if s_bytes.iter().all(|&b| (32..=126).contains(&b)) {
                        let s = String::from_utf8_lossy(s_bytes).into_owned();
                        entries.push(s);
                        i = (end + 1 + 3) & !3;
                        continue;
                    }
                }
            }
            i += 4;
        }

        let is_refdes = |s: &str| -> bool {
            let bytes = s.as_bytes();
            if bytes.len() < 2 || bytes.len() > 8 {
                return false;
            }
            let mut alpha_len = 0;
            while alpha_len < bytes.len() && bytes[alpha_len].is_ascii_uppercase() {
                alpha_len += 1;
            }
            if alpha_len == 0 || alpha_len > 3 || alpha_len == bytes.len() {
                return false;
            }
            bytes[alpha_len..].iter().all(|b| b.is_ascii_digit())
        };

        for s in entries {
            if is_refdes(&s) {
                if !board.parts.iter().any(|p| p.name == s) {
                    let part_id = board.parts.len();
                    board.parts.push(Part {
                        id: part_id,
                        name: s,
                        mfg_code: String::new(),
                        side: Side::Top,
                        part_type: PartType::Smd,
                        p1: Point::ZERO,
                        p2: Point::ZERO,
                        pin_indices: Vec::new(),
                        bounding_box: None,
                        ..Default::default()
                    });
                }
            }
        }

        board.rebuild_indices();
        Ok(board)
    }

    /// Checks if text looks like FlexBV boardview format
    pub fn looks_like_flexbv(text: &str) -> bool {
        let lines: Vec<&str> = text.lines().take(10).map(|l| l.trim()).collect();
        if lines.iter().any(|l| l.to_ascii_lowercase().starts_with("format:") && l.to_ascii_lowercase().contains("flexbv")) {
            return true;
        }
        let mut has_part = false;
        let mut has_pin_or_net = false;
        for line in text.lines().take(40) {
            let l = line.trim();
            if l.is_empty() || l.starts_with('#') || l.starts_with(';') || l.starts_with("//") {
                continue;
            }
            let toks: Vec<&str> = l.split_whitespace().collect();
            if toks.is_empty() {
                continue;
            }
            let kw = toks[0].to_ascii_lowercase();
            if kw == "part" && toks.len() >= 5 {
                has_part = true;
            } else if (kw == "pin" || kw == "net") && toks.len() >= 6 {
                has_pin_or_net = true;
            }
            if has_part && has_pin_or_net {
                return true;
            }
        }
        false
    }

    /// Parses FlexBV plain-text board format (supporting both `pin` and `net` pin line variants)
    pub fn parse_flexbv(text: &str) -> Result<Board, ParseError> {
        let mut board = Board::new();
        let mut parts_map: BTreeMap<String, usize> = BTreeMap::new();

        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with(';') || trimmed.starts_with("//") {
                continue;
            }
            let toks: Vec<&str> = trimmed.split_whitespace().collect();
            if toks.is_empty() {
                continue;
            }
            let kw = toks[0].to_ascii_lowercase();

            if kw == "part" && toks.len() >= 5 {
                let refdes = toks[1].to_string();
                let x: f64 = toks[2].parse().unwrap_or(0.0);
                let y: f64 = toks[3].parse().unwrap_or(0.0);
                let side = if toks[4].eq_ignore_ascii_case("B") { Side::Bottom } else { Side::Top };
                if !parts_map.contains_key(&refdes) {
                    let part_id = board.parts.len();
                    board.parts.push(Part {
                        id: part_id,
                        name: refdes.clone(),
                        mfg_code: String::new(),
                        side,
                        part_type: PartType::Smd,
                        p1: Point::new(x, y),
                        p2: Point::new(x, y),
                        pin_indices: Vec::new(),
                        bounding_box: None,
                        ..Default::default()
                    });
                    parts_map.insert(refdes, part_id);
                }
            } else if (kw == "pin" || kw == "net") && toks.len() >= 6 {
                let refdes = toks[1];
                let num = toks[2];
                let net = toks[3];
                let x: f64 = match toks[4].parse() { Ok(v) => v, Err(_) => continue };
                let y: f64 = match toks[5].parse() { Ok(v) => v, Err(_) => continue };
                let radius: f64 = if toks.len() >= 7 { toks[6].parse().unwrap_or(5.0) } else { 5.0 };

                let part_id = match parts_map.get(refdes) {
                    Some(&id) => id,
                    None => {
                        let id = board.parts.len();
                        board.parts.push(Part {
                            id,
                            name: refdes.to_string(),
                            mfg_code: String::new(),
                            side: Side::Top,
                            part_type: PartType::Smd,
                            p1: Point::new(x, y),
                            p2: Point::new(x, y),
                            pin_indices: Vec::new(),
                            bounding_box: None,
                            ..Default::default()
                        });
                        parts_map.insert(refdes.to_string(), id);
                        id
                    }
                };

                let pin_id = board.pins.len();
                let pin_side = board.parts[part_id].side;
                board.pins.push(Pin {
                    id: pin_id,
                    part_id,
                    name: num.to_string(),
                    net_name: net.to_string(),
                    pos: Point::new(x, y),
                    side: pin_side,
                    radius,
                    ..Default::default()
                });
                board.parts[part_id].pin_indices.push(pin_id);
            } else if (kw == "outline" || kw == "border" || kw == "board") && toks.len() >= 5 {
                if let (Ok(x1), Ok(y1), Ok(x2), Ok(y2)) = (toks[1].parse(), toks[2].parse(), toks[3].parse(), toks[4].parse()) {
                    board.outline_polygon.push(Point::new(x1, y1));
                    board.outline_polygon.push(Point::new(x2, y2));
                }
            }
        }

        // Compute centroid for parts whose coordinates were 0,0
        for part in &mut board.parts {
            if part.p1 == Point::ZERO && !part.pin_indices.is_empty() {
                let mut sum_x = 0.0;
                let mut sum_y = 0.0;
                for &pid in &part.pin_indices {
                    let p = &board.pins[pid];
                    sum_x += p.pos.x;
                    sum_y += p.pos.y;
                }
                let count = part.pin_indices.len() as f64;
                part.p1 = Point::new(sum_x / count, sum_y / count);
                part.p2 = part.p1;
            }
        }

        board.rebuild_indices();

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

impl Default for BrdParser {
    fn default() -> Self {
        Self::new()
    }
}

impl FormatParser for BrdParser {
    fn name(&self) -> &'static str {
        "BRD"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["brd"]
    }

    fn verify_format(&self, data: &[u8]) -> bool {
        if data.len() >= 4 && data[0..4] == SUBST_BRD_MAGIC {
            return true;
        }
        if Self::looks_like_packed_brd(data) {
            return true;
        }
        let sample = &data[..data.len().min(4096)];
        let s = String::from_utf8_lossy(sample);
        if s.contains("BRDOUT:") {
            return true;
        }
        if Self::looks_like_flexbv(&s) {
            return true;
        }
        (s.contains("str_length:") && s.contains("var_data:")) || (s.contains("var_data:") && (s.contains("Format:") || s.contains("FORMAT:")))
    }

    fn parse(&self, data: &[u8]) -> Result<Board, ParseError> {
        // 1. Substitution-encoded BRD
        if data.len() >= 4 && data[0..4] == SUBST_BRD_MAGIC {
            return Self::parse_subst_brd(data);
        }

        // 2. Packed binary BRD
        if Self::looks_like_packed_brd(data) {
            return Self::parse_packed_brd(data);
        }

        let s = String::from_utf8_lossy(data);

        // 3. FlexBV plain-text format
        if Self::looks_like_flexbv(&s) {
            return Self::parse_flexbv(&s);
        }

        // 4. BRD2
        if s.contains("BRDOUT:") {
            return Brd2Parser::parse_content(&s);
        }

        // 5. Test_Link standard ASCII
        TestLinkEngine::parse_text(&s, &DialectMarkers::default())
    }
}
