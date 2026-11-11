//! XZZ encrypted `.pcb` format parser.
//!
//! Features:
//! - Magic check `XZZPCB` or XOR descrambled header with key at offset 0x10
//! - DES-ECB decryption of PART blocks with standard XZZ key and custom key support
//! - Two-sided butterfly layer separation (TOP on left, mirrored BOTTOM on right)
//! - Extracted parts, pins, nets, test pads, vias, and stitched outline polygon

use std::collections::BTreeMap;
use std::env;
use crate::board::Board;
use crate::model::{PadShape, Part, PartType, Pin, Point, Side, Via};
use crate::parser::{FormatParser, ParseError};

pub const XZZ_KEY_ENV: &str = "WRENCH_BOARD_XZZ_KEY";
pub const DEFAULT_XZZ_KEY: u64 = 0xdcfc_12ac_0000_0000;
const XZZ_GLOBAL_SCALE: f64 = 10000.0;
const V6_SENTINEL: &[u8] = b"v6v6555v6v6";

struct RawPin {
    name: String,
    x: f64,
    y: f64,
    net_name: String,
    shape: PadShape,
    width: Option<f64>,
    height: Option<f64>,
    rotation_deg: f64,
}

struct RawPart {
    name: String,
    mfg_code: String,
    rotation_deg: f64,
    pins: Vec<RawPin>,
}

struct RawVia {
    x: f64,
    y: f64,
    radius: f64,
    net_name: String,
    is_through_hole: bool,
}

pub struct XzzParser;

impl XzzParser {
    pub const DEFAULT_XZZ_KEY: u64 = DEFAULT_XZZ_KEY;

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

    #[inline(always)]
    fn rd_i32(buf: &[u8], off: usize) -> Option<i32> {
        let end = off.checked_add(4)?;
        if end > buf.len() {
            return None;
        }
        Some(i32::from_le_bytes([buf[off], buf[off + 1], buf[off + 2], buf[off + 3]]))
    }

    /// Key parity check: bytes 0..6 have even popcount, byte 7 has odd popcount.
    pub fn check_key(key: u64) -> bool {
        const REQUIRED_KEY_PARITY: [u8; 8] = [1, 1, 1, 1, 1, 1, 1, 0];
        for i in 0..8 {
            let mut b = ((key >> (i * 8)) & 0xff) as u8;
            b ^= b >> 4;
            b ^= b >> 2;
            b ^= b >> 1;
            let bit = (!b) & 1;
            if bit != REQUIRED_KEY_PARITY[i] {
                return false;
            }
        }
        true
    }

    pub fn parse_key_text(text: &str) -> Option<u64> {
        for line in text.lines() {
            let line = if let Some(pos) = line.find('#') { &line[..pos] } else { line };
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let token = line.split_whitespace().next()?;
            let token = token.trim_start_matches("0x").trim_start_matches("0X");
            let clean: String = token.chars().filter(|c| c.is_ascii_hexdigit()).collect();
            if clean.len() >= 16 {
                if let Ok(val) = u64::from_str_radix(&clean[..16], 16) {
                    return Some(val);
                }
            } else if let Ok(val) = u64::from_str_radix(&clean, 16) {
                return Some(val);
            }
        }
        None
    }

