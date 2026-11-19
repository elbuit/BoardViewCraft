//! ODB++ product model archive parser (`.tgz`, `.tar.gz`, `.zip`).
//!
//! Follows ODB++ Design Format Specification:
//! - Steps & layers inspection from `matrix/matrix`.
//! - Components from `steps/<step>/layers/comp_+_top/components` and `comp_+_bot`.
//! - EDA data from `steps/<step>/eda/data` (packages, pins, and nets).
//! - Profile outline from `steps/<step>/profile`.
//! - Supports `.zip` and `.tar.gz` containers.

use crate::board::Board;
use crate::model::{PadShape, Part, PartType, Pin, Point, Side};
use crate::parser::zip_container::ZipArchiveReader;
use crate::parser::{FormatParser, ParseError};
use flate2::read::GzDecoder;
use std::collections::HashMap;
use std::io::Read;

pub struct OdbppParser;

impl OdbppParser {
    pub const fn new() -> Self {
        Self
    }

    /// Check if buffer starts with Gzip magic bytes 0x1f, 0x8b
    pub fn is_gzip(data: &[u8]) -> bool {
        data.len() >= 2 && data[0] == 0x1f && data[1] == 0x8b
    }

    /// Attempts to decompress gzip data in memory
    pub fn decompress_gzip(data: &[u8]) -> Result<Vec<u8>, ParseError> {
        let decoder = GzDecoder::new(data);
        let mut buf = Vec::new();
        // Limit max decompressed size to 128MB to protect memory
        decoder
            .take(128 * 1024 * 1024)
            .read_to_end(&mut buf)
            .map_err(|e| ParseError::Other(format!("Failed to decompress gzip archive: {}", e)))?;
        Ok(buf)
    }

    /// Extracts tar entries into a map of filename -> content
    pub fn extract_tar(data: &[u8]) -> Result<HashMap<String, Vec<u8>>, ParseError> {
        let mut entries = HashMap::new();
        let mut offset = 0;
        let mut total_bytes = 0usize;
        const MAX_TOTAL_BYTES: usize = 128 * 1024 * 1024;
        const MAX_FILE_SIZE: usize = 64 * 1024 * 1024;

        while offset + 512 <= data.len() {
            let header = &data[offset..offset + 512];
            // Two consecutive zero blocks indicate end of archive; a single zero block is also end
            if header.iter().all(|&b| b == 0) {
                break;
            }

            // Extract file name (up to 100 bytes)
            let raw_name = &header[0..100];
            let name_len = raw_name.iter().position(|&b| b == 0).unwrap_or(raw_name.len());
            let name_str = String::from_utf8_lossy(&raw_name[..name_len]).trim().to_string();

            // Check prefix (bytes 345..500) if ustar
            let prefix_raw = &header[345..500];
            let prefix_len = prefix_raw.iter().position(|&b| b == 0).unwrap_or(prefix_raw.len());
            let prefix_str = String::from_utf8_lossy(&prefix_raw[..prefix_len]).trim().to_string();

            let full_name = if !prefix_str.is_empty() {
                format!("{}/{}", prefix_str, name_str)
            } else {
                name_str
            };

            // Parse size (12 bytes octal in ascii at offset 124..136)
            let size_field = &header[124..136];
            let size_str = String::from_utf8_lossy(size_field);
            let size = usize::from_str_radix(size_str.trim().trim_matches('\0'), 8).unwrap_or(0);

            // Type flag at offset 156 ('0' or '\0' for normal file, '5' for directory)
            let type_flag = header[156];

            let data_offset = offset + 512;
            let blocks = (size + 511) / 512;
            let next_offset = data_offset + blocks * 512;

            if size > MAX_FILE_SIZE {
                return Err(ParseError::Other(format!(
                    "Tar entry '{}' exceeds size limit of 64MB",
                    full_name
                )));
            }
            total_bytes += size;
            if total_bytes > MAX_TOTAL_BYTES {
                return Err(ParseError::Other(
                    "Total extracted tar bytes exceeded 128MB limit".into(),
                ));
            }

            if (type_flag == b'0' || type_flag == 0) && size > 0 {
                if data_offset + size <= data.len() {
                    let content = data[data_offset..data_offset + size].to_vec();
                    entries.insert(full_name, content);
                }
            }

            offset = next_offset;
        }

        Ok(entries)
    }

