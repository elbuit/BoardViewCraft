//! Extensible parser registry and two-stage format autodetection.

use std::sync::Arc;
use crate::board::Board;
use crate::parser::allegro::AllegroParser;
use crate::parser::altium::AltiumParser;
use crate::parser::asc::AscParser;
use crate::parser::bdv::BdvParser;
use crate::parser::brd::BrdParser;
use crate::parser::brd2::Brd2Parser;
use crate::parser::bv2::{Bv2Parser, PinListParser};
use crate::parser::cad::CadParser;
use crate::parser::cpd::CpdParser;
use crate::parser::dialects::{BvParser, BvrParser, CstParser, F2bParser, GrParser};
use crate::parser::eagle::EagleParser;
use crate::parser::easyeda::{EasyEdaProParser, EasyEdaStdParser};
use crate::parser::extended_formats::{MentorNeutralParser, SamsungCadParser, TeboIctParser};
use crate::parser::fz::FzParser;
use crate::parser::fz_zlib::FzZlibParser;
use crate::parser::gencad::GenCadParser;
use crate::parser::ipc2581::Ipc2581Parser;
use crate::parser::ipc356::Ipc356Parser;
use crate::parser::kicad::KicadParser;
use crate::parser::odbpp::OdbppParser;
use crate::parser::rejection::{BrdV1Rejector, GerberRejector, PadsBinaryRejector, Vs2Rejector};
use crate::parser::tvw::TvwParser;
use crate::parser::xzz::XzzParser;
use crate::parser::zip_container::ZipArchiveReader;
use crate::parser::{FormatParser, ParseError};

pub struct ParserRegistry {
    parsers: Vec<Arc<dyn FormatParser>>,
}

impl Default for ParserRegistry {
    fn default() -> Self {
        let mut reg = Self::new();
        // Register parsers. Priority order for sniffing and extension matching:
        // 1. High-specificity formats (GenCAD, FZ-zlib, XZZ, TVW binary, CPD, BRD2, Subst-BRD)
        // Diagnostic rejection filters (intercept formats before generic fallbacks)
        reg.register(Arc::new(GerberRejector::new()));
        reg.register(Arc::new(PadsBinaryRejector::new()));
        reg.register(Arc::new(Vs2Rejector::new()));
        reg.register(Arc::new(BrdV1Rejector::new()));

        reg.register(Arc::new(GenCadParser::new()));
        reg.register(Arc::new(FzZlibParser::new()));
        reg.register(Arc::new(FzParser::new()));
        reg.register(Arc::new(XzzParser::new()));
        reg.register(Arc::new(KicadParser::new()));
        reg.register(Arc::new(Ipc2581Parser::new()));
        reg.register(Arc::new(Ipc356Parser::new()));
        reg.register(Arc::new(OdbppParser::new()));
        reg.register(Arc::new(TvwParser::new()));
        reg.register(Arc::new(BdvParser::new()));
        reg.register(Arc::new(CpdParser::new()));
        reg.register(Arc::new(Brd2Parser::new()));
        reg.register(Arc::new(EagleParser::new()));
        reg.register(Arc::new(AllegroParser::new()));
        reg.register(Arc::new(AltiumParser::new()));
        reg.register(Arc::new(BrdParser::new()));
        reg.register(Arc::new(SamsungCadParser::new()));
        reg.register(Arc::new(CadParser::new()));
        reg.register(Arc::new(MentorNeutralParser::new()));
        reg.register(Arc::new(TeboIctParser::new()));
        reg.register(Arc::new(Bv2Parser::new()));
        reg.register(Arc::new(PinListParser::new()));
        reg.register(Arc::new(EasyEdaStdParser::new()));
        reg.register(Arc::new(EasyEdaProParser::new()));
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
        // 0. Transparent ZIP archive container inspection
        if ZipArchiveReader::is_zip(data) {
            if let Ok(mut archive) = ZipArchiveReader::open(data) {
                // Collect candidates that match recognized board extensions
                let mut candidate_indices = Vec::new();
                for (idx, entry) in archive.entries.iter().enumerate() {
                    let entry_ext = std::path::Path::new(&entry.name)
                        .extension()
                        .and_then(|s| s.to_str())
                        .unwrap_or("")
                        .to_ascii_lowercase();

                    for parser in &self.parsers {
                        if parser.extensions().iter().any(|&e| e.eq_ignore_ascii_case(&entry_ext)) {
                            candidate_indices.push(idx);
                            break;
                        }
                    }
                }

                // If candidate board files found in ZIP, extract and parse
                for &idx in &candidate_indices {
                    let entry = archive.entries[idx].clone();
                    if let Ok(extracted) = archive.extract(&entry) {
                        if let Ok(board) = self.parse_file(&entry.name, &extracted) {
                            return Ok(board);
                        }
                    }
                }
            }
        }

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
                // Diagnostic rejectors return an explicit Err when verify_format matches
                return parser.parse(data);
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
