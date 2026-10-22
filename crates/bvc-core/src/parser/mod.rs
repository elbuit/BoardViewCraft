//! Format parsing interface and registry.

pub mod cad;
pub mod gencad;
pub mod registry;

use crate::board::Board;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ParseError {
    #[error("I/O error: {0}")]
    Io(String),
    #[error("Invalid or unrecognized format: {0}")]
    UnrecognizedFormat(String),
    #[error("Parse syntax error at line {line}: {message}")]
    SyntaxError { line: usize, message: String },
    #[error("Unexpected error: {0}")]
    Other(String),
}

/// Trait implemented by boardview file format decoders.
pub trait FormatParser: Send + Sync {
    /// Format name, e.g. "CAD", "BRD", "FZ".
    fn name(&self) -> &'static str;

    /// File extensions supported (without dot), e.g. ["cad"].
    fn extensions(&self) -> &'static [&'static str];

    /// Quick format signature check on raw bytes.
    fn verify_format(&self, data: &[u8]) -> bool;

    /// Parses the file bytes into a `Board`.
    fn parse(&self, data: &[u8]) -> Result<Board, ParseError>;
}
