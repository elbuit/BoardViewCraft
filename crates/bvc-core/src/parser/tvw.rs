//! Teleplan / TestView `.tvw` format parser.
//!
//! Handles both:
//! 1. Production binary walker (little-endian records, Pascal strings, aperture table, zero-copy pin records,
//!    component section matching refdes & PINS canonical mapping).
//! 2. Obfuscated ASCII (+3/+10 rotation cipher) delegating to `TestLinkEngine`.

use crate::board::Board;
use crate::model::{Part, PartType, Pin, Point, Rect, Side};
use crate::parser::test_link::{DialectMarkers, TestLinkEngine};
use crate::parser::{FormatParser, ParseError};
use std::collections::HashMap;

const TVW_MAGIC_SIG: (u8, &[u8]) = (0x13, b"O95w-28ps49m 02v9o.");
const TVW_VERSION_LE: [u8; 4] = [0x01, 0x00, 0x00, 0x00];
const MAX_COORD_CMILS: i64 = 5_000_000;

#[derive(Clone, Debug)]
struct TvwPin {
    part_index: u32,
    pin_local: u32,
    x: i32,
    y: i32,
}

#[derive(Clone, Debug)]
struct ComponentPin {
    pad_index: u32,
    pin_index: u32,
    name: String,
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
struct ComponentRecord {
    refdes: String,
    value: String,
    comment: String,
    footprint: String,
    cx: i32,
    cy: i32,
    bx1: i32,
    by1: i32,
    bx2: i32,
    by2: i32,
    kind: u32,
    pins: Vec<ComponentPin>,
}

pub struct TvwParser;

impl TvwParser {
    pub const fn new() -> Self {
        Self
    }

    pub fn is_production_binary(data: &[u8]) -> bool {
        if data.len() < 64 {
            return false;
        }
        if data[0] != TVW_MAGIC_SIG.0 || &data[1..1 + (TVW_MAGIC_SIG.0 as usize)] != TVW_MAGIC_SIG.1 {
            return false;
        }
        let off = 1 + (TVW_MAGIC_SIG.0 as usize);
        if &data[off..off + 4] != TVW_VERSION_LE {
            return false;
        }
        true
    }

