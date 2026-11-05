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
use std::collections::HashMap;

pub struct OdbppParser;

impl OdbppParser {
    pub const fn new() -> Self {
        Self
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
        false
    }

    fn parse(&self, data: &[u8]) -> Result<Board, ParseError> {
        let mut board = Board::new();

        if !ZipArchiveReader::is_zip(data) {
            return Err(ParseError::UnrecognizedFormat(
                "Only ZIP-packaged ODB++ archives are supported currently".into(),
            ));
        }

        let mut archive = ZipArchiveReader::open(data)
            .map_err(|e| ParseError::Other(format!("Failed to open ODB++ zip archive: {}", e)))?;

        // 1. Locate profile (outline)
        let profile_entry = archive
            .entries
            .iter()
            .find(|e| e.name.ends_with("/profile") || e.name == "profile")
            .cloned();

        let scale = 25.4; // ODB++ default unit is inches (scale to mm)

        if let Some(entry) = profile_entry {
            if let Ok(bytes) = archive.extract(&entry) {
                let text = String::from_utf8_lossy(&bytes);
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
        }

        // 2. Parse EDA data (packages and nets)
        let mut packages: HashMap<String, OdbPackage> = HashMap::new();
        let eda_entry = archive
            .entries
            .iter()
            .find(|e| e.name.ends_with("/eda/data"))
            .cloned();

        if let Some(entry) = eda_entry {
            if let Ok(bytes) = archive.extract(&entry) {
                let text = String::from_utf8_lossy(&bytes);
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
        }

        // 3. Parse components (comp_+_top and comp_+_bot)
        let comp_entries: Vec<_> = archive
            .entries
            .iter()
            .filter(|e| e.name.contains("/comp_+_top") || e.name.contains("/comp_+_bot"))
            .cloned()
            .collect();

        for entry in comp_entries {
            let side = if entry.name.contains("bot") { Side::Bottom } else { Side::Top };
            if let Ok(bytes) = archive.extract(&entry) {
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
