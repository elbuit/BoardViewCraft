//! XZZ encrypted `.pcb` format parser.
//!
//! Features:
//! - Magic check `XZZPCB` or XOR descrambled header with key at offset 0x10
//! - DES-ECB decryption of PART blocks
//! - Extracted parts, pins, nets, test pads, and board outlines

use std::collections::BTreeMap;
use std::env;
use crate::board::Board;
use crate::model::{Part, PartType, Pin, Point, Side, Via};
use crate::parser::{FormatParser, ParseError};

pub const XZZ_KEY_ENV: &str = "WRENCH_BOARD_XZZ_KEY";
const XZZ_GLOBAL_SCALE: f64 = 10000.0;

// Diode pattern used to find end of XOR scrambling
const DIODE_PATTERN: &[u8] = &[
    0x76, 0x36, 0x76, 0x36, 0x35, 0x35, 0x35, 0x76,
    0x36, 0x76, 0x36, 0x3D, 0x3D, 0x3D, 0xD7, 0xE8,
    0xD6, 0xB5, 0x0A,
];

pub struct XzzParser;

impl XzzParser {
    pub const fn new() -> Self {
        Self
    }

    pub fn looks_like_xzz(data: &[u8]) -> bool {
        if data.len() < 17 {
            return false;
        }
        if &data[0..6] == b"XZZPCB" {
            return true;
        }
        let xor_key = data[0x10];
        if xor_key != 0 {
            let descrambled: Vec<u8> = data[0..6].iter().map(|&b| b ^ xor_key).collect();
            if descrambled == b"XZZPCB" {
                return true;
            }
        }
        false
    }

    #[inline(always)]
    fn rd_u32(buf: &[u8], off: usize) -> Option<u32> {
        let end = off.checked_add(4)?;
        if end > buf.len() {
            return None;
        }
        Some(u32::from_le_bytes([buf[off], buf[off + 1], buf[off + 2], buf[off + 3]]))
    }

    pub fn load_des_key() -> Option<u64> {
        if let Ok(raw) = env::var(XZZ_KEY_ENV) {
            let s = raw.trim().trim_start_matches("0x");
            if let Ok(val) = u64::from_str_radix(s, 16) {
                return Some(val);
            }
        }
        None
    }

