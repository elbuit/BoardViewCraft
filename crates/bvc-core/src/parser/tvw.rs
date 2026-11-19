//! Teleplan / TestView `.tvw` format parser.
//!
//! Handles both:
//! 1. Production binary walker with full physical layer decoding (TOP / BOTTOM),
//!    UID-to-physical-pad canonical indexing (`uid / 8`), coordinate axis swapping `(x=raw_y, y=raw_x)`,
//!    and validated component & net registries.
//! 2. Obfuscated ASCII (+3/+10 rotation cipher) delegating to `TestLinkEngine`.

use crate::board::Board;
use crate::model::{Part, PartType, Pin, Point, Rect, Side, Via};
use crate::parser::test_link::{DialectMarkers, TestLinkEngine};
use crate::parser::{FormatParser, ParseError};
use std::collections::HashMap;

const TVW_MAGIC_SIG: (u8, &[u8]) = (0x13, b"O95w-28ps49m 02v9o.");
const TVW_VERSION_LE: [u8; 4] = [0x01, 0x00, 0x00, 0x00];
const PAD_RENDER_SHRINK: f64 = 0.65;
const DEFAULT_PAD_RADIUS_MILS: f64 = 8.0;

const TVW_LAYER_TOP: u32 = 1;
const TVW_LAYER_BOTTOM: u32 = 2;

#[derive(Clone, Debug)]
struct TvwLayerPad {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    #[allow(dead_code)]
    shape_type: u32,
    net: i32,
}

#[derive(Clone, Debug)]
struct TvwLayer {
    #[allow(dead_code)]
    name: String,
    side: Side,
    pads: Vec<TvwLayerPad>,
}

#[derive(Clone, Debug)]
struct LayerHeader {
    #[allow(dead_code)]
    index: usize,
    #[allow(dead_code)]
    name: String,
    #[allow(dead_code)]
    layer_type: u32,
    layer: Option<TvwLayer>,
}

#[derive(Clone, Debug)]
struct TvwPinRef {
    uid: u32,
    ordinal: u32,
    name: String,
}

#[derive(Clone, Debug)]
struct TvwPinGroup {
    kind: u32,
    pins: Vec<TvwPinRef>,
}

#[derive(Clone, Debug)]
struct TvwComponent {
    refdes: String,
    value: String,
    package: String,
    min_x: i32,
    min_y: i32,
    max_x: i32,
    max_y: i32,
    #[allow(dead_code)]
    cx: i32,
    #[allow(dead_code)]
    cy: i32,
    #[allow(dead_code)]
    rotation: i32,
    pin_groups: Vec<TvwPinGroup>,
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

    fn read_pascal_string(buf: &[u8], off: usize, empty_allowed: bool) -> Option<(String, usize)> {
        let len = *buf.get(off)? as usize;
        if len == 0 && !empty_allowed {
            return None;
        }
        let end = off.checked_add(1)?.checked_add(len)?;
        if end > buf.len() {
            return None;
        }
        let bytes = &buf[off + 1..end];
        let s = String::from_utf8_lossy(bytes).into_owned();
        Some((s, end))
    }