    pub fn load_des_key() -> u64 {
        for var in &[XZZ_KEY_ENV, "XZZPCB_KEY", "XZZ_KEY"] {
            if let Ok(raw) = env::var(var) {
                if let Some(key) = Self::parse_key_text(&raw) {
                    if Self::check_key(key) {
                        return key;
                    }
                }
            }
        }

        let key_paths = [
            "private/XZZ_Key.txt",
            "private/xzz_key.txt",
            "XZZ_Key.txt",
            "xzz_key.txt",
        ];
        for path in &key_paths {
            if let Ok(text) = std::fs::read_to_string(path) {
                if let Some(key) = Self::parse_key_text(&text) {
                    if Self::check_key(key) {
                        return key;
                    }
                }
            }
        }

        if let Ok(home) = env::var("HOME") {
            let legacy = std::path::Path::new(&home).join(".boardviewer").join("xzz_key");
            if let Ok(text) = std::fs::read_to_string(legacy) {
                if let Some(key) = Self::parse_key_text(&text) {
                    if Self::check_key(key) {
                        return key;
                    }
                }
            }
        }

        Self::DEFAULT_XZZ_KEY
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

    fn arc_to_segments(cx: f64, cy: f64, r: f64, a0: f64, a1: f64, n: usize) -> Vec<(Point, Point)> {
        let (mut s_deg, mut e_deg) = (a0, a1);
        if s_deg > e_deg {
            std::mem::swap(&mut s_deg, &mut e_deg);
        }
        if e_deg - s_deg > 180.0 {
            s_deg += 360.0;
        }
        let rad = std::f64::consts::PI / 180.0;
        let s = s_deg * rad;
        let e = e_deg * rad;
        let step = (e - s) / (n - 1) as f64;
        let mut segments = Vec::with_capacity(n - 1);
        let mut px = cx + r * s.cos();
        let mut py = cy + r * s.sin();
        for i in 1..n {
            let a = s + (i as f64) * step;
            let nx = cx + r * a.cos();
            let ny = cy + r * a.sin();
            segments.push((Point::new(px, py), Point::new(nx, ny)));
            px = nx;
            py = ny;
        }
        segments
    }

    fn chain_segments(mut segments: Vec<(Point, Point)>, tol: f64) -> Vec<Point> {
        if segments.is_empty() {
            return Vec::new();
        }
        let mut chain = Vec::new();
        let first = segments.remove(0);
        chain.push(first.0);
        chain.push(first.1);

        while !segments.is_empty() {
            let curr = *chain.last().unwrap();
            let mut found_idx = None;
            let mut reversed = false;
            let mut min_d = tol;

            for (idx, (p1, p2)) in segments.iter().enumerate() {
                let d1 = (curr.x - p1.x).abs().max((curr.y - p1.y).abs());
                let d2 = (curr.x - p2.x).abs().max((curr.y - p2.y).abs());
                if d1 <= min_d {
                    min_d = d1;
                    found_idx = Some(idx);
                    reversed = false;
                }
                if d2 <= min_d {
                    min_d = d2;
                    found_idx = Some(idx);
                    reversed = true;
                }
            }

            if let Some(idx) = found_idx {
                let seg = segments.remove(idx);
                if reversed {
                    chain.push(seg.0);
                } else {
                    chain.push(seg.1);
                }
            } else {
                break;
            }
        }

        if chain.len() > 1 {
            let first_pt = chain[0];
            let last_pt = *chain.last().unwrap();
            if (first_pt.x - last_pt.x).abs() <= tol && (first_pt.y - last_pt.y).abs() <= tol {
                chain.pop();
            }
        }
        chain
    }

    fn cluster_segments(segments: &[(Point, Point)], eps: f64) -> Vec<Vec<(Point, Point)>> {
        let n = segments.len();
        if n == 0 {
            return Vec::new();
        }
        let mut parent: Vec<usize> = (0..n).collect();
        fn find(parent: &mut [usize], mut i: usize) -> usize {
            while parent[i] != i {
                parent[i] = parent[parent[i]];
                i = parent[i];
            }
            i
        }
        for i in 0..n {
            for j in (i + 1)..n {
                let (s1a, s1b) = segments[i];
                let (s2a, s2b) = segments[j];
                let near = |p1: Point, p2: Point| (p1.x - p2.x).abs() <= eps && (p1.y - p2.y).abs() <= eps;
                if near(s1a, s2a) || near(s1a, s2b) || near(s1b, s2a) || near(s1b, s2b) {
                    let ri = find(&mut parent, i);
                    let rj = find(&mut parent, j);
                    if ri != rj {
                        parent[ri] = rj;
                    }
                }
            }
        }
        let mut groups: BTreeMap<usize, Vec<(Point, Point)>> = BTreeMap::new();
        for i in 0..n {
            let root = find(&mut parent, i);
            groups.entry(root).or_default().push(segments[i]);
        }
        groups.into_values().collect()
    }

    fn segments_bbox(segs: &[(Point, Point)]) -> (f64, f64, f64, f64) {
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        for (p1, p2) in segs {
            min_x = min_x.min(p1.x).min(p2.x);
            min_y = min_y.min(p1.y).min(p2.y);
            max_x = max_x.max(p1.x).max(p2.x);
            max_y = max_y.max(p1.y).max(p2.y);
        }
        (min_x, min_y, max_x, max_y)
    }

    pub fn parse_data(raw: &[u8]) -> Result<Board, ParseError> {
        if raw.len() < 0x28 {
            return Err(ParseError::SyntaxError { line: 0, message: "XZZ file too short".into() });
        }

        // Descramble XOR
        let mut buf = raw.to_vec();
        if buf.len() > 0x10 && buf[0x10] != 0 {
            let xor_key = buf[0x10];
            let limit = if let Some(pos) = buf.windows(V6_SENTINEL.len()).position(|w| w == V6_SENTINEL) {
                pos
            } else {
                buf.len()
            };
            for b in buf[..limit].iter_mut() {
                *b ^= xor_key;
            }
        }

        let key = Self::load_des_key();

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
                    let name = String::from_utf8_lossy(&net_buf[ptr..ptr + name_len])
                        .trim_matches('\0')
                        .trim()
                        .to_string();
                    net_dict.insert(net_idx, name);
                    ptr += name_len;
                } else {
                    break;
                }
            }
        }

        let mut layer28_segments = Vec::new();
        let mut raw_parts = Vec::new();
        let mut raw_vias = Vec::new();

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
                            if layer == 28 {
                                let cx = Self::rd_u32(block_buf, 4).unwrap_or(0) as f64 / XZZ_GLOBAL_SCALE;
                                let cy = Self::rd_u32(block_buf, 8).unwrap_or(0) as f64 / XZZ_GLOBAL_SCALE;
                                let r = Self::rd_u32(block_buf, 12).unwrap_or(0) as f64 / XZZ_GLOBAL_SCALE;
                                let a0 = Self::rd_u32(block_buf, 16).unwrap_or(0) as f64 / XZZ_GLOBAL_SCALE;
                                let a1 = Self::rd_u32(block_buf, 20).unwrap_or(0) as f64 / XZZ_GLOBAL_SCALE;
                                layer28_segments.extend(Self::arc_to_segments(cx, cy, r, a0, a1, 10));
                            }
                        }
                    }
                    0x02 => {
                        // VIA
                        if block_buf.len() >= 28 {
                            let x = Self::rd_u32(block_buf, 0).unwrap_or(0) as f64 / XZZ_GLOBAL_SCALE;
                            let y = Self::rd_u32(block_buf, 4).unwrap_or(0) as f64 / XZZ_GLOBAL_SCALE;
                            let diam = Self::rd_u32(block_buf, 8).unwrap_or(0) as f64 / XZZ_GLOBAL_SCALE;
                            let layer_start = Self::rd_u32(block_buf, 16).unwrap_or(0);
                            let layer_end = Self::rd_u32(block_buf, 20).unwrap_or(0);
                            let net_idx = Self::rd_u32(block_buf, 24).unwrap_or(0);
                            let net_name = net_dict.get(&net_idx).cloned().unwrap_or_default();
                            let net_name = if net_name == "NC" || net_name.starts_with("UNCONNECTED") {
                                String::new()
                            } else {
                                net_name
                            };
                            let radius = if diam > 0.0 { diam / 2.0 } else { 8.0 };
                            let is_through_hole = layer_start <= 1 && layer_end >= 16;
                            raw_vias.push(RawVia {
                                x,
                                y,
                                radius,
                                net_name,
                                is_through_hole,
                            });
                        }
                    }
                    0x05 => {
                        // LINE SEGMENT (layer 28 = board edge)
                        if block_buf.len() >= 24 {
                            let layer = Self::rd_u32(block_buf, 0).unwrap_or(0);
                            if layer == 28 {
                                let x1 = Self::rd_u32(block_buf, 4).unwrap_or(0) as f64 / XZZ_GLOBAL_SCALE;
                                let y1 = Self::rd_u32(block_buf, 8).unwrap_or(0) as f64 / XZZ_GLOBAL_SCALE;
                                let x2 = Self::rd_u32(block_buf, 12).unwrap_or(0) as f64 / XZZ_GLOBAL_SCALE;
                                let y2 = Self::rd_u32(block_buf, 16).unwrap_or(0) as f64 / XZZ_GLOBAL_SCALE;
                                layer28_segments.push((Point::new(x1, y1), Point::new(x2, y2)));
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
                            let part_rot = Self::rd_u32(&dec_buf, 16).unwrap_or(0) as f64 / XZZ_GLOBAL_SCALE;
                            let fp_len = Self::rd_u32(&dec_buf, 22).unwrap_or(0) as usize;
                            let mfg_code = if 26 + fp_len <= dec_buf.len() {
                                String::from_utf8_lossy(&dec_buf[26..26 + fp_len])
                                    .trim_matches('\0')
                                    .trim()
                                    .to_string()
                            } else {
                                String::new()
                            };

                            let mut cptr = 26 + fp_len;
                            let mut part_name = String::new();
                            let mut part_pins = Vec::new();
                            let end_part = (part_size + 4).min(dec_buf.len());

                            while cptr + 5 <= end_part {
                                let sub_type = dec_buf[cptr];
                                cptr += 1;
                                if sub_type == 0x00 {
                                    continue;
                                }
                                let sub_size = Self::rd_u32(&dec_buf, cptr).unwrap_or(0) as usize;
                                let sub_start = cptr + 4;
                                let sub_end = sub_start + sub_size;
                                if sub_end > dec_buf.len() {
                                    break;
                                }

                                match sub_type {
                                    0x06 => {
                                        // Text Label (sub_type 0x06)
                                        // Format: [26 bytes header] [name_size: u32] [name: bytes]
                                        if sub_size >= 30 {
                                            let name_size = Self::rd_u32(&dec_buf, sub_start + 26).unwrap_or(0) as usize;
                                            if sub_start + 30 + name_size <= sub_end {
                                                let text = String::from_utf8_lossy(&dec_buf[sub_start + 30..sub_start + 30 + name_size])
                                                    .trim_matches('\0')
                                                    .trim()
                                                    .to_string();
                                                if part_name.is_empty() {
                                                    part_name = text;
                                                }
                                            }
                                        }
                                    }
                                    0x09 => {
                                        // Pin block (sub_type 0x09)
                                        // Offset 4: pin_x (i32)
                                        // Offset 8: pin_y (i32)
                                        // Offset 16: pin_rotation (u32)
                                        // Offset 20: pin_name_size (u32)
                                        // Offset 24: pin_name
                                        if sub_size >= 24 {
                                            let x_orig = Self::rd_i32(&dec_buf, sub_start + 4).unwrap_or(0) as f64 / XZZ_GLOBAL_SCALE;
                                            let y_orig = Self::rd_i32(&dec_buf, sub_start + 8).unwrap_or(0) as f64 / XZZ_GLOBAL_SCALE;
                                            let pin_rot = Self::rd_u32(&dec_buf, sub_start + 16).unwrap_or(0) as f64 / XZZ_GLOBAL_SCALE;
                                            let pin_name_size = Self::rd_u32(&dec_buf, sub_start + 20).unwrap_or(0) as usize;

                                            let mut pin_name = String::new();
                                            let mut outline_ptr = 24 + pin_name_size;
                                            if sub_start + outline_ptr <= sub_end {
                                                pin_name = String::from_utf8_lossy(&dec_buf[sub_start + 24..sub_start + 24 + pin_name_size])
                                                    .trim_matches('\0')
                                                    .trim()
                                                    .to_string();
                                            }

                                            let mut shape = PadShape::Round;
                                            let mut width = None;
                                            let mut height = None;

                                            // Outline loops (up to 4)
                                            for _ in 0..4 {
                                                if outline_ptr + 5 > sub_size {
                                                    break;
                                                }
                                                if dec_buf[sub_start + outline_ptr..sub_start + outline_ptr + 5].iter().all(|&b| b == 0) {
                                                    break;
                                                }
                                                if outline_ptr + 9 > sub_size {
                                                    break;
                                                }
                                                let w = Self::rd_u32(&dec_buf, sub_start + outline_ptr).unwrap_or(0) as f64 / XZZ_GLOBAL_SCALE;
                                                let h = Self::rd_u32(&dec_buf, sub_start + outline_ptr + 4).unwrap_or(0) as f64 / XZZ_GLOBAL_SCALE;
                                                let otype = dec_buf[sub_start + outline_ptr + 8];
                                                outline_ptr += 9;

                                                if width.is_none() && w > 0.0 && h > 0.0 {
                                                    width = Some(w);
                                                    height = Some(h);
                                                    shape = match otype {
                                                        0x02 => {
                                                            if (w - h).abs() < 1e-3 {
                                                                PadShape::Square
                                                            } else {
                                                                PadShape::Rect
                                                            }
                                                        }
                                                        _ => PadShape::Round,
                                                    };
                                                }
                                            }

                                            // Net index from 12 bytes footer: [net_id: u32, padding: 8 bytes]
                                            let net_idx = if sub_size >= 12 {
                                                Self::rd_u32(&dec_buf, sub_end - 12).unwrap_or(0)
                                            } else {
                                                0
                                            };
                                            let net_name = net_dict.get(&net_idx).cloned().unwrap_or_default();
                                            let net_name = if net_name == "NC" || net_name.starts_with("UNCONNECTED") {
                                                String::new()
                                            } else {
                                                net_name
                                            };

                                            part_pins.push(RawPin {
                                                name: pin_name,
                                                x: x_orig,
                                                y: y_orig,
                                                net_name,
                                                shape,
                                                width,
                                                height,
                                                rotation_deg: pin_rot,
                                            });
                                        }
                                    }
                                    _ => {}
                                }
                                cptr = sub_end;
                            }

                            if !part_name.is_empty() || !part_pins.is_empty() {
                                raw_parts.push(RawPart {
                                    name: part_name,
                                    mfg_code,
                                    rotation_deg: part_rot,
                                    pins: part_pins,
                                });
                            }
                        }
                    }
                    0x09 => {
                        // TEST PADS / DRILL HOLES
                        if block_buf.len() >= 24 {
                            let x = Self::rd_u32(block_buf, 4).unwrap_or(0) as f64 / XZZ_GLOBAL_SCALE;
                            let y = Self::rd_u32(block_buf, 8).unwrap_or(0) as f64 / XZZ_GLOBAL_SCALE;
                            let name_len = Self::rd_u32(block_buf, 20).unwrap_or(0) as usize;
                            let net_pos = 24 + name_len + 32;
                            let net_idx = if net_pos + 4 <= block_buf.len() {
                                Self::rd_u32(block_buf, net_pos).unwrap_or(0)
                            } else if block_buf.len() >= 4 {
                                Self::rd_u32(block_buf, block_buf.len() - 4).unwrap_or(0)
                            } else {
                                0
                            };
                            let net_name = net_dict.get(&net_idx).cloned().unwrap_or_default();
                            let net_name = if net_name == "NC" || net_name.starts_with("UNCONNECTED") {
                                String::new()
                            } else {
                                net_name
                            };
                            raw_vias.push(RawVia {
                                x,
                                y,
                                radius: 8.0,
                                net_name,
                                is_through_hole: false,
                            });
                        }
                    }
                    _ => {}
                }
                ptr += block_size;
            }
        }

        let mut board = Board::new();

        // 1. Cluster layer 28 segments into connected components
        let clusters = Self::cluster_segments(&layer28_segments, 5.0);

        // Detect if the file is two-sided (adjacent side-by-side boards)
        let mut is_two_sided = false;
        let mut midpoint = 0.0;
        let mut r_max_x = 0.0;
        let mut origin_x = 0.0;
        let mut origin_y = 0.0;
        let mut board_outline_segments = Vec::new();

        if clusters.len() >= 2 {
            let mut sorted_clusters = clusters;
            sorted_clusters.sort_by_key(|c| std::cmp::Reverse(c.len()));

            let c1 = &sorted_clusters[0];
            let c2 = &sorted_clusters[1];
            let b1 = Self::segments_bbox(c1);
            let b2 = Self::segments_bbox(c2);

            let (left_cluster, _right_cluster, bbox_l, bbox_r) = if b1.0 < b2.0 {
                (c1, c2, b1, b2)
            } else {
                (c2, c1, b2, b1)
            };

            // Check if they are disjoint horizontally (side-by-side)
            if bbox_r.0 >= bbox_l.2 - 5.0 && c2.len() >= 4 {
                is_two_sided = true;
                midpoint = (bbox_l.2 + bbox_r.0) / 2.0;
                r_max_x = bbox_r.2;
                origin_x = bbox_l.0;
                origin_y = bbox_l.1;
                board_outline_segments = left_cluster.clone();
            } else {
                board_outline_segments = sorted_clusters[0].clone();
                let bbox = Self::segments_bbox(&board_outline_segments);
                origin_x = bbox.0;
                origin_y = bbox.1;
            }
        } else if !clusters.is_empty() {
            board_outline_segments = clusters[0].clone();
            let bbox = Self::segments_bbox(&board_outline_segments);
            origin_x = bbox.0;
            origin_y = bbox.1;
        }

        // Chain board outline segments
        let mut chained = Self::chain_segments(board_outline_segments, 5.0);
        for p in &mut chained {
            p.x -= origin_x;
            p.y -= origin_y;
        }
        board.outline_polygon = chained;

        // 2. Add Parts and Pins
        for raw_part in raw_parts {
            let part_id = board.parts.len();
            let is_bottom = if is_two_sided {
                if !raw_part.pins.is_empty() {
                    raw_part.pins.iter().any(|p| p.x >= midpoint)
                } else {
                    false
                }
            } else {
                false
            };

            let side = if is_bottom { Side::Bottom } else { Side::Top };
            let mut pin_indices = Vec::with_capacity(raw_part.pins.len());

            for (pin_seq, raw_pin) in raw_part.pins.into_iter().enumerate() {
                let pin_id = board.pins.len();
                let pin_name = if raw_pin.name.is_empty() {
                    format!("{}", pin_seq + 1)
                } else {
                    raw_pin.name
                };

                let (x, y) = if is_bottom {
                    (r_max_x - raw_pin.x, raw_pin.y - origin_y)
                } else {
                    (raw_pin.x - origin_x, raw_pin.y - origin_y)
                };

                let pin_rot = if is_bottom {
                    (360.0 - raw_pin.rotation_deg).rem_euclid(360.0)
                } else {
                    raw_pin.rotation_deg
                };

                let radius = match (raw_pin.width, raw_pin.height) {
                    (Some(w), Some(h)) => (w.min(h)) * 0.5,
                    (Some(w), None) => w * 0.5,
                    (None, Some(h)) => h * 0.5,
                    _ => 12.0,
                };

                board.pins.push(Pin {
                    id: pin_id,
                    part_id,
                    name: pin_name,
                    net_name: raw_pin.net_name,
                    pos: Point::new(x, y),
                    side,
                    radius,
                    shape: raw_pin.shape,
                    width: raw_pin.width,
                    height: raw_pin.height,
                    rotation_deg: pin_rot,
                });
                pin_indices.push(pin_id);
            }

            let part_rot = if is_bottom {
                (360.0 - raw_part.rotation_deg).rem_euclid(360.0)
            } else {
                raw_part.rotation_deg
            };

            board.parts.push(Part {
                id: part_id,
                name: raw_part.name,
                mfg_code: raw_part.mfg_code,
                side,
                part_type: PartType::Smd,
                p1: Point::ZERO,
                p2: Point::ZERO,
                pin_indices,
                bounding_box: None,
                outline: Vec::new(),
                rotation_deg: part_rot,
            });
        }

        // 3. Add Vias / Test Pads
        for raw_via in raw_vias {
            let (x, y, via_side) = if is_two_sided {
                if raw_via.x >= midpoint {
                    (r_max_x - raw_via.x, raw_via.y - origin_y, Side::Bottom)
                } else {
                    (raw_via.x - origin_x, raw_via.y - origin_y, Side::Top)
                }
            } else {
                (raw_via.x - origin_x, raw_via.y - origin_y, Side::Top)
            };

            let mut matched = false;
            if raw_via.is_through_hole && via_side == Side::Bottom {
                for existing in &mut board.vias {
                    if (existing.pos.x - x).abs() <= 0.5 && (existing.pos.y - y).abs() <= 0.5 {
                        existing.side = Side::Both;
                        matched = true;
                        break;
                    }
                }
            }

            if !matched {
                let via_id = board.vias.len();
                board.vias.push(Via {
                    id: via_id,
                    net_name: raw_via.net_name,
                    pos: Point::new(x, y),
                    side: if raw_via.is_through_hole { Side::Both } else { via_side },
                    radius: raw_via.radius,
                });
            }
        }

        board.rebuild_indices();

        // Synthesize outline bounding box with 20-mil margin if still empty
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