    // --- Standard DES Implementation matching OpenBoardView / Daniel Huertas DES ---
    pub fn des(input: u64, key: u64, mode_decrypt: bool) -> u64 {
        const IP: [u8; 64] = [
            58, 50, 42, 34, 26, 18, 10, 2, 60, 52, 44, 36, 28, 20, 12, 4,
            62, 54, 46, 38, 30, 22, 14, 6, 64, 56, 48, 40, 32, 24, 16, 8,
            57, 49, 41, 33, 25, 17, 9, 1, 59, 51, 43, 35, 27, 19, 11, 3,
            61, 53, 45, 37, 29, 21, 13, 5, 63, 55, 47, 39, 31, 23, 15, 7,
        ];

        const PI: [u8; 64] = [
            40, 8, 48, 16, 56, 24, 64, 32, 39, 7, 47, 15, 55, 23, 63, 31,
            38, 6, 46, 14, 54, 22, 62, 30, 37, 5, 45, 13, 53, 21, 61, 29,
            36, 4, 44, 12, 52, 20, 60, 28, 35, 3, 43, 11, 51, 19, 59, 27,
            34, 2, 42, 10, 50, 18, 58, 26, 33, 1, 41, 9, 49, 17, 57, 25,
        ];

        const E: [u8; 48] = [
            32, 1, 2, 3, 4, 5, 4, 5, 6, 7, 8, 9,
            8, 9, 10, 11, 12, 13, 12, 13, 14, 15, 16, 17,
            16, 17, 18, 19, 20, 21, 20, 21, 22, 23, 24, 25,
            24, 25, 26, 27, 28, 29, 28, 29, 30, 31, 32, 1,
        ];

        const P: [u8; 32] = [
            16, 7, 20, 21, 29, 12, 28, 17, 1, 15, 23, 26, 5, 18, 31, 10,
            2, 8, 24, 14, 32, 27, 3, 9, 19, 13, 30, 6, 22, 11, 4, 25,
        ];

        const S: [[[u8; 16]; 4]; 8] = [
            [[14,4,13,1,2,15,11,8,3,10,6,12,5,9,0,7],[0,15,7,4,14,2,13,1,10,6,12,11,9,5,3,8],[4,1,14,8,13,6,2,11,15,12,9,7,3,10,5,0],[15,12,8,2,4,9,1,7,5,11,3,14,10,0,6,13]],
            [[15,1,8,14,6,11,3,4,9,7,2,13,12,0,5,10],[3,13,4,7,15,2,8,14,12,0,1,10,6,9,11,5],[0,14,7,11,10,4,13,1,5,8,12,6,9,3,2,15],[13,8,10,1,3,15,4,2,11,6,7,12,0,5,14,9]],
            [[10,0,9,14,6,3,15,5,1,13,12,7,11,4,2,8],[13,7,0,9,3,4,6,10,2,8,5,14,12,11,15,1],[13,6,4,9,8,15,3,0,11,1,2,12,5,10,14,7],[1,10,13,0,6,9,8,7,4,15,14,3,11,5,2,12]],
            [[7,13,14,3,0,6,9,10,1,2,8,5,11,12,4,15],[13,8,11,5,6,15,0,3,4,7,2,12,1,10,14,9],[10,6,9,0,12,11,7,13,15,1,3,14,5,2,8,4],[3,15,0,6,10,1,13,8,9,4,5,11,12,7,2,14]],
            [[2,12,4,1,7,10,11,6,8,5,3,15,13,0,14,9],[14,11,2,12,4,7,13,1,5,0,15,10,3,9,8,6],[4,2,1,11,10,13,7,8,15,9,12,5,6,3,0,14],[11,8,12,7,1,14,2,13,6,15,0,9,10,4,5,3]],
            [[12,1,10,15,9,2,6,8,0,13,3,4,14,7,5,11],[10,15,4,2,7,12,9,5,6,1,13,14,0,11,3,8],[9,14,15,5,2,8,12,3,7,0,4,10,1,13,11,6],[4,3,2,12,9,5,15,10,11,14,1,7,6,0,8,13]],
            [[4,11,2,14,15,0,8,13,3,12,9,7,5,10,6,1],[13,0,11,7,4,9,1,10,14,3,5,12,2,15,8,6],[1,4,11,13,12,3,7,14,10,15,6,8,0,5,9,2],[6,11,13,8,1,4,10,7,9,5,0,15,14,2,3,12]],
            [[13,2,8,4,6,15,11,1,10,9,3,14,5,0,12,7],[1,15,13,8,10,3,7,4,12,5,6,11,0,14,9,2],[7,11,4,1,9,12,14,2,0,6,10,13,15,3,5,8],[2,1,14,7,4,10,8,13,15,12,9,0,3,5,6,11]],
        ];

        const PC1: [u8; 56] = [
            57, 49, 41, 33, 25, 17, 9, 1, 58, 50, 42, 34, 26, 18,
            10, 2, 59, 51, 43, 35, 27, 19, 11, 3, 60, 52, 44, 36,
            63, 55, 47, 39, 31, 23, 15, 7, 62, 54, 46, 38, 30, 22,
            14, 6, 61, 53, 45, 37, 29, 21, 13, 5, 28, 20, 12, 4,
        ];

        const PC2: [u8; 48] = [
            14, 17, 11, 24, 1, 5, 3, 28, 15, 6, 21, 10,
            23, 19, 12, 4, 26, 8, 16, 7, 27, 20, 13, 2,
            41, 52, 31, 37, 47, 55, 30, 40, 51, 45, 33, 48,
            44, 49, 39, 56, 34, 53, 46, 42, 50, 36, 29, 32,
        ];

        const SHIFTS: [u8; 16] = [1, 1, 2, 2, 2, 2, 2, 2, 1, 2, 2, 2, 2, 2, 2, 1];

        // Initial Permutation
        let mut init_perm = 0u64;
        for &bit in &IP {
            init_perm <<= 1;
            init_perm |= (input >> (64 - bit)) & 1;
        }

        let mut l = (init_perm >> 32) as u32;
        let mut r = init_perm as u32;

        // Key Schedule
        let mut pc1 = 0u64;
        for &bit in &PC1 {
            pc1 <<= 1;
            pc1 |= (key >> (64 - bit)) & 1;
        }

        let mut c = ((pc1 >> 28) & 0x0FFF_FFFF) as u32;
        let mut d = (pc1 & 0x0FFF_FFFF) as u32;

        let mut sub_keys = [0u64; 16];
        for i in 0..16 {
            let shift = SHIFTS[i];
            for _ in 0..shift {
                c = (0x0fffffff & (c << 1)) | (0x00000001 & (c >> 27));
                d = (0x0fffffff & (d << 1)) | (0x00000001 & (d >> 27));
            }

            let cd = ((c as u64) << 28) | (d as u64);
            let mut k = 0u64;
            for &bit in &PC2 {
                k <<= 1;
                k |= (cd >> (56 - bit)) & 1;
            }
            sub_keys[i] = k;
        }

        // 16 Rounds
        for i in 0..16 {
            let k = if mode_decrypt {
                sub_keys[15 - i]
            } else {
                sub_keys[i]
            };

            // Expansion
            let mut er = 0u64;
            for &bit in &E {
                er <<= 1;
                er |= ((r >> (32 - bit)) & 1) as u64;
            }

            let s_in = er ^ k;
            let mut s_out = 0u32;
            for j in 0..8 {
                let chunk = ((s_in >> (42 - 6 * j)) & 0x3F) as u8;
                let row = ((chunk & 0x20) >> 4) | (chunk & 0x01);
                let col = (chunk >> 1) & 0x0F;
                s_out = (s_out << 4) | (S[j][row as usize][col as usize] as u32);
            }

            // Permutation P
            let mut p_res = 0u32;
            for &bit in &P {
                p_res <<= 1;
                p_res |= (s_out >> (32 - bit)) & 1;
            }

            let temp = r;
            r = l ^ p_res;
            l = temp;
        }

        let pre_output = ((r as u64) << 32) | (l as u64);

        // Inverse Initial Permutation
        let mut out = 0u64;
        for &bit in &PI {
            out <<= 1;
            out |= (pre_output >> (64 - bit)) & 1;
        }
        out
    }