    /// Finds and parses the net names table
    fn read_net_table(buf: &[u8]) -> Option<(Vec<String>, usize, usize)> {
        const PREFIX: usize = 69;
        let len = buf.len();
        if len < 100 {
            return None;
        }

        let mut candidate_offsets = Vec::new();
        // Priority 1: standard prefix pattern (words 7 and 4)
        for q in PREFIX..len.saturating_sub(9) {
            if buf[q - 56] == 7 && buf[q - 44] == 4 {
                if let (Some(w1), Some(w2)) = (Self::rd_u32(buf, q - 56), Self::rd_u32(buf, q - 44)) {
                    if w1 == 7 && w2 == 4 {
                        candidate_offsets.push(q);
                    }
                }
            }
        }

        // Priority 2: direct search for duplicate counts
        let try_table_at = |q: usize| -> Option<(Vec<String>, usize, usize)> {
            let c1 = Self::rd_u32(buf, q)?;
            let c2 = Self::rd_u32(buf, q + 4)?;
            if c1 == 0 || c1 > 100_000 || c1 != c2 {
                return None;
            }
            let mut cur = q + 8;
            let mut names = Vec::with_capacity(c1 as usize);
            for _ in 0..c1 {
                let (name, next) = Self::read_pascal_string(buf, cur, true)?;
                names.push(name);
                cur = next;
            }
            if cur + 21 > len {
                return None;
            }
            let _px = Self::rd_i32(buf, cur)?;
            let _py = Self::rd_i32(buf, cur + 4)?;
            let kind = Self::rd_u32(buf, cur + 8)?;
            if kind != 4 {
                return None;
            }
            let (probe_name, probe_end) = Self::read_pascal_string(buf, cur + 12, false)?;
            if probe_name != "ProbeDB" {
                return None;
            }
            let mut after_probe = probe_end;
            if probe_end + 8 <= len {
                let size = Self::rd_u32(buf, probe_end)?;
                let packs = Self::rd_u32(buf, probe_end + 4)?;
                if size > 0 && size <= 2_000_000 && packs > 0 && packs <= 4096 {
                    after_probe = probe_end + 8;
                }
            }
            let table_start = if q >= PREFIX { q - PREFIX } else { q };
            Some((names, table_start, after_probe))
        };

        for q in candidate_offsets {
            if let Some(res) = try_table_at(q) {
                return Some(res);
            }
        }

        for q in 0..len.saturating_sub(9) {
            if let Some(res) = try_table_at(q) {
                return Some(res);
            }
        }

        None
    }

