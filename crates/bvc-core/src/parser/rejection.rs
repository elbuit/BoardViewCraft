//! Diagnostic rejectors for recognized unsupported formats.
//!
//! Provides informative rejection explanations when the user attempts to load
//! files that carry specific known signatures but are not complete electrical boardviews:
//! - Gerber RS-274X (`.gbr`): single-layer graphical fabrication data without electrical components/nets.
//! - PADS native SDB (`.pcb`): binary database versions `0x2026` / `0x2027` requiring ASCII export.
//! - VS2 listing (`.lst`): CASTw assembly listing without complete pin/net layer stack.
//! - BRD_V1.0 (`.brd`): proprietary encoded boardview without public cipher specification.

use crate::board::Board;
use crate::parser::{FormatParser, ParseError};

// ============================================================================
// Gerber RS-274X Rejector
// ============================================================================

pub struct GerberRejector;

impl GerberRejector {
    pub const fn new() -> Self {
        Self
    }

    pub fn looks_like_gerber(data: &[u8]) -> bool {
        let sample = if data.len() > 2048 { &data[..2048] } else { data };
        let text = String::from_utf8_lossy(sample);
        text.contains("%FS") || (text.contains("%MO") && (text.contains("%ADD") || text.contains("D02*")))
    }
}

impl Default for GerberRejector {
    fn default() -> Self {
        Self::new()
    }
}

impl FormatParser for GerberRejector {
    fn name(&self) -> &'static str {
        "Gerber-RS274X"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["gbr", "ger", "pho"]
    }

    fn verify_format(&self, data: &[u8]) -> bool {
        Self::looks_like_gerber(data)
    }

    fn parse(&self, _data: &[u8]) -> Result<Board, ParseError> {
        Err(ParseError::Other(
            "Gerber RS-274X detected: single-layer graphics without component or net hierarchy. A complete CAD or boardview export is required.".into(),
        ))
    }
}

// ============================================================================
// PADS Native SDB Rejector
// ============================================================================

pub struct PadsBinaryRejector;

impl PadsBinaryRejector {
    pub const fn new() -> Self {
        Self
    }

    pub fn looks_like_pads_binary(data: &[u8]) -> bool {
        // PADS native database: magic 0x00 0xFF followed by database version 0x2026 / 0x2027
        data.len() >= 4 && data[0] == 0x00 && data[1] == 0xff && (data[2] == 0x26 || data[2] == 0x27) && data[3] == 0x20
    }
}

impl Default for PadsBinaryRejector {
    fn default() -> Self {
        Self::new()
    }
}

impl FormatParser for PadsBinaryRejector {
    fn name(&self) -> &'static str {
        "PADS-Binary"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["pcb"]
    }

    fn verify_format(&self, data: &[u8]) -> bool {
        Self::looks_like_pads_binary(data)
    }

    fn parse(&self, _data: &[u8]) -> Result<Board, ParseError> {
        Err(ParseError::Other(
            "PADS native SDB database detected: binary container not imported as an electrical board. Export PADS Layout ASCII (.asc) or GenCAD (.cad).".into(),
        ))
    }
}

// ============================================================================
// VS2 Assembly Listing Rejector
// ============================================================================

pub struct Vs2Rejector;

impl Vs2Rejector {
    pub const fn new() -> Self {
        Self
    }

    pub fn looks_like_vs2(data: &[u8]) -> bool {
        let sample = if data.len() > 1024 { &data[..1024] } else { data };
        let text = String::from_utf8_lossy(sample);
        text.starts_with("$VS2") || text.contains("\n$VS2")
    }
}

impl Default for Vs2Rejector {
    fn default() -> Self {
        Self::new()
    }
}

impl FormatParser for Vs2Rejector {
    fn name(&self) -> &'static str {
        "VS2-Listing"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["lst"]
    }

    fn verify_format(&self, data: &[u8]) -> bool {
        Self::looks_like_vs2(data)
    }

    fn parse(&self, _data: &[u8]) -> Result<Board, ParseError> {
        Err(ParseError::Other(
            "CASTw VS2 assembly listing detected: per-design outer plane mapping and pin netlist are absent. A full CAD/CST export is required.".into(),
        ))
    }
}

// ============================================================================
// BRD_V1.0 Encoded Rejector
// ============================================================================

pub struct BrdV1Rejector;

impl BrdV1Rejector {
    pub const fn new() -> Self {
        Self
    }

    pub fn looks_like_brd_v1(data: &[u8]) -> bool {
        data.len() >= 16 && &data[0..8] == b"BRD_V1.0"
    }
}

impl Default for BrdV1Rejector {
    fn default() -> Self {
        Self::new()
    }
}

impl FormatParser for BrdV1Rejector {
    fn name(&self) -> &'static str {
        "BRD_V1.0"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["brd"]
    }

    fn verify_format(&self, data: &[u8]) -> bool {
        Self::looks_like_brd_v1(data)
    }

    fn parse(&self, _data: &[u8]) -> Result<Board, ParseError> {
        Err(ParseError::Other(
            "BRD_V1.0 encoded boardview detected: proprietary container without a public decoding specification. Request a readable GenCAD export.".into(),
        ))
    }
}
