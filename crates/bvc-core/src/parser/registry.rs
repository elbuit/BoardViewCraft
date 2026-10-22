//! Extensible parser registry and format autodetection.

use std::sync::Arc;
use crate::board::Board;
use crate::parser::cad::CadParser;
use crate::parser::gencad::GenCadParser;
use crate::parser::{FormatParser, ParseError};

pub struct ParserRegistry {
    parsers: Vec<Arc<dyn FormatParser>>,
}

impl Default for ParserRegistry {
    fn default() -> Self {
        let mut reg = Self::new();
        // Register GenCAD first so that GenCAD format (.cad) is preferred over legacy CAD
        reg.register(Arc::new(GenCadParser::new()));
        reg.register(Arc::new(CadParser::new()));
        reg
    }
}

impl ParserRegistry {
    pub fn new() -> Self {
        Self { parsers: Vec::new() }
    }

    pub fn register(&mut self, parser: Arc<dyn FormatParser>) {
        self.parsers.push(parser);
    }

    /// Autodetects format and parses board data.
    pub fn parse_file(&self, filename: &str, data: &[u8]) -> Result<Board, ParseError> {
        let ext = std::path::Path::new(filename)
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();

        // 1. Try parsers matching the file extension first
        for p in &self.parsers {
            if p.extensions().iter().any(|e| e.eq_ignore_ascii_case(&ext)) {
                if p.verify_format(data) {
                    return p.parse(data);
                }
            }
        }

        // 2. Try all registered parsers via verify_format
        for p in &self.parsers {
            if p.verify_format(data) {
                return p.parse(data);
            }
        }

        // 3. Fallback: if extension matched, try parsing anyway
        for p in &self.parsers {
            if p.extensions().iter().any(|e| e.eq_ignore_ascii_case(&ext)) {
                return p.parse(data);
            }
        }

        Err(ParseError::UnrecognizedFormat(format!(
            "Could not detect parser for file: {filename}"
        )))
    }
}