    /// Deobfuscates ASCII TVW rotation cipher:
    /// - Digits '0'-'9': rotate -3
    /// - Lowercase 'a'-'z': rotate -10
    /// - Uppercase 'A'-'Z': rotate -10
    pub fn deobfuscate_ascii(data: &[u8]) -> Vec<u8> {
        data.iter()
            .map(|&b| {
                if (b'0'..=b'9').contains(&b) {
                    ((b - b'0' + 10 - 3) % 10) + b'0'
                } else if (b'a'..=b'z').contains(&b) {
                    ((b - b'a' + 26 - 10) % 26) + b'a'
                } else if (b'A'..=b'Z').contains(&b) {
                    ((b - b'A' + 26 - 10) % 26) + b'A'
                } else {
                    b
                }
            })
            .collect()
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

    #[inline(always)]
    fn rd_u8(buf: &[u8], off: usize) -> Option<u8> {
        buf.get(off).copied()
    }

    fn read_pin_record(buf: &[u8], off0: usize, region_end: usize) -> (Option<TvwPin>, usize) {
        let mut off = off0;
        if off + 18 > region_end {
            return (None, off);
        }

        let part_idx = match Self::rd_u32(buf, off) {
            Some(v) => { off += 4; v },
            None => return (None, off),
        };
        let pin_local = match Self::rd_u32(buf, off) {
            Some(v) => { off += 4; v },
            None => return (None, off),
        };
        let x = match Self::rd_i32(buf, off) {
            Some(v) => { off += 4; v },
            None => return (None, off),
        };
        let y = match Self::rd_i32(buf, off) {
            Some(v) => { off += 4; v },
            None => return (None, off),
        };
        let _flag1 = match Self::rd_u8(buf, off) {
            Some(v) => { off += 1; v },
            None => return (None, off),
        };
        let has_ext = match Self::rd_u8(buf, off) {
            Some(v) => { off += 1; v },
            None => return (None, off),
        };

        if has_ext != 0 {
            if off + 3 > region_end {
                return (None, off);
            }
            let sub_a = match Self::rd_u8(buf, off) {
                Some(v) => { off += 1; v },
                None => return (None, off),
            };
            if sub_a == 1 {
                if off + 12 > region_end {
                    return (None, off);
                }
                off += 12;
            }
            let sub_b = match Self::rd_u8(buf, off) {
                Some(v) => { off += 1; v },
                None => return (None, off),
            };
            if sub_b != 0 {
                if off + 16 > region_end {
                    return (None, off);
                }
                off += 16;
            }
            let sub_c = match Self::rd_u8(buf, off) {
                Some(v) => { off += 1; v },
                None => return (None, off),
            };
            if sub_c != 0 {
                if off + 16 > region_end {
                    return (None, off);
                }
                off += 16;
            }
        }

        if off + 1 > region_end {
            return (None, off);
        }
        let _flag3 = match Self::rd_u8(buf, off) {
            Some(v) => { off += 1; v },
            None => return (None, off),
        };

        (
            Some(TvwPin {
                part_index: part_idx,
                pin_local,
                x,
                y,
            }),
            off,
        )
    }

    #[inline]
    fn is_plausible_pin(p: &TvwPin) -> bool {
        if (p.x as i64).abs() > MAX_COORD_CMILS {
            return false;
        }
        if (p.y as i64).abs() > MAX_COORD_CMILS {
            return false;
        }
        if p.part_index >= (1u32 << 31) {
            return false;
        }
        if p.pin_local > 0x100000 {
            return false;
        }
        true
    }

    fn try_walk_pins_at(
        buf: &[u8],
        off: usize,
        region_end: usize,
        max_pin_count: u32,
        min_partial_ratio: f64,
    ) -> Option<(Vec<TvwPin>, usize, u32)> {
        if off + 12 > region_end {
            return None;
        }
        let first_count = Self::rd_u32(buf, off)?;
        let pin_count = Self::rd_u32(buf, off + 4)?;
        let mut off2 = off + 8;
        if pin_count == 0 {
            return Some((Vec::new(), off2, 0));
        }
        if pin_count > max_pin_count || first_count > max_pin_count {
            return None;
        }
        let _gap = Self::rd_u32(buf, off2)?;
        off2 += 4;

        let mut pins = Vec::new();
        let mut cur = off2;
        for _ in 0..pin_count {
            let (rec, new_off) = Self::read_pin_record(buf, cur, region_end);
            match rec {
                Some(r) if Self::is_plausible_pin(&r) => {
                    pins.push(r);
                    cur = new_off;
                }
                _ => break,
            }
        }
        if pins.is_empty() {
            return None;
        }
        if pin_count >= 10 && (pins.len() as f64) < f64::max(2.0, pin_count as f64 * min_partial_ratio) {
            return None;
        }
        Some((pins, cur, pin_count))
    }

    #[inline]
    fn looks_like_pin_record(buf: &[u8], off: usize, region_end: usize) -> bool {
        if off + 18 > region_end {
            return false;
        }
        let x = match Self::rd_i32(buf, off + 8) {
            Some(v) => v,
            None => return false,
        };
        let y = match Self::rd_i32(buf, off + 12) {
            Some(v) => v,
            None => return false,
        };
        (x as i64).abs() <= MAX_COORD_CMILS && (y as i64).abs() <= MAX_COORD_CMILS
    }

    #[inline]
    fn looks_like_pin_section_header(buf: &[u8], off: usize, region_end: usize) -> bool {
        if off + 12 + 18 > region_end || off + 16 > buf.len() {
            return false;
        }
        if buf[off + 3] != 0 || buf[off + 7] != 0 || buf[off + 11] != 0 || buf[off + 15] != 0 {
            return false;
        }
        let first_count = match Self::rd_u32(buf, off) {
            Some(v) => v,
            None => return false,
        };
        let pin_count = match Self::rd_u32(buf, off + 4) {
            Some(v) => v,
            None => return false,
        };
        if pin_count < 1 || pin_count > 100_000 || first_count > 100_000 {
            return false;
        }
        Self::looks_like_pin_record(buf, off + 12, region_end)
    }

    fn scan_best_pin_section(
        buf: &[u8],
        scan_start: usize,
        scan_end: usize,
        region_end: usize,
        step: usize,
    ) -> Option<(usize, Vec<TvwPin>, usize, u32)> {
        let mut best = None;
        let mut best_len = 0usize;
        let step = step.max(1);
        let mut cand = scan_start;
        while cand < scan_end {
            if Self::looks_like_pin_section_header(buf, cand, region_end) {
                if let Some((pins, end, declared)) = Self::try_walk_pins_at(buf, cand, region_end, 200_000, 0.5) {
                    if pins.len() > best_len {
                        best_len = pins.len();
                        best = Some((cand, pins, end, declared));
                    }
                }
            }
            cand += step;
        }
        best
    }

    fn scan_pascal_string_run(buf: &[u8], start: usize, end: usize) -> (Vec<String>, usize) {
        let mut names = Vec::new();
        let mut cur = start;
        while cur < end {
            let n = buf[cur] as usize;
            if n == 0 || cur + 1 + n > end || n > 64 {
                break;
            }
            let s_bytes = &buf[cur + 1..cur + 1 + n];
            if s_bytes.iter().all(|&b| (32..=126).contains(&b)) {
                names.push(String::from_utf8_lossy(s_bytes).into_owned());
                cur += 1 + n;
            } else {
                break;
            }
        }
        (names, cur)
    }

    fn scan_network_names(buf: &[u8]) -> Vec<String> {
        let end = buf.len();
        let safe_window = end.saturating_sub(end / 4 + 4096);
        let mut cur = safe_window;
        let mut best_names = Vec::new();

        while cur + 4 < end {
            let (names, _) = Self::scan_pascal_string_run(buf, cur, end);
            if names.len() >= best_names.len() && !names.is_empty() {
                best_names = names;
            }
            cur += 1;
        }
        best_names
    }

    /// Check if body matches refdes pattern: 1-5 letters followed by 1-5 digits.
    fn is_refdes_pattern(body: &[u8]) -> bool {
        if body.is_empty() || body.len() > 10 {
            return false;
        }
        let mut letters = 0;
        let mut digits = 0;
        let mut in_digits = false;

        for &b in body {
            if !in_digits {
                if b.is_ascii_alphabetic() {
                    letters += 1;
                    if letters > 5 {
                        return false;
                    }
                } else if b.is_ascii_digit() {
                    if letters == 0 {
                        return false;
                    }
                    in_digits = true;
                    digits += 1;
                } else {
                    return false;
                }
            } else if b.is_ascii_digit() {
                digits += 1;
                if digits > 5 {
                    return false;
                }
            } else {
                return false;
            }
        }
        letters > 0 && digits > 0
    }

    fn scan_component_records(buf: &[u8]) -> Vec<ComponentRecord> {
        let n = buf.len();
        let mut candidates = Vec::new();

        for i in 0..n.saturating_sub(40) {
            let plen = buf[i] as usize;
            if !(2..=8).contains(&plen) || i + 1 + plen + 32 > n {
                continue;
            }
            let body = &buf[i + 1..i + 1 + plen];
            if !Self::is_refdes_pattern(body) {
                continue;
            }
            // Skip false-positive layer names like L1..L15
            if (plen == 2 || plen == 3) && body[0] == b'L' && body[1..].iter().all(|b| b.is_ascii_digit()) {
                continue;
            }

            let o = i + 1 + plen;
            let bx1 = match Self::rd_i32(buf, o) { Some(v) => v, None => continue };
            let by1 = match Self::rd_i32(buf, o + 4) { Some(v) => v, None => continue };
            let bx2 = match Self::rd_i32(buf, o + 8) { Some(v) => v, None => continue };
            let by2 = match Self::rd_i32(buf, o + 12) { Some(v) => v, None => continue };
            let cx = match Self::rd_i32(buf, o + 16) { Some(v) => v, None => continue };
            let cy = match Self::rd_i32(buf, o + 20) { Some(v) => v, None => continue };

            if [bx1, by1, bx2, by2, cx, cy].iter().any(|&v| (v as i64).abs() > 10_000_000) {
                continue;
            }
            if bx1 >= bx2 || by1 >= by2 {
                continue;
            }
            let w = bx2 - bx1;
            let h = by2 - by1;
            if w < 100 || h < 100 || w > 5_000_000 || h > 5_000_000 {
                continue;
            }
            let mx = w / 2;
            let my = h / 2;
            if !(bx1 - mx <= cx && cx <= bx2 + mx && by1 - my <= cy && cy <= by2 + my) {
                continue;
            }
            candidates.push(i);
        }

        let mut components = Vec::new();
        for idx in 0..candidates.len() {
            let start = candidates[idx];
            let next_start = if idx + 1 < candidates.len() {
                candidates[idx + 1]
            } else {
                n
            };

            if let Some(comp) = Self::parse_component_at(buf, start, next_start) {
                components.push(comp);
            }
        }
        components
    }

    fn parse_component_at(buf: &[u8], start: usize, region_end: usize) -> Option<ComponentRecord> {
        let mut o = start;
        let plen = buf.get(o).copied()? as usize;
        o += 1;
        if !(1..=12).contains(&plen) || o + plen + 32 > region_end {
            return None;
        }
        let refdes_bytes = &buf[o..o + plen];
        if !refdes_bytes.iter().all(|&b| (32..=126).contains(&b)) {
            return None;
        }
        let refdes = String::from_utf8_lossy(refdes_bytes).into_owned();
        o += plen;

        let bx1 = Self::rd_i32(buf, o)?; o += 4;
        let by1 = Self::rd_i32(buf, o)?; o += 4;
        let bx2 = Self::rd_i32(buf, o)?; o += 4;
        let by2 = Self::rd_i32(buf, o)?; o += 4;
        let cx = Self::rd_i32(buf, o)?; o += 4;
        let cy = Self::rd_i32(buf, o)?; o += 4;
        let _rotation = Self::rd_u32(buf, o)?; o += 4;
        let kind = Self::rd_u32(buf, o)?; o += 4;
        if o + 12 > region_end {
            return None;
        }
        o += 12;

        let mut value = String::new();
        let mut comment = String::new();
        let mut footprint = String::new();
        let mut string_region_ok = true;

        if o + 2 <= region_end {
            let sep_v = buf[o];
            let plen_v = buf[o + 1] as usize;
            if sep_v == 0x01 && plen_v <= 64 && o + 2 + plen_v <= region_end {
                o += 2;
                value = String::from_utf8_lossy(&buf[o..o + plen_v]).into_owned();
                o += plen_v;
            } else {
                string_region_ok = false;
            }
        }
        if string_region_ok && o + 1 <= region_end {
            let plen_a = buf[o] as usize;
            if plen_a <= 64 && o + 1 + plen_a <= region_end {
                o += 1;
                comment = String::from_utf8_lossy(&buf[o..o + plen_a]).into_owned();
                o += plen_a;
            } else {
                string_region_ok = false;
            }
        }
        if string_region_ok && o + 1 <= region_end {
            let plen_b = buf[o] as usize;
            if plen_b <= 64 && o + 1 + plen_b <= region_end {
                o += 1 + plen_b; // duplicate comment
            } else {
                string_region_ok = false;
            }
        }
        if string_region_ok && o + 1 <= region_end {
            let plen_f = buf[o] as usize;
            if plen_f <= 64 && o + 1 + plen_f <= region_end {
                o += 1;
                footprint = String::from_utf8_lossy(&buf[o..o + plen_f]).into_owned();
                o += plen_f;
            }
        }

        let mut pins = Vec::new();
        if o + 9 <= region_end {
            o += 5;
            let cnt_u32 = Self::rd_u32(buf, o).unwrap_or(0);
            o += 4;
            let pin_count = if cnt_u32 <= 10_000 { cnt_u32 } else { 0 };

            if o + 8 <= region_end {
                o += 8; // _sep_a, _sep_b
                for _ in 0..pin_count {
                    if o + 12 > region_end {
                        break;
                    }
                    let pad_idx = Self::rd_u32(buf, o)?; o += 4;
                    let _u = Self::rd_u32(buf, o)?; o += 4;
                    let pin_idx = Self::rd_u32(buf, o)?; o += 4;
                    if o + 1 > region_end {
                        break;
                    }
                    let name_len = buf[o] as usize;
                    o += 1;
                    if name_len > 16 || o + name_len + 4 > region_end {
                        break;
                    }
                    let pin_name = String::from_utf8_lossy(&buf[o..o + name_len]).into_owned();
                    o += name_len;
                    let _trailer = Self::rd_u32(buf, o)?; o += 4;

                    pins.push(ComponentPin {
                        pad_index: pad_idx,
                        pin_index: pin_idx,
                        name: pin_name,
                    });
                }
            }
        }

        Some(ComponentRecord {
            refdes,
            value,
            comment,
            footprint,
            cx,
            cy,
            bx1,
            by1,
            bx2,
            by2,
            kind,
            pins,
        })
    }

    /// Walk binary TVW
    pub fn parse_binary(buf: &[u8]) -> Result<Board, ParseError> {
        let mut board = Board::new();

        // 1. Scan network names table
        let net_names = Self::scan_network_names(buf);

        // 2. Scan component records
        let components = Self::scan_component_records(buf);

        // 3. Scan pin sections across layers
        // Find layers marked by TOP, BOTTOM, etc.
        let mut layer_markers = Vec::new();
        let target_names: &[&[u8]] = &[b"TOP", b"BOTTOM", b"top", b"bottom"];
        for &layer_name in target_names {
            let marker = [layer_name.len() as u8];
            let mut search_from = 0;
            while let Some(pos) = buf[search_from..].windows(layer_name.len() + 1).position(|w| w[0] == marker[0] && &w[1..] == layer_name) {
                let actual_pos = search_from + pos;
                layer_markers.push((actual_pos, String::from_utf8_lossy(layer_name).to_uppercase()));
                search_from = actual_pos + 1;
            }
        }
        layer_markers.sort_by_key(|&(pos, _)| pos);

        // Walk pin sections
        let mut top_pins = Vec::new();
        let mut bot_pins = Vec::new();

        if !layer_markers.is_empty() {
            for i in 0..layer_markers.len() {
                let (marker_pos, ref name) = layer_markers[i];
                let layer_end = if i + 1 < layer_markers.len() {
                    layer_markers[i + 1].0
                } else {
                    buf.len().saturating_sub(buf.len() / 4)
                };

                if marker_pos >= layer_end {
                    continue;
                }

                let is_top = name.contains("TOP");
                let scan_start = marker_pos;
                let scan_end = layer_end.saturating_sub(30);

                if let Some((_, pins, _, _)) = Self::scan_best_pin_section(buf, scan_start, scan_end, layer_end, 4) {
                    if is_top {
                        if pins.len() > top_pins.len() {
                            top_pins = pins;
                        }
                    } else if pins.len() > bot_pins.len() {
                        bot_pins = pins;
                    }
                }
            }
        }

        // If markers didn't yield pins, fallback to brute force scan across whole file
        if top_pins.is_empty() && bot_pins.is_empty() {
            let end = buf.len().saturating_sub(buf.len() / 4);
            if let Some((_, pins, _, _)) = Self::scan_best_pin_section(buf, 64, end.saturating_sub(30), end, 4) {
                top_pins = pins;
            }
        }

        // Helper to resolve net name from part_index
        let resolve_net = |part_idx: u32| -> String {
            let idx = part_idx as usize;
            if idx < net_names.len() {
                net_names[idx].clone()
            } else {
                String::new()
            }
        };

        // Construct parts from ComponentRecords
        let mut part_id_map: HashMap<String, usize> = HashMap::new();
        let mut claimed_top_indices = std::collections::HashSet::new();
        let mut claimed_bot_indices = std::collections::HashSet::new();

        for comp in &components {
            let side = if (comp.kind & 1) == 1 { Side::Top } else { Side::Bottom };
            let part_id = board.parts.len();
            part_id_map.insert(comp.refdes.clone(), part_id);

            board.parts.push(Part {
                id: part_id,
                name: comp.refdes.clone(),
                mfg_code: comp.value.clone(),
                side,
                part_type: PartType::Smd,
                p1: Point::new(comp.bx1 as f64 / 100.0, comp.by1 as f64 / 100.0),
                p2: Point::new(comp.bx2 as f64 / 100.0, comp.by2 as f64 / 100.0),
                pin_indices: Vec::new(),
                bounding_box: Some(Rect {
                    min_x: comp.bx1 as f64 / 100.0,
                    min_y: comp.by1 as f64 / 100.0,
                    max_x: comp.bx2 as f64 / 100.0,
                    max_y: comp.by2 as f64 / 100.0,
                }),
            });

            // Canonical pin mapping: pad_index // 8
            for cp in &comp.pins {
                let idx = (cp.pad_index / 8) as usize;
                let top_pr = top_pins.get(idx);
                let bot_pr = bot_pins.get(idx);

                let chosen = match (top_pr, bot_pr) {
                    (Some(tp), Some(bp)) => {
                        let dt = (tp.x - comp.cx).pow(2) + (tp.y - comp.cy).pow(2);
                        let db = (bp.x - comp.cx).pow(2) + (bp.y - comp.cy).pow(2);
                        if dt <= db {
                            claimed_top_indices.insert(idx);
                            Some((Side::Top, tp))
                        } else {
                            claimed_bot_indices.insert(idx);
                            Some((Side::Bottom, bp))
                        }
                    }
                    (Some(tp), None) => {
                        claimed_top_indices.insert(idx);
                        Some((Side::Top, tp))
                    }
                    (None, Some(bp)) => {
                        claimed_bot_indices.insert(idx);
                        Some((Side::Bottom, bp))
                    }
                    (None, None) => None,
                };

                if let Some((pin_side, pr)) = chosen {
                    let pin_id = board.pins.len();
                    let net = resolve_net(pr.part_index);
                    board.pins.push(Pin {
                        id: pin_id,
                        part_id,
                        name: if cp.name.is_empty() { cp.pin_index.to_string() } else { cp.name.clone() },
                        net_name: net,
                        pos: Point::new(pr.x as f64 / 100.0, pr.y as f64 / 100.0),
                        side: pin_side,
                        radius: 8.0,
                    });
                    board.parts[part_id].pin_indices.push(pin_id);
                }
            }
        }

        // Add remaining unclaimed pins (e.g. vias, test points, or if no component section existed)
        let mut add_unclaimed_pins = |pins: &[TvwPin], claimed: &std::collections::HashSet<usize>, side: Side| {
            for (idx, pr) in pins.iter().enumerate() {
                if claimed.contains(&idx) {
                    continue;
                }
                let pin_id = board.pins.len();
                let net = resolve_net(pr.part_index);

                // Check if pin falls inside any component bbox
                let px = pr.x as f64 / 100.0;
                let py = pr.y as f64 / 100.0;
                let mut owning_part = None;

                for part in &mut board.parts {
                    if part.side == side {
                        if let Some(bbox) = &part.bounding_box {
                            if px >= bbox.min_x - 5.0 && px <= bbox.max_x + 5.0 && py >= bbox.min_y - 5.0 && py <= bbox.max_y + 5.0 {
                                owning_part = Some(part.id);
                                break;
                            }
                        }
                    }
                }

                let final_part_id = if let Some(pid) = owning_part {
                    board.parts[pid].pin_indices.push(pin_id);
                    pid
                } else {
                    usize::MAX
                };

                board.pins.push(Pin {
                    id: pin_id,
                    part_id: final_part_id,
                    name: pr.pin_local.to_string(),
                    net_name: net,
                    pos: Point::new(px, py),
                    side,
                    radius: 8.0,
                });
            }
        };

        add_unclaimed_pins(&top_pins, &claimed_top_indices, Side::Top);
        add_unclaimed_pins(&bot_pins, &claimed_bot_indices, Side::Bottom);

        board.rebuild_indices();

        // Synthesize outline bounding box with 20-mil margin
        if board.outline_polygon.is_empty() && (!board.pins.is_empty() || !board.parts.is_empty()) {
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

impl Default for TvwParser {
    fn default() -> Self {
        Self::new()
    }
}

impl FormatParser for TvwParser {
    fn name(&self) -> &'static str {
        "TVW"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["tvw"]
    }

    fn verify_format(&self, data: &[u8]) -> bool {
        if Self::is_production_binary(data) {
            return true;
        }
        let decoded = Self::deobfuscate_ascii(&data[..data.len().min(1024)]);
        let s = String::from_utf8_lossy(&decoded);
        s.contains("var_data:") || s.contains("Format:") || s.contains("Parts:")
    }

    fn parse(&self, data: &[u8]) -> Result<Board, ParseError> {
        if Self::is_production_binary(data) {
            return Self::parse_binary(data);
        }

        // Deobfuscate ASCII TVW
        let decoded = Self::deobfuscate_ascii(data);
        let s = String::from_utf8_lossy(&decoded);
        TestLinkEngine::parse_text(&s, &DialectMarkers::default())
    }
}
