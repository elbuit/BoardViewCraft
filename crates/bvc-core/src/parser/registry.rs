//! Extensible parser registry and two-stage format autodetection.

use std::sync::Arc;
use crate::board::Board;
use crate::parser::asc::AscParser;
use crate::parser::bdv::BdvParser;
use crate::parser::brd::BrdParser;
use crate::parser::brd2::Brd2Parser;
use crate::parser::cad::CadParser;
use crate::parser::cpd::CpdParser;
use crate::parser::dialects::{BvParser, BvrParser, CstParser, F2bParser, GrParser};
use crate::parser::fz::FzParser;
use crate::parser::fz_zlib::FzZlibParser;
use crate::parser::gencad::GenCadParser;
use crate::parser::tvw::TvwParser;
use crate::parser::xzz::XzzParser;
use crate::parser::{FormatParser, ParseError};

pub struct ParserRegistry {
    parsers: Vec<Arc<dyn FormatParser>>,
}

impl Default for ParserRegistry {
    fn default() -> Self {
        let mut reg = Self::new();
        // Register parsers. Priority order for sniffing and extension matching:
        // 1. High-specificity formats (GenCAD, FZ-zlib, XZZ, TVW binary, CPD, BRD2, Subst-BRD)
        reg.register(Arc::new(GenCadParser::new()));
        reg.register(Arc::new(FzZlibParser::new()));
        reg.register(Arc::new(FzParser::new()));
        reg.register(Arc::new(XzzParser::new()));
        reg.register(Arc::new(TvwParser::new()));
        reg.register(Arc::new(BdvParser::new()));
        reg.register(Arc::new(CpdParser::new()));
        reg.register(Arc::new(Brd2Parser::new()));
        reg.register(Arc::new(BrdParser::new()));
        reg.register(Arc::new(CadParser::new()));
        reg.register(Arc::new(AscParser::new()));
        reg.register(Arc::new(BvrParser::new()));
        reg.register(Arc::new(BvParser::new()));
        reg.register(Arc::new(CstParser::new()));
        reg.register(Arc::new(GrParser::new()));
        reg.register(Arc::new(F2bParser::new()));
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

    /// Autodetects format using two-stage sniffing:
    /// Stage 1: Fast content signature sniffing matching extension if possible, or any parser with positive verify_format
    /// Stage 2: Extension fallback
    pub fn parse_file(&self, filename: &str, data: &[u8]) -> Result<Board, ParseError> {
        let ext = std::path::Path::new(filename)
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();

        // 1. Stage 1a: Try parsers matching the file extension whose verify_format succeeds
        for parser in &self.parsers {
            if parser.extensions().iter().any(|&e| e.eq_ignore_ascii_case(&ext))
                && parser.verify_format(data)
            {
                if let Ok(board) = parser.parse(data) {
                    return Ok(board);
                }
            }
        }

        // 2. Stage 1b: Try any parser whose verify_format succeeds regardless of extension
        for parser in &self.parsers {
            if parser.verify_format(data) {
                if let Ok(board) = parser.parse(data) {
                    return Ok(board);
                }
            }
        }

        // 3. Stage 2: Extension-based fallback if format verification was non-conclusive
        for parser in &self.parsers {
            if parser.extensions().iter().any(|&e| e.eq_ignore_ascii_case(&ext)) {
                if let Ok(board) = parser.parse(data) {
                    return Ok(board);
                }
            }
        }

        Err(ParseError::UnrecognizedFormat(format!(
            "Could not detect or parse format for file '{}' (data size: {} bytes)",
            filename,
            data.len()
        )))
    }
}