    /// Read layer headers and decoded pads from top/bottom layers
    fn read_layers(buf: &[u8], net_table_start: usize, net_count: usize) -> (Vec<LayerHeader>, HashMap<usize, TvwLayer>) {
        let mut headers = Vec::new();
        let mut layers_map = HashMap::new();
        let mut p = 0;

        while p + 35 <= net_table_start {
            // Check 16-byte fixed layer prefix
            if buf[p] != 0 || buf[p + 1] != 0 || buf[p + 2] != 0 || buf[p + 3] != 0
                || (buf[p + 4] != 1 && buf[p + 4] != 3) || buf[p + 5] != 0 || buf[p + 6] != 0 || buf[p + 7] != 0
                || buf[p + 8] != 2 || buf[p + 9] != 0 || buf[p + 10] != 0 || buf[p + 11] != 0
                || buf[p + 12] != 1 || buf[p + 13] != 0 || buf[p + 14] != 0 || buf[p + 15] != 0
            {
                p += 1;
                continue;
            }

            let mut cur = p + 16;
            let name_res = Self::read_pascal_string(buf, cur, true);
            let (name, next1) = match name_res {
                Some(v) => v,
                None => { p += 1; continue; }
            };
            cur = next1;

            let initial_res = Self::read_pascal_string(buf, cur, true);
            let (initial, next2) = match initial_res {
                Some(v) => v,
                None => { p += 1; continue; }
            };
            cur = next2;

            let desc_res = Self::read_pascal_string(buf, cur, true);
            cur = match desc_res {
                Some((_, next3)) => next3,
                None => { p += 1; continue; }
            };

            let l_type = match Self::rd_u32(buf, cur) {
                Some(v) => { cur += 4; v },
                None => { p += 1; continue; }
            };
            cur += 8; // skip 2 words
            let dcode_count = match Self::rd_u32(buf, cur) {
                Some(v) => { cur += 4; v },
                None => { p += 1; continue; }
            };

            let named = !name.is_empty() && !initial.is_empty();
            if !named {
                if buf[p + 4] == 3 && name.is_empty() && l_type == 0 && dcode_count == 0 && cur == p + 35 {
                    // Empty layer slot
                    headers.push(LayerHeader {
                        index: headers.len(),
                        name: String::new(),
                        layer_type: 0,
                        layer: None,
                    });
                    p += 82;
                    continue;
                }
                p += 1;
                continue;
            }

            let header_idx = headers.len();
            if l_type != TVW_LAYER_TOP && l_type != TVW_LAYER_BOTTOM {
                headers.push(LayerHeader {
                    index: header_idx,
                    name,
                    layer_type: l_type,
                    layer: None,
                });
                p = cur;
                continue;
            }

            if dcode_count < 10 {
                p = cur;
                continue;
            }

            // Decode D-code apertures
            let mut defs = Vec::with_capacity((dcode_count - 10) as usize);
            let mut valid_defs = true;
            for _ in 0..(dcode_count - 10) {
                if cur + 20 > net_table_start {
                    valid_defs = false;
                    break;
                }
                if Self::rd_u32(buf, cur) != Some(1) {
                    valid_defs = false;
                    break;
                }
                cur += 4;
                // Height then Width (disk Y, X order)
                let h = match Self::rd_i32(buf, cur) { Some(v) => { cur += 4; v }, None => { valid_defs = false; break; } };
                let w = match Self::rd_i32(buf, cur) { Some(v) => { cur += 4; v }, None => { valid_defs = false; break; } };
                let stype = match Self::rd_u32(buf, cur) { Some(v) => { cur += 4; v }, None => { valid_defs = false; break; } };
                let extra = match Self::rd_u32(buf, cur) { Some(v) => { cur += 4; v }, None => { valid_defs = false; break; } };

                if stype <= 3 {
                    cur += 4;
                } else if stype == 5 && extra == 0 {
                    // Custom macro
                    if let Some((_, cnext)) = Self::read_pascal_string(buf, cur, false) {
                        cur = cnext + 16;
                        if let Some(c_cnt) = Self::rd_u32(buf, cur) {
                            cur += 4;
                            for _ in 0..c_cnt {
                                if cur + 16 > net_table_start { valid_defs = false; break; }
                                let prim = match Self::rd_u32(buf, cur) { Some(v) => { cur += 16; v }, None => { valid_defs = false; break; } };
                                if prim == 2 {
                                    if let Some(edges) = Self::rd_u32(buf, cur) {
                                        cur += 4 + (edges as usize * 8);
                                    } else { valid_defs = false; break; }
                                } else if prim == 5 {
                                    cur += 20;
                                }
                            }
                        }
                    }
                }
                defs.push((w, h, stype));
            }

            if !valid_defs {
                p = cur;
                continue;
            }

            // Fixed trailer [1, 0, 1]
            if cur + 12 > net_table_start || Self::rd_u32(buf, cur) != Some(1) || Self::rd_u32(buf, cur + 4) != Some(0) || Self::rd_u32(buf, cur + 8) != Some(1) {
                p = cur;
                continue;
            }
            cur += 12;

            let pad_count = match Self::rd_u32(buf, cur) {
                Some(v) => { cur += 4; v },
                None => { p = cur; continue; }
            };
            if Self::rd_u32(buf, cur) != Some(2) {
                p = cur;
                continue;
            }
            cur += 4;

            let mut pads = Vec::with_capacity(pad_count as usize);
            let mut valid_pads = true;
            for _ in 0..pad_count {
                if cur + 18 > net_table_start {
                    valid_pads = false;
                    break;
                }
                let net = match Self::rd_i32(buf, cur) { Some(v) => { cur += 4; v }, None => { valid_pads = false; break; } };
                let dcode = match Self::rd_u32(buf, cur) { Some(v) => { cur += 4; v }, None => { valid_pads = false; break; } };
                // Disk coordinates are (Y, X)
                let y = match Self::rd_i32(buf, cur) { Some(v) => { cur += 4; v }, None => { valid_pads = false; break; } };
                let x = match Self::rd_i32(buf, cur) { Some(v) => { cur += 4; v }, None => { valid_pads = false; break; } };
                let flag = match Self::rd_u8(buf, cur) { Some(v) => { cur += 1; v }, None => { valid_pads = false; break; } };
                let details = match Self::rd_u8(buf, cur) { Some(v) => { cur += 1; v }, None => { valid_pads = false; break; } };

                let (mut w, mut h, stype) = if (dcode as usize) >= 10 && ((dcode - 10) as usize) < defs.len() {
                    defs[(dcode - 10) as usize]
                } else {
                    (0, 0, 0)
                };

                if details != 0 {
                    let extended = match Self::rd_u8(buf, cur) { Some(v) => { cur += 1; v }, None => { valid_pads = false; break; } };
                    if extended != 0 {
                        cur += 12;
                    }
                    let geom = match Self::rd_u8(buf, cur) { Some(v) => { cur += 1; v }, None => { valid_pads = false; break; } };
                    if geom == 1 {
                        let min_y = match Self::rd_i32(buf, cur) { Some(v) => { cur += 4; v }, None => { valid_pads = false; break; } };
                        let min_x = match Self::rd_i32(buf, cur) { Some(v) => { cur += 4; v }, None => { valid_pads = false; break; } };
                        let max_y = match Self::rd_i32(buf, cur) { Some(v) => { cur += 4; v }, None => { valid_pads = false; break; } };
                        let max_x = match Self::rd_i32(buf, cur) { Some(v) => { cur += 4; v }, None => { valid_pads = false; break; } };
                        w = (max_x - min_x).abs();
                        h = (max_y - min_y).abs();
                        let hole = match Self::rd_u8(buf, cur) { Some(v) => { cur += 1; v }, None => { valid_pads = false; break; } };
                        if hole != 0 {
                            cur += 16;
                        }
                    } else if geom == 0 && (flag != 0 || (Self::rd_u32(buf, cur) == Some(1) && Self::rd_u32(buf, cur + 4) == Some(0) && Self::rd_u8(buf, cur + 8) == Some(0))) {
                        cur += 9;
                        h = match Self::rd_i32(buf, cur) { Some(v) => { cur += 4; v * 2 }, None => { valid_pads = false; break; } };
                        w = match Self::rd_i32(buf, cur) { Some(v) => { cur += 4; v * 2 }, None => { valid_pads = false; break; } };
                    } else if geom == 0 {
                        let hole = match Self::rd_u8(buf, cur) { Some(v) => { cur += 1; v }, None => { valid_pads = false; break; } };
                        if hole != 0 {
                            cur += 16;
                        }
                    }
                }
                cur += 1; // opaque final flag

                pads.push(TvwLayerPad {
                    x,
                    y,
                    width: w,
                    height: h,
                    shape_type: stype,
                    net: if net >= 0 && (net as usize) < net_count { net } else { -1 },
                });
            }

            if !valid_pads {
                p = cur;
                continue;
            }

            let side = if l_type == TVW_LAYER_TOP { Side::Top } else { Side::Bottom };
            let layer = TvwLayer {
                name: name.clone(),
                side,
                pads,
            };

            layers_map.insert(header_idx, layer.clone());
            headers.push(LayerHeader {
                index: header_idx,
                name,
                layer_type: l_type,
                layer: Some(layer),
            });

            p = cur;
        }

        (headers, layers_map)
    }