    /// Read entries from either a ZIP container or a TAR/TAR.GZ container
    fn read_container(data: &[u8]) -> Result<HashMap<String, Vec<u8>>, ParseError> {
        if ZipArchiveReader::is_zip(data) {
            let mut archive = ZipArchiveReader::open(data)
                .map_err(|e| ParseError::Other(format!("Failed to open ODB++ zip archive: {}", e)))?;
            let mut map = HashMap::new();
            for entry in archive.entries.clone() {
                if let Ok(bytes) = archive.extract(&entry) {
                    map.insert(entry.name, bytes);
                }
            }
            return Ok(map);
        }

        if Self::is_gzip(data) {
            let raw_tar = Self::decompress_gzip(data)?;
            return Self::extract_tar(&raw_tar);
        }

        // Could be uncompressed tar
        Self::extract_tar(data)
    }
}

impl Default for OdbppParser {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Default, Clone)]
struct OdbPin {
    name: String,
    x: f64,
    y: f64,
}

#[derive(Default, Clone)]
struct OdbPackage {
    pins: Vec<OdbPin>,
}

impl FormatParser for OdbppParser {
    fn name(&self) -> &'static str {
        "ODB++"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["tgz", "tar", "gz"]
    }

    fn verify_format(&self, data: &[u8]) -> bool {
        if ZipArchiveReader::is_zip(data) {
            if let Ok(archive) = ZipArchiveReader::open(data) {
                return archive.entries.iter().any(|e| {
                    e.name.contains("matrix/matrix") || e.name.contains("steps/")
                });
            }
        }
        if Self::is_gzip(data) {
            if let Ok(decompressed) = Self::decompress_gzip(data) {
                if let Ok(entries) = Self::extract_tar(&decompressed) {
                    return entries.keys().any(|k| {
                        k.contains("matrix/matrix") || k.contains("steps/")
                    });
                }
            }
        }
        // Direct uncompressed tar
        if data.len() >= 512 && &data[257..262] == b"ustar" {
            if let Ok(entries) = Self::extract_tar(data) {
                return entries.keys().any(|k| {
                    k.contains("matrix/matrix") || k.contains("steps/")
                });
            }
        }
        false
    }

    fn parse(&self, data: &[u8]) -> Result<Board, ParseError> {
        let mut board = Board::new();

        let entries = Self::read_container(data)?;
        if entries.is_empty() {
            return Err(ParseError::UnrecognizedFormat(
                "ODB++ archive is empty or contains no valid files".into(),
            ));
        }

        let scale = 25.4; // ODB++ default unit is inches (scale to mm)

        // 1. Locate profile (outline)
        let profile_entry = entries
            .iter()
            .find(|(k, _)| k.ends_with("/profile") || *k == "profile")
            .map(|(_, v)| v);

        if let Some(bytes) = profile_entry {
            let text = String::from_utf8_lossy(bytes);
            for line in text.lines() {
                let trimmed = line.trim();
                let tokens: Vec<&str> = trimmed.split_whitespace().collect();
                // OB x y ... or OS x y ...
                if (tokens.first() == Some(&"OB") || tokens.first() == Some(&"OS")) && tokens.len() >= 3 {
                    if let (Ok(x), Ok(y)) = (tokens[1].parse::<f64>(), tokens[2].parse::<f64>()) {
                        board.outline_polygon.push(Point::new(x * scale, y * scale));
                    }
                }
            }
        }

        // 2. Parse EDA data (packages and nets)
        let mut packages: HashMap<String, OdbPackage> = HashMap::new();
        let eda_entry = entries
            .iter()
            .find(|(k, _)| k.ends_with("/eda/data"))
            .map(|(_, v)| v);

        if let Some(bytes) = eda_entry {
            let text = String::from_utf8_lossy(bytes);
            let mut current_pkg_name = String::new();
            let mut current_pkg_pins = Vec::new();
            let mut in_pkg = false;

            for line in text.lines() {
                let trimmed = line.trim();
                let tokens: Vec<&str> = trimmed.split_whitespace().collect();
                if tokens.is_empty() {
                    continue;
                }

                if tokens[0] == "PKG" && tokens.len() >= 2 {
                    if in_pkg && !current_pkg_name.is_empty() {
                        packages.insert(
                            current_pkg_name.clone(),
                            OdbPackage {
                                pins: current_pkg_pins.clone(),
                            },
                        );
                    }
                    in_pkg = true;
                    current_pkg_name = tokens[1].to_string();
                    current_pkg_pins.clear();
                } else if in_pkg && tokens[0] == "PIN" && tokens.len() >= 4 {
                    let pin_name = tokens[1].to_string();
                    let px = tokens[2].parse::<f64>().unwrap_or(0.0) * scale;
                    let py = tokens[3].parse::<f64>().unwrap_or(0.0) * scale;
                    current_pkg_pins.push(OdbPin {
                        name: pin_name,
                        x: px,
                        y: py,
                    });
                }
            }

            if in_pkg && !current_pkg_name.is_empty() {
                packages.insert(
                    current_pkg_name,
                    OdbPackage {
                        pins: current_pkg_pins,
                    },
                );
            }
        }

        // 3. Parse components (comp_+_top and comp_+_bot)
        let comp_entries: Vec<_> = entries
            .iter()
            .filter(|(k, _)| k.contains("/comp_+_top") || k.contains("/comp_+_bot"))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();

        for (name, bytes) in comp_entries {
            let side = if name.contains("bot") { Side::Bottom } else { Side::Top };
            let text = String::from_utf8_lossy(&bytes);
            for line in text.lines() {
                let trimmed = line.trim();
                let tokens: Vec<&str> = trimmed.split_whitespace().collect();
                // CMP <comp_num> <pkg_name> <x> <y> <rot> <mirror> <ref_des> ...
                if tokens.first() == Some(&"CMP") && tokens.len() >= 8 {
                    let pkg_name = tokens[2];
                    let x = tokens[3].parse::<f64>().unwrap_or(0.0) * scale;
                    let y = tokens[4].parse::<f64>().unwrap_or(0.0) * scale;
                    let rot_deg = tokens[5].parse::<f64>().unwrap_or(0.0);
                    let ref_des = tokens[7].to_string();

                    let part_id = board.parts.len();
                    board.parts.push(Part {
                        id: part_id,
                        name: ref_des.clone(),
                        mfg_code: pkg_name.to_string(),
                        side,
                        part_type: PartType::Smd,
                        p1: Point::new(x, y),
                        p2: Point::new(x, y),
                        pin_indices: Vec::new(),
                        bounding_box: None,
                        rotation_deg: rot_deg,
                        ..Default::default()
                    });

                    if let Some(pkg) = packages.get(pkg_name) {
                        let rot_rad = rot_deg.to_radians();
                        let cos_a = rot_rad.cos();
                        let sin_a = rot_rad.sin();

                        for pin in &pkg.pins {
                            let px = if side == Side::Bottom { -pin.x } else { pin.x };
                            let py = pin.y;
                            let wx = x + px * cos_a - py * sin_a;
                            let wy = y + px * sin_a + py * cos_a;

                            let pin_id = board.pins.len();
                            board.pins.push(Pin {
                                id: pin_id,
                                part_id,
                                name: pin.name.clone(),
                                pos: Point::new(wx, wy),
                                side,
                                net_name: String::new(),
                                radius: 0.4,
                                shape: PadShape::Rect,
                                width: Some(0.8),
                                height: Some(0.8),
                                rotation_deg: rot_deg,
                            });
                            board.parts[part_id].pin_indices.push(pin_id);
                        }
                    }
                }
            }
        }

        board.rebuild_indices();
        if board.outline_polygon.is_empty() {
            if let Some(bbox) = board.bounding_box {
                const MARGIN: f64 = 2.0;
                board.outline_polygon = vec![
                    Point::new(bbox.min_x - MARGIN, bbox.min_y - MARGIN),
                    Point::new(bbox.max_x + MARGIN, bbox.min_y - MARGIN),
                    Point::new(bbox.max_x + MARGIN, bbox.max_y + MARGIN),
                    Point::new(bbox.min_x - MARGIN, bbox.max_y + MARGIN),
                ];
                board.rebuild_indices();
            }
        }

        Ok(board)
    }
}
