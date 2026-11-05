//! Safe in-memory ZIP archive reader for board and companion imports.
//!
//! Follows PKWARE APPNOTE.TXT specifications:
//! - End of central directory record locator
//! - Central directory headers
//! - Safe extraction limits (anti-decompression-bomb)
//! - CRC-32 verification and size matching
//! - Pure Rust implementation using `flate2` and `crc32fast`

use std::collections::HashMap;
use std::io::Read;
use flate2::read::DeflateDecoder;
use thiserror::Error;

pub const ZIP_MAX_ENTRIES: usize = 4096;
pub const ZIP_MAX_ENTRY_BYTES: usize = 64 * 1024 * 1024;
pub const ZIP_MAX_TOTAL_BYTES: usize = 128 * 1024 * 1024;

#[derive(Error, Debug)]
pub enum ZipError {
    #[error("Not a valid ZIP archive: {0}")]
    InvalidArchive(&'static str),
    #[error("ZIP format limit exceeded: {0}")]
    LimitExceeded(String),
    #[error("Corrupted ZIP entry '{0}': {1}")]
    CorruptEntry(String, String),
    #[error("Unsupported ZIP feature in '{0}': {1}")]
    UnsupportedFeature(String, String),
}

#[derive(Debug, Clone)]
pub struct ZipEntryMeta {
    pub name: String,
    pub method: u16,
    pub compressed_size: usize,
    pub uncompressed_size: usize,
    pub crc32: u32,
    pub local_header_offset: usize,
}

pub struct ZipArchiveReader<'a> {
    data: &'a [u8],
    pub entries: Vec<ZipEntryMeta>,
    pub entries_by_name: HashMap<String, usize>,
    extracted_total_bytes: usize,
}

impl<'a> ZipArchiveReader<'a> {
    pub fn is_zip(data: &[u8]) -> bool {
        data.len() >= 4 && &data[0..4] == b"PK\x03\x04"
    }

    pub fn open(data: &'a [u8]) -> Result<Self, ZipError> {
        if data.len() < 22 {
            return Err(ZipError::InvalidArchive("File too small to be a ZIP archive"));
        }

        // 1. Locate End of Central Directory (EOCD) from the end of the file
        let mut eocd_offset = None;
        let search_start = data.len().saturating_sub(22 + 65535);
        for i in (search_start..=data.len() - 22).rev() {
            if &data[i..i + 4] == b"PK\x05\x06" {
                let comment_len = u16::from_le_bytes([data[i + 20], data[i + 21]]) as usize;
                if i + 22 + comment_len <= data.len() {
                    eocd_offset = Some(i);
                    break;
                }
            }
        }

        let eocd = match eocd_offset {
            Some(idx) => idx,
            None => return Err(ZipError::InvalidArchive("End of Central Directory not found")),
        };

        let total_entries = u16::from_le_bytes([data[eocd + 10], data[eocd + 11]]) as usize;
        let cd_size = u32::from_le_bytes([data[eocd + 12], data[eocd + 13], data[eocd + 14], data[eocd + 15]]) as usize;
        let cd_offset = u32::from_le_bytes([data[eocd + 16], data[eocd + 17], data[eocd + 18], data[eocd + 19]]) as usize;

        if total_entries > ZIP_MAX_ENTRIES {
            return Err(ZipError::LimitExceeded(format!(
                "Archive contains {} entries; limit is {}",
                total_entries, ZIP_MAX_ENTRIES
            )));
        }

        if cd_offset + cd_size > data.len() {
            return Err(ZipError::InvalidArchive("Central directory out of bounds"));
        }

        let mut entries = Vec::with_capacity(total_entries);
        let mut entries_by_name = HashMap::with_capacity(total_entries);
        let mut cur = cd_offset;

        for _ in 0..total_entries {
            if cur + 46 > cd_offset + cd_size {
                break;
            }
            if &data[cur..cur + 4] != b"PK\x01\x02" {
                return Err(ZipError::InvalidArchive("Malformed central directory record"));
            }

            let flags = u16::from_le_bytes([data[cur + 8], data[cur + 9]]);
            let method = u16::from_le_bytes([data[cur + 10], data[cur + 11]]);
            let crc = u32::from_le_bytes([data[cur + 16], data[cur + 17], data[cur + 18], data[cur + 19]]);
            let comp_size = u32::from_le_bytes([data[cur + 20], data[cur + 21], data[cur + 22], data[cur + 23]]) as usize;
            let uncomp_size = u32::from_le_bytes([data[cur + 24], data[cur + 25], data[cur + 26], data[cur + 27]]) as usize;
            let name_len = u16::from_le_bytes([data[cur + 28], data[cur + 29]]) as usize;
            let extra_len = u16::from_le_bytes([data[cur + 30], data[cur + 31]]) as usize;
            let comment_len = u16::from_le_bytes([data[cur + 32], data[cur + 33]]) as usize;
            let local_offset = u32::from_le_bytes([data[cur + 42], data[cur + 43], data[cur + 44], data[cur + 45]]) as usize;

            if cur + 46 + name_len > data.len() {
                return Err(ZipError::InvalidArchive("Entry filename out of bounds"));
            }

            let name_bytes = &data[cur + 46..cur + 46 + name_len];
            let name = String::from_utf8_lossy(name_bytes).replace('\\', "/");
            cur += 46 + name_len + extra_len + comment_len;

            // Skip directories and metadata
            if name.ends_with('/') || name.starts_with("__MACOSX/") || name.starts_with("._") {
                continue;
            }

            if (flags & 1) != 0 {
                return Err(ZipError::UnsupportedFeature(name, "Encrypted entries not supported".into()));
            }

            if method != 0 && method != 8 {
                return Err(ZipError::UnsupportedFeature(
                    name,
                    format!("Unsupported compression method {}", method),
                ));
            }

            let meta = ZipEntryMeta {
                name: name.clone(),
                method,
                compressed_size: comp_size,
                uncompressed_size: uncomp_size,
                crc32: crc,
                local_header_offset: local_offset,
            };

            let idx = entries.len();
            entries_by_name.insert(name, idx);
            entries.push(meta);
        }

        Ok(Self {
            data,
            entries,
            entries_by_name,
            extracted_total_bytes: 0,
        })
    }

