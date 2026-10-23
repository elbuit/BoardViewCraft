//! TSICT `.asc` format parser.
//!
//! Handles single combined `.asc` file or split files assembled together.

use crate::board::Board;
use crate::parser::test_link::{TestLinkEngine, ASC_MARKERS};
use crate::parser::{FormatParser, ParseError};

pub struct AscParser;

impl AscParser {
    pub const fn new() -> Self {
        Self
    }

    pub fn looks_like_combined(text: &str) -> bool {
        let markers = &["Format:", "Parts:", "Pins:", "Nails:", "Components:", "Outline:"];
        let hits = markers.iter().filter(|&&m| text.contains(m)).count();
        hits >= 2
    }
}

impl Default for AscParser {
    fn default() -> Self {
        Self::new()
    }
}

impl FormatParser for AscParser {
    fn name(&self) -> &'static str {
        "ASC"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["asc"]
    }

    fn verify_format(&self, data: &[u8]) -> bool {
        let sample = &data[..data.len().min(4096)];
        let text = String::from_utf8_lossy(sample);
        Self::looks_like_combined(&text)
    }

    fn parse(&self, data: &[u8]) -> Result<Board, ParseError> {
        let text = String::from_utf8_lossy(data);
        if !Self::looks_like_combined(&text) {
            return Err(ParseError::SyntaxError {
                line: 0,
                message: "Payload does not contain combined TSICT .asc markers".into(),
            });
        }
        TestLinkEngine::parse_text(&text, &ASC_MARKERS)
    }
}