    /// Decrypts a buffer with DES ECB mode
    pub fn des_decrypt(buf: &[u8], key: u64) -> Vec<u8> {
        let mut out = vec![0u8; buf.len()];
        let chunks = buf.len() / 8;
        for i in 0..chunks {
            let off = i * 8;
            let mut input = 0u64;
            for b in 0..8 {
                input |= (buf[off + b] as u64) << (56 - b * 8);
            }
            let output = Self::des(input, key, true);
            for b in 0..8 {
                out[off + b] = ((output >> (56 - b * 8)) & 0xFF) as u8;
            }
        }
        // Copy remaining non-aligned bytes as is
        let rem = chunks * 8;
        if rem < buf.len() {
            out[rem..].copy_from_slice(&buf[rem..]);
        }
        out
    }

    pub fn parse_data(raw: &[u8]) -> Result<Board, ParseError> {
        if raw.len() < 0x28 {
            return Err(ParseError::SyntaxError { line: 0, message: "XZZ file too short".into() });
        }

        // Descramble XOR
        let mut buf = raw.to_vec();
        if buf.len() > 0x10 && buf[0x10] != 0 {
            let xor_key = buf[0x10];
            let limit = if let Some(pos) = buf.windows(DIODE_PATTERN.len()).position(|w| w == DIODE_PATTERN) {
                pos
            } else {
                buf.len()
            };
            for b in buf[..limit].iter_mut() {
                *b ^= xor_key;
            }
        }

        let key = Self::load_des_key().unwrap_or(0);

        let main_data_offset = Self::rd_u32(&buf, 0x20).unwrap_or(0) as usize;
        let net_data_offset = Self::rd_u32(&buf, 0x28).unwrap_or(0) as usize;

        let main_data_start = main_data_offset + 0x20;
        let net_data_start = net_data_offset + 0x20;

        if net_data_start + 4 > buf.len() || main_data_start + 4 > buf.len() {
            return Err(ParseError::SyntaxError { line: 0, message: "Invalid XZZ data offsets".into() });
        }

        let main_data_blocks_size = Self::rd_u32(&buf, main_data_start).unwrap_or(0) as usize;
        let net_block_size = Self::rd_u32(&buf, net_data_start).unwrap_or(0) as usize;

        // Parse Net dictionary
        let mut net_dict: BTreeMap<u32, String> = BTreeMap::new();
        if net_data_start + 4 + net_block_size <= buf.len() {
            let net_buf = &buf[net_data_start + 4..net_data_start + 4 + net_block_size];
            let mut ptr = 0;
            while ptr + 8 <= net_buf.len() {
                let net_size = Self::rd_u32(net_buf, ptr).unwrap_or(0) as usize;
                ptr += 4;
                let net_idx = Self::rd_u32(net_buf, ptr).unwrap_or(0);
                ptr += 4;
                if net_size >= 8 && ptr + (net_size - 8) <= net_buf.len() {
                    let name_len = net_size - 8;
                    let name = String::from_utf8_lossy(&net_buf[ptr..ptr + name_len]).into_owned();
                    net_dict.insert(net_idx, name);
                    ptr += name_len;
                } else {
                    break;
                }
            }
        }

        let mut board = Board::new();
        let mut outline_points = Vec::new();

        // Process main blocks
        if main_data_start + 4 + main_data_blocks_size <= buf.len() {
            let mut ptr = main_data_start + 4;
            let end_ptr = main_data_start + 4 + main_data_blocks_size;

            while ptr + 5 <= end_ptr {
                let block_type = buf[ptr];
                ptr += 1;
                let block_size = Self::rd_u32(&buf, ptr).unwrap_or(0) as usize;
                ptr += 4;

                if ptr + block_size > end_ptr {
                    break;
                }

                let block_buf = &buf[ptr..ptr + block_size];
                match block_type {
                    0x01 => {
                        // ARC
                        if block_buf.len() >= 24 {
                            let layer = Self::rd_u32(block_buf, 0).unwrap_or(0);
                            let x = Self::rd_u32(block_buf, 4).unwrap_or(0);
                            let y = Self::rd_u32(block_buf, 8).unwrap_or(0);
                            if layer == 28 {
                                outline_points.push(Point::new(x as f64 / XZZ_GLOBAL_SCALE, y as f64 / XZZ_GLOBAL_SCALE));
                            }
                        }
                    }
                    0x05 => {
                        // LINE SEGMENT (layer 28 = board edge)
                        if block_buf.len() >= 24 {
                            let layer = Self::rd_u32(block_buf, 0).unwrap_or(0);
                            let x1 = Self::rd_u32(block_buf, 4).unwrap_or(0);
                            let y1 = Self::rd_u32(block_buf, 8).unwrap_or(0);
                            let x2 = Self::rd_u32(block_buf, 12).unwrap_or(0);
                            let y2 = Self::rd_u32(block_buf, 16).unwrap_or(0);
                            if layer == 28 {
                                outline_points.push(Point::new(x1 as f64 / XZZ_GLOBAL_SCALE, y1 as f64 / XZZ_GLOBAL_SCALE));
                                outline_points.push(Point::new(x2 as f64 / XZZ_GLOBAL_SCALE, y2 as f64 / XZZ_GLOBAL_SCALE));
                            }
                        }
                    }
                    0x07 => {
                        // PART / PIN (DES encrypted)
                        let dec_buf = if key != 0 {
                            Self::des_decrypt(block_buf, key)
                        } else {
                            block_buf.to_vec()
                        };

                        if dec_buf.len() >= 40 {
                            let part_size = Self::rd_u32(&dec_buf, 0).unwrap_or(0) as usize;
                            let mut cptr = 4 + 18;
                            if cptr + 4 <= dec_buf.len() {
                                let group_name_size = Self::rd_u32(&dec_buf, cptr).unwrap_or(0) as usize;
                                cptr += 4 + group_name_size;
                                if cptr < dec_buf.len() && dec_buf[cptr] == 0x06 {
                                    cptr += 31;
                                    if cptr + 4 <= dec_buf.len() {
                                        let name_size = Self::rd_u32(&dec_buf, cptr).unwrap_or(0) as usize;
                                        cptr += 4;
                                        if cptr + name_size <= dec_buf.len() {
                                            let part_name = String::from_utf8_lossy(&dec_buf[cptr..cptr + name_size]).into_owned();
                                            cptr += name_size;

                                            let part_id = board.parts.len();
                                            board.parts.push(Part {
                                                id: part_id,
                                                name: part_name,
                                                mfg_code: String::new(),
                                                side: Side::Top,
                                                part_type: PartType::Smd,
                                                p1: Point::ZERO,
                                                p2: Point::ZERO,
                                                pin_indices: Vec::new(),
                                                bounding_box: None,
                                            });

                                            // Read sub-blocks (pins are type 0x09)
                                            while cptr + 5 <= (part_size + 4).min(dec_buf.len()) {
                                                let sub_type = dec_buf[cptr];
                                                cptr += 1;
                                                if sub_type == 0x09 {
                                                    // Pin block
                                                    let pin_block_size = Self::rd_u32(&dec_buf, cptr).unwrap_or(0) as usize;
                                                    let pin_end = cptr + 4 + pin_block_size;
                                                    cptr += 8; // skip size + unknown
                                                    if cptr + 16 <= dec_buf.len() {
                                                        let x_orig = Self::rd_u32(&dec_buf, cptr).unwrap_or(0);
                                                        cptr += 4;
                                                        let y_orig = Self::rd_u32(&dec_buf, cptr).unwrap_or(0);
                                                        cptr += 4 + 8; // skip y + unknown
                                                        let pin_name_size = Self::rd_u32(&dec_buf, cptr).unwrap_or(0) as usize;
                                                        cptr += 4;
                                                        let pin_name = if cptr + pin_name_size <= dec_buf.len() {
                                                            let s = String::from_utf8_lossy(&dec_buf[cptr..cptr + pin_name_size]).into_owned();
                                                            cptr += pin_name_size;
                                                            s
                                                        } else {
                                                            String::new()
                                                        };
                                                        cptr += 32;
                                                        let net_idx = Self::rd_u32(&dec_buf, cptr).unwrap_or(0);
                                                        let net_name = net_dict.get(&net_idx).cloned().unwrap_or_default();

                                                        let pin_id = board.pins.len();
                                                        board.pins.push(Pin {
                                                            id: pin_id,
                                                            part_id,
                                                            name: pin_name,
                                                            net_name: if net_name == "NC" || net_name == "UNCONNECTED" { String::new() } else { net_name },
                                                            pos: Point::new(x_orig as f64 / XZZ_GLOBAL_SCALE, y_orig as f64 / XZZ_GLOBAL_SCALE),
                                                            side: Side::Top,
                                                            radius: 12.0,
                                                        });
                                                    }
                                                    cptr = pin_end;
                                                } else {
                                                    let sub_size = Self::rd_u32(&dec_buf, cptr).unwrap_or(0) as usize;
                                                    cptr += 4 + sub_size;
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    0x09 => {
                        // TEST PADS / DRILL HOLES
                        if block_buf.len() >= 20 {
                            let x_orig = Self::rd_u32(block_buf, 4).unwrap_or(0);
                            let y_orig = Self::rd_u32(block_buf, 8).unwrap_or(0);
                            let net_idx = if block_buf.len() >= 4 {
                                Self::rd_u32(block_buf, block_buf.len() - 4).unwrap_or(0)
                            } else {
                                0
                            };
                            let net_name = net_dict.get(&net_idx).cloned().unwrap_or_default();
                            let via_id = board.vias.len();
                            board.vias.push(Via {
                                id: via_id,
                                net_name,
                                pos: Point::new(x_orig as f64 / XZZ_GLOBAL_SCALE, y_orig as f64 / XZZ_GLOBAL_SCALE),
                                side: Side::Top,
                                radius: 8.0,
                            });
                        }
                    }
                    _ => {}
                }
                ptr += block_size;
            }
        }

        board.outline_polygon = outline_points;
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

impl Default for XzzParser {
    fn default() -> Self {
        Self::new()
    }
}

impl FormatParser for XzzParser {
    fn name(&self) -> &'static str {
        "XZZ"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["pcb"]
    }

    fn verify_format(&self, data: &[u8]) -> bool {
        Self::looks_like_xzz(data)
    }

    fn parse(&self, data: &[u8]) -> Result<Board, ParseError> {
        Self::parse_data(data)
    }
}