    pub fn extract(&mut self, entry: &ZipEntryMeta) -> Result<Vec<u8>, ZipError> {
        if entry.uncompressed_size > ZIP_MAX_ENTRY_BYTES {
            return Err(ZipError::LimitExceeded(format!(
                "Entry '{}' size {} exceeds max single entry limit {}",
                entry.name, entry.uncompressed_size, ZIP_MAX_ENTRY_BYTES
            )));
        }

        if self.extracted_total_bytes + entry.uncompressed_size > ZIP_MAX_TOTAL_BYTES {
            return Err(ZipError::LimitExceeded(
                "Total archive extraction budget exceeded".into(),
            ));
        }

        let loc = entry.local_header_offset;
        if loc + 30 > self.data.len() || &self.data[loc..loc + 4] != b"PK\x03\x04" {
            return Err(ZipError::CorruptEntry(
                entry.name.clone(),
                "Local header missing or invalid".into(),
            ));
        }

        let name_len = u16::from_le_bytes([self.data[loc + 26], self.data[loc + 27]]) as usize;
        let extra_len = u16::from_le_bytes([self.data[loc + 28], self.data[loc + 29]]) as usize;
        let data_start = loc + 30 + name_len + extra_len;
        let data_end = data_start + entry.compressed_size;

        if data_end > self.data.len() {
            return Err(ZipError::CorruptEntry(
                entry.name.clone(),
                "Entry data extends past file buffer".into(),
            ));
        }

        let comp_bytes = &self.data[data_start..data_end];
        let decompressed = if entry.method == 0 {
            comp_bytes.to_vec()
        } else {
            let mut decoder = DeflateDecoder::new(comp_bytes);
            let mut out = Vec::with_capacity(entry.uncompressed_size);
            decoder.read_to_end(&mut out).map_err(|e| {
                ZipError::CorruptEntry(entry.name.clone(), format!("Deflate decoding error: {}", e))
            })?;
            out
        };

        if decompressed.len() != entry.uncompressed_size {
            return Err(ZipError::CorruptEntry(
                entry.name.clone(),
                format!(
                    "Decompressed size mismatch: got {}, declared {}",
                    decompressed.len(),
                    entry.uncompressed_size
                ),
            ));
        }

        let mut hasher = crc32fast::Hasher::new();
        hasher.update(&decompressed);
        if hasher.finalize() != entry.crc32 {
            return Err(ZipError::CorruptEntry(
                entry.name.clone(),
                "CRC-32 checksum verification failed".into(),
            ));
        }

        self.extracted_total_bytes += decompressed.len();
        Ok(decompressed)
    }
}