    /// Read components starting after the net table / ProbeDB registry
    fn read_components(buf: &[u8], after_probe: usize) -> Vec<TvwComponent> {
        let len = buf.len();
        let stop = (after_probe + 256 * 1024).min(len.saturating_sub(8));
        let mut header_opt = None;
        for q in after_probe..stop {
            if let (Some(c), Some(k)) = (Self::rd_u32(buf, q), Self::rd_u32(buf, q + 4)) {
                if c > 0 && c <= 250_000 && k == 12 {
                    header_opt = Some((q + 8, c as usize));
                    break;
                }
            }
        }

        let (mut cur, total_count) = match header_opt {
            Some(v) => v,
            None => return Vec::new(),
        };

        let mut components = Vec::with_capacity(total_count);
        for _ in 0..total_count {
            if cur >= len {
                break;
            }
            let (refdes, a) = match Self::read_pascal_string(buf, cur, false) {
                Some(v) => v,
                None => break,
            };
            if a + 36 > len {
                break;
            }

            let min_y = Self::rd_i32(buf, a).unwrap_or(0);
            let min_x = Self::rd_i32(buf, a + 4).unwrap_or(0);
            let max_y = Self::rd_i32(buf, a + 8).unwrap_or(0);
            let max_x = Self::rd_i32(buf, a + 12).unwrap_or(0);
            let cy = Self::rd_i32(buf, a + 16).unwrap_or(0);
            let cx = Self::rd_i32(buf, a + 20).unwrap_or(0);
            let rot = Self::rd_i32(buf, a + 24).unwrap_or(0);
            let classif = Self::rd_u32(buf, a + 32).unwrap_or(0);

            let mut matched = false;
            for &fixed in &[44usize, 48usize] {
                if a + fixed >= len {
                    continue;
                }
                let flag = buf[a + fixed];
                let mut tails = Vec::new();
                if flag == 0 {
                    if classif == 18 || classif == 0xffff_ffff {
                        tails.push((String::new(), String::new(), a + fixed + 1));
                    }
                } else if flag == 1 {
                    if let Some((val, v_end)) = Self::read_pascal_string(buf, a + fixed + 1, true) {
                        let e1_res = Self::read_pascal_string(buf, v_end, true);
                        let e2_res = if let Some((_, e1_end)) = &e1_res {
                            Self::read_pascal_string(buf, *e1_end, true)
                        } else {
                            None
                        };
                        let mut pkg_offsets = vec![v_end + 2];
                        if let Some((_, e2_end)) = e2_res {
                            pkg_offsets.push(e2_end);
                        }
                        for pkg_off in pkg_offsets {
                            if let Some((pkg, pkg_end)) = Self::read_pascal_string(buf, pkg_off, classif == 18) {
                                if let Some((_serial, s_end)) = Self::read_pascal_string(buf, pkg_end, true) {
                                    tails.push((val.clone(), pkg, s_end));
                                }
                            }
                        }
                    }
                }

                for (val, pkg, s_end) in tails {
                    let reserved = if fixed == 44 { 4 } else { 0 };
                    if s_end + 12 + reserved > len || Self::rd_u32(buf, s_end) != Some(0) {
                        continue;
                    }
                    let p_cnt = match Self::rd_u32(buf, s_end + 4) {
                        Some(v) => v,
                        None => continue,
                    };
                    let p_kind = match Self::rd_u32(buf, s_end + 8) {
                        Some(v) => v,
                        None => continue,
                    };
                    if p_cnt > 16_384 {
                        continue;
                    }

                    let mut p_cur = s_end + 12 + reserved;
                    let mut g_cnt = p_cnt;
                    let mut g_kind = p_kind;
                    let mut pin_groups = Vec::new();
                    let mut complete = true;

                    for g in 0..2 {
                        if g_cnt > 16_384 {
                            complete = false;
                            break;
                        }
                        let mut grp_pins = Vec::with_capacity(g_cnt as usize);
                        for pi in 0..g_cnt {
                            if p_cur + 13 > len || Self::rd_u32(buf, p_cur + 4) != Some(0) {
                                complete = false;
                                break;
                            }
                            let uid = match Self::rd_u32(buf, p_cur) { Some(v) => v, None => { complete = false; break; } };
                            let ord_idx = match Self::rd_u32(buf, p_cur + 8) { Some(v) => v, None => { complete = false; break; } };
                            let is_test_pt = g_cnt == 1 && (classif == 18 || pkg.is_empty());
                            let (pname, p_next) = match Self::read_pascal_string(buf, p_cur + 12, is_test_pt) {
                                Some(v) => v,
                                None => { complete = false; break; }
                            };
                            grp_pins.push(TvwPinRef {
                                uid,
                                ordinal: ord_idx,
                                name: pname,
                            });
                            p_cur = p_next;
                            if pi + 1 < g_cnt || reserved == 0 {
                                if Self::rd_u32(buf, p_cur) != Some(0) {
                                    complete = false;
                                    break;
                                }
                                p_cur += 4;
                            }
                        }

                        if !complete || grp_pins.len() != g_cnt as usize {
                            complete = false;
                            break;
                        }
                        pin_groups.push(TvwPinGroup {
                            kind: g_kind,
                            pins: grp_pins,
                        });

                        if reserved == 0 || g_cnt == 0 || g == 1 {
                            break;
                        }
                        g_cnt = match Self::rd_u32(buf, p_cur) { Some(v) => { p_cur += 4; v }, None => break };
                        if g_cnt == 0 {
                            break;
                        }
                        if p_cur + 8 > len || Self::rd_u32(buf, p_cur + 4) != Some(0) {
                            complete = false;
                            break;
                        }
                        g_kind = match Self::rd_u32(buf, p_cur) { Some(v) => { p_cur += 8; v }, None => { complete = false; break; } };
                    }

                    if complete {
                        components.push(TvwComponent {
                            refdes: refdes.clone(),
                            value: val,
                            package: pkg,
                            min_x,
                            min_y,
                            max_x,
                            max_y,
                            cx,
                            cy,
                            rotation: rot,
                            pin_groups,
                        });
                        cur = p_cur;
                        matched = true;
                        break;
                    }
                }

                if matched {
                    break;
                }
            }

            if !matched {
                break;
            }
        }

        components
    }

