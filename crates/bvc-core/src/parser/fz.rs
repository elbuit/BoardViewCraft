//! Foxconn `.fz` format parser.
//!
//! Handles:
//! 1. FZ-zlib container (decompresses and parses pipe-delimited payload)
//! 2. Foxconn XOR / RC6-like 16-byte sliding window cipher with 44x32-bit key schedule

use std::env;
use crate::board::Board;
use crate::parser::fz_zlib::FzZlibParser;
use crate::parser::{FormatParser, ParseError};

pub const FZ_KEY_ENV: &str = "WRENCH_BOARD_FZ_KEY";
const WINDOW: usize = 16;
const ROUNDS: usize = 20;

pub struct FzParser;

impl FzParser {
    pub const fn new() -> Self {
        Self
    }

    #[inline(always)]
    fn rol32(v: u32, s: u32) -> u32 {
        let s = s & 31;
        if s == 0 {
            v
        } else {
            (v << s) | (v >> (32 - s))
        }
    }

    /// Decrypts Foxconn sliding window XOR cipher using 44 uint32 words key
    pub fn decrypt_fz_xor(cipher: &[u8], k: &[u32; 44]) -> Vec<u8> {
        let mut window = [0u8; WINDOW];
        let (mut n5, mut n4, mut n3, mut n2): (u32, u32, u32, u32) = (0, 0, 0, 0);
        let mut out = vec![0u8; cipher.len()];

        for (i, &b) in cipher.iter().enumerate() {
            n4 = n4.wrapping_add(k[0]);
            n2 = n2.wrapping_add(k[1]);
            for r in 1..=ROUNDS {
                let t4 = n4.wrapping_mul((n4 << 1).wrapping_add(1));
                let mix4 = Self::rol32(t4, 5);
                let t2 = n2.wrapping_mul((n2 << 1).wrapping_add(1));
                let mix2 = Self::rol32(t2, 5);
                let new_n5 = Self::rol32(n5 ^ mix4, mix2 & 0xff).wrapping_add(k[r * 2]);
                let new_n3 = Self::rol32(n3 ^ mix2, mix4 & 0xff).wrapping_add(k[r * 2 + 1]);
                let saved_n5 = new_n5;
                n5 = n4;
                n4 = new_n3;
                n3 = n2;
                n2 = saved_n5;
            }
            n5 = n5.wrapping_add(k[42]);
            out[i] = b ^ (n5 & 0xff) as u8;

            window.copy_within(1..WINDOW, 0);
            window[WINDOW - 1] = b;
            n5 = u32::from_le_bytes([window[0], window[1], window[2], window[3]]);
            n4 = u32::from_le_bytes([window[4], window[5], window[6], window[7]]);
            n3 = u32::from_le_bytes([window[8], window[9], window[10], window[11]]);
            n2 = u32::from_le_bytes([window[12], window[13], window[14], window[15]]);
        }
        out
    }

    pub fn load_key() -> Option<[u32; 44]> {
        if let Ok(raw) = env::var(FZ_KEY_ENV) {
            let s = raw.trim();
            if s.contains(' ') || s.contains(',') {
                let tokens: Vec<&str> = s.split(|c| c == ' ' || c == ',').filter(|t| !t.is_empty()).collect();
                if tokens.len() == 44 {
                    let mut words = [0u32; 44];
                    for (i, tok) in tokens.iter().enumerate() {
                        if let Ok(val) = tok.parse::<u32>() {
                            words[i] = val;
                        } else if let Ok(val) = u32::from_str_radix(tok.trim_start_matches("0x"), 16) {
                            words[i] = val;
                        } else {
                            return None;
                        }
                    }
                    return Some(words);
                }
            } else if s.len() == 176 * 2 {
                // Hex-encoded 176 bytes
                if let Ok(bytes) = (0..s.len())
                    .step_by(2)
                    .map(|i| u8::from_str_radix(&s[i..i + 2], 16))
                    .collect::<Result<Vec<u8>, _>>()
                {
                    if bytes.len() == 176 {
                        let mut words = [0u32; 44];
                        for i in 0..44 {
                            words[i] = u32::from_le_bytes([
                                bytes[i * 4],
                                bytes[i * 4 + 1],
                                bytes[i * 4 + 2],
                                bytes[i * 4 + 3],
                            ]);
                        }
                        return Some(words);
                    }
                }
            }
        }
        None
    }
}

impl Default for FzParser {
    fn default() -> Self {
        Self::new()
    }
}

impl FormatParser for FzParser {
    fn name(&self) -> &'static str {
        "FZ"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["fz"]
    }

    fn verify_format(&self, data: &[u8]) -> bool {
        if FzZlibParser::looks_like_fz_zlib(data) {
            return true;
        }
        // XOR scrambled FZ lacks zlib magic at offset 4 but is > 8 bytes
        data.len() >= 8
    }

    fn parse(&self, data: &[u8]) -> Result<Board, ParseError> {
        // If FZ-zlib, direct parse
        if FzZlibParser::looks_like_fz_zlib(data) {
            return FzZlibParser::new().parse(data);
        }

        // Try XOR decryption
        if let Some(key) = Self::load_key() {
            let decrypted = Self::decrypt_fz_xor(data, &key);
            if FzZlibParser::looks_like_fz_zlib(&decrypted) {
                return FzZlibParser::new().parse(&decrypted);
            }
        }

        Err(ParseError::Other(
            "FZ file appears XOR encrypted, but no valid key is configured in WRENCH_BOARD_FZ_KEY".into(),
        ))
    }
}