    /// Walk binary TVW
    pub fn parse_binary(buf: &[u8]) -> Result<Board, ParseError> {
        let (net_names, net_table_start, after_probe) = match Self::read_net_table(buf) {
            Some(v) => v,
            None => return Err(ParseError::UnrecognizedFormat("TVW: missing net name registry".to_string())),
        };

        let (headers, layers_map) = Self::read_layers(buf, net_table_start, net_names.len());
        let components = Self::read_components(buf, after_probe);

        if components.is_empty() {
            return Err(ParseError::UnrecognizedFormat("TVW: no valid components found".to_string()));
        }

        let mut board = Board::new();

        // Helper to resolve layer by reference kind
        let resolve_layer = |kind: u32| -> Option<&TvwLayer> {
            if let Some(hdr) = headers.get(kind as usize) {
                if let Some(layer) = &hdr.layer {
                    return Some(layer);
                }
            }
            // Fallback for legacy kinds
            if kind == 2 {
                for layer in layers_map.values() {
                    if layer.side == Side::Top {
                        return Some(layer);
                    }
                }
            } else if kind == 5 || kind == 7 {
                for layer in layers_map.values() {
                    if layer.side == Side::Bottom {
                        return Some(layer);
                    }
                }
            }
            None
        };

        let mut claimed_pads: HashMap<usize, std::collections::HashSet<usize>> = HashMap::new();

        for comp in &components {
            let part_id = board.parts.len();
            if !comp.value.is_empty() {
                board.metadata.set_part_value(&comp.refdes, &comp.value);
            }

            let mut part_sides = std::collections::HashSet::new();
            let mut pin_indices = Vec::new();

            for group in &comp.pin_groups {
                let layer = match resolve_layer(group.kind) {
                    Some(l) => l,
                    None => continue,
                };
                part_sides.insert(layer.side);
                let layer_claimed = claimed_pads.entry(group.kind as usize).or_default();

                for p_ref in &group.pins {
                    let pad_idx = (p_ref.uid / 8) as usize;
                    if let Some(pad) = layer.pads.get(pad_idx) {
                        layer_claimed.insert(pad_idx);
                        let pin_id = board.pins.len();
                        let net_name = if pad.net >= 0 && (pad.net as usize) < net_names.len() {
                            net_names[pad.net as usize].clone()
                        } else {
                            String::new()
                        };

                        let w_mils = pad.width.abs() as f64 / 100.0;
                        let h_mils = pad.height.abs() as f64 / 100.0;
                        let radius = (w_mils.max(h_mils) / 2.0 * PAD_RENDER_SHRINK).max(DEFAULT_PAD_RADIUS_MILS);

                        let is_round = pad.shape_type == 0 && (w_mils - h_mils).abs() < 1e-3;
                        let shape = if is_round {
                            crate::model::PadShape::Round
                        } else if (w_mils - h_mils).abs() < 1e-3 {
                            crate::model::PadShape::Square
                        } else {
                            crate::model::PadShape::Rect
                        };

                        // Check if pad coordinates are footprint-local (e.g. edge connectors like CN1)
                        // where raw pad coordinates are relative to placement center cy/cx.
                        let (px, py) = {
                            let mut x = pad.x as f64 / 100.0;
                            let mut y = pad.y as f64 / 100.0;
                            let min_y = comp.min_y as f64 / 100.0;
                            let max_y = comp.max_y as f64 / 100.0;
                            let cy = comp.cy as f64 / 100.0;
                            if comp.cy != 0 && (y < min_y - 50.0 || y > max_y + 50.0) {
                                let trans_y = y + cy;
                                if trans_y >= min_y - 50.0 && trans_y <= max_y + 50.0 {
                                    y = trans_y;
                                }
                            }
                            let min_x = comp.min_x as f64 / 100.0;
                            let max_x = comp.max_x as f64 / 100.0;
                            let cx = comp.cx as f64 / 100.0;
                            if comp.cx != 0 && (x < min_x - 50.0 || x > max_x + 50.0) {
                                let trans_x = x + cx;
                                if trans_x >= min_x - 50.0 && trans_x <= max_x + 50.0 {
                                    x = trans_x;
                                }
                            }
                            (x, y)
                        };

                        board.pins.push(Pin {
                            id: pin_id,
                            part_id,
                            name: if p_ref.name.is_empty() { p_ref.ordinal.to_string() } else { p_ref.name.clone() },
                            net_name,
                            pos: Point::new(px, py),
                            side: layer.side,
                            radius,
                            shape,
                            width: if w_mils > 0.0 { Some(w_mils) } else { None },
                            height: if h_mils > 0.0 { Some(h_mils) } else { None },
                            rotation_deg: 0.0,
                        });
                        pin_indices.push(pin_id);
                    }
                }
            }

            let part_side = if part_sides.len() == 1 {
                part_sides.into_iter().next().unwrap()
            } else if part_sides.len() > 1 {
                Side::Both
            } else {
                Side::Top
            };

            let bx1 = comp.min_x as f64 / 100.0;
            let by1 = comp.min_y as f64 / 100.0;
            let bx2 = comp.max_x as f64 / 100.0;
            let by2 = comp.max_y as f64 / 100.0;

            let mut min_x = bx1.min(bx2);
            let mut min_y = by1.min(by2);
            let mut max_x = bx1.max(bx2);
            let mut max_y = by1.max(by2);

            for &pi in &pin_indices {
                let p = &board.pins[pi];
                let pw = p.width.unwrap_or(p.radius * 2.0);
                let ph = p.height.unwrap_or(p.radius * 2.0);
                min_x = min_x.min(p.pos.x - pw / 2.0);
                max_x = max_x.max(p.pos.x + pw / 2.0);
                min_y = min_y.min(p.pos.y - ph / 2.0);
                max_y = max_y.max(p.pos.y + ph / 2.0);
            }

            let outline = if min_x < max_x && min_y < max_y {
                vec![
                    Point::new(min_x, min_y),
                    Point::new(max_x, min_y),
                    Point::new(max_x, max_y),
                    Point::new(min_x, max_y),
                    Point::new(min_x, min_y),
                ]
            } else {
                Vec::new()
            };

            board.parts.push(Part {
                id: part_id,
                name: comp.refdes.clone(),
                mfg_code: comp.package.clone(),
                side: part_side,
                part_type: PartType::Smd,
                p1: Point::new(bx1, by1),
                p2: Point::new(bx2, by2),
                pin_indices,
                bounding_box: Some(Rect {
                    min_x,
                    min_y,
                    max_x,
                    max_y,
                }),
                outline,
                ..Default::default()
            });
        }

        // Add unclaimed physical pads as vias / test pads
        for (layer_idx, layer) in &layers_map {
            let empty_claimed = std::collections::HashSet::new();
            let claimed = claimed_pads.get(layer_idx).unwrap_or(&empty_claimed);
            for (p_idx, pad) in layer.pads.iter().enumerate() {
                if claimed.contains(&p_idx) {
                    continue;
                }
                let net_name = if pad.net >= 0 && (pad.net as usize) < net_names.len() {
                    net_names[pad.net as usize].clone()
                } else {
                    String::new()
                };

                let w_mils = pad.width.abs() as f64 / 100.0;
                let h_mils = pad.height.abs() as f64 / 100.0;
                let radius = (w_mils.max(h_mils) / 2.0 * PAD_RENDER_SHRINK).max(DEFAULT_PAD_RADIUS_MILS);

                let via_id = board.vias.len();
                board.vias.push(Via {
                    id: via_id,
                    pos: Point::new(pad.x as f64 / 100.0, pad.y as f64 / 100.0),
                    side: layer.side,
                    net_name,
                    radius,
                });
            }
        }

        board.rebuild_indices();

        // Synthesize board outline from bounding box with 20-mil margin
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
