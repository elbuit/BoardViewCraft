//! EAGLE XML board parser (`.brd`).
//!
//! Supports Autodesk/CadSoft EAGLE XML board format:
//! - Elements (components), library packages, and SMD/through-hole pads.
//! - Signals (nets) and pad connections.
//! - Board outline and dimension wires (layer 20).
//! - Rotation handling with mirroring (M prefix for bottom layer).

use crate::board::Board;
use crate::model::{PadShape, Part, PartType, Pin, Point, Side};
use crate::parser::{FormatParser, ParseError};
use quick_xml::events::Event;
use quick_xml::Reader;
use std::collections::HashMap;

pub struct EagleParser;

impl EagleParser {
    pub const fn new() -> Self {
        Self
    }
}

impl Default for EagleParser {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Default, Clone)]
struct PackagePad {
    name: String,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    is_smd: bool,
    layer: u8,
    rot_deg: f64,
}

#[derive(Default, Clone)]
struct LibraryPackage {
    pads: Vec<PackagePad>,
}

#[derive(Default, Clone)]
struct EagleElement {
    name: String,
    library: String,
    package: String,
    x: f64,
    y: f64,
    rot_deg: f64,
    mirrored: bool,
}

#[allow(deprecated)]
impl FormatParser for EagleParser {
    fn name(&self) -> &'static str {
        "EAGLE"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["brd"]
    }

    fn verify_format(&self, data: &[u8]) -> bool {
        let sample = if data.len() > 2048 { &data[..2048] } else { data };
        let text = String::from_utf8_lossy(sample);
        text.contains("<eagle") && (text.contains("<board>") || text.contains("<drawing>"))
    }

    fn parse(&self, data: &[u8]) -> Result<Board, ParseError> {
        let mut reader = Reader::from_reader(data);
        reader.config_mut().trim_text(true);

        let mut board = Board::new();
        let mut packages: HashMap<String, LibraryPackage> = HashMap::new();
        let mut elements: Vec<EagleElement> = Vec::new();
        let mut signal_nets: HashMap<(String, String), String> = HashMap::new(); // (element_name, pad_name) -> net_name

        let mut current_library = String::new();
        let mut current_package = String::new();
        let mut current_pkg_pads = Vec::new();
        let mut in_package = false;
        let mut current_signal = String::new();

        let mut buf = Vec::new();

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                    let qname = e.name();
                    let tag = qname.as_ref();
                    match tag {
                        b"library" => {
                            for attr in e.attributes().flatten() {
                                if attr.key.as_ref() == b"name" {
                                    current_library = String::from_utf8_lossy(&attr.value).to_string();
                                }
                            }
                        }
                        b"package" => {
                            in_package = true;
                            current_pkg_pads.clear();
                            for attr in e.attributes().flatten() {
                                if attr.key.as_ref() == b"name" {
                                    current_package = String::from_utf8_lossy(&attr.value).to_string();
                                }
                            }
                        }
                        b"smd" if in_package => {
                            let mut pad = PackagePad { is_smd: true, layer: 1, ..Default::default() };
                            for attr in e.attributes().flatten() {
                                match attr.key.as_ref() {
                                    b"name" => pad.name = String::from_utf8_lossy(&attr.value).to_string(),
                                    b"x" => pad.x = attr.unescape_value().unwrap_or_default().parse().unwrap_or(0.0),
                                    b"y" => pad.y = attr.unescape_value().unwrap_or_default().parse().unwrap_or(0.0),
                                    b"dx" => pad.width = attr.unescape_value().unwrap_or_default().parse().unwrap_or(0.5),
                                    b"dy" => pad.height = attr.unescape_value().unwrap_or_default().parse().unwrap_or(0.5),
                                    b"layer" => pad.layer = attr.unescape_value().unwrap_or_default().parse().unwrap_or(1),
                                    b"rot" => {
                                        let rot_str = String::from_utf8_lossy(&attr.value);
                                        if let Some(deg_str) = rot_str.strip_prefix('R') {
                                            pad.rot_deg = deg_str.parse().unwrap_or(0.0);
                                        }
                                    }
                                    _ => {}
                                }
                            }
                            current_pkg_pads.push(pad);
                        }
                        b"pad" if in_package => {
                            let mut pad = PackagePad { is_smd: false, layer: 11, width: 0.8, height: 0.8, ..Default::default() };
                            for attr in e.attributes().flatten() {
                                match attr.key.as_ref() {
                                    b"name" => pad.name = String::from_utf8_lossy(&attr.value).to_string(),
                                    b"x" => pad.x = attr.unescape_value().unwrap_or_default().parse().unwrap_or(0.0),
                                    b"y" => pad.y = attr.unescape_value().unwrap_or_default().parse().unwrap_or(0.0),
                                    b"diameter" => {
                                        let d = attr.unescape_value().unwrap_or_default().parse().unwrap_or(0.8);
                                        pad.width = d;
                                        pad.height = d;
                                    }
                                    _ => {}
                                }
                            }
                            current_pkg_pads.push(pad);
                        }
                        b"element" => {
                            let mut el = EagleElement::default();
                            for attr in e.attributes().flatten() {
                                match attr.key.as_ref() {
                                    b"name" => el.name = String::from_utf8_lossy(&attr.value).to_string(),
                                    b"library" => el.library = String::from_utf8_lossy(&attr.value).to_string(),
                                    b"package" => el.package = String::from_utf8_lossy(&attr.value).to_string(),
                                    b"x" => el.x = attr.unescape_value().unwrap_or_default().parse().unwrap_or(0.0),
                                    b"y" => el.y = attr.unescape_value().unwrap_or_default().parse().unwrap_or(0.0),
                                    b"rot" => {
                                        let r_str = String::from_utf8_lossy(&attr.value);
                                        if r_str.starts_with("MR") || r_str.starts_with('M') {
                                            el.mirrored = true;
                                        }
                                        if let Some(idx) = r_str.rfind('R') {
                                            el.rot_deg = r_str[idx + 1..].parse().unwrap_or(0.0);
                                        }
                                    }
                                    _ => {}
                                }
                            }
                            elements.push(el);
                        }
                        b"signal" => {
                            for attr in e.attributes().flatten() {
                                if attr.key.as_ref() == b"name" {
                                    current_signal = String::from_utf8_lossy(&attr.value).to_string();
                                }
                            }
                        }
                        b"contactref" => {
                            let mut elem = String::new();
                            let mut pad = String::new();
                            for attr in e.attributes().flatten() {
                                match attr.key.as_ref() {
                                    b"element" => elem = String::from_utf8_lossy(&attr.value).to_string(),
                                    b"pad" => pad = String::from_utf8_lossy(&attr.value).to_string(),
                                    _ => {}
                                }
                            }
                            if !elem.is_empty() && !pad.is_empty() && !current_signal.is_empty() {
                                signal_nets.insert((elem, pad), current_signal.clone());
                            }
                        }
                        b"wire" => {
                            // Layer 20 is dimension/outline
                            let mut layer = 0u8;
                            let mut x1 = 0.0;
                            let mut y1 = 0.0;
                            let mut x2 = 0.0;
                            let mut y2 = 0.0;
                            for attr in e.attributes().flatten() {
                                match attr.key.as_ref() {
                                    b"layer" => layer = attr.unescape_value().unwrap_or_default().parse().unwrap_or(0),
                                    b"x1" => x1 = attr.unescape_value().unwrap_or_default().parse().unwrap_or(0.0),
                                    b"y1" => y1 = attr.unescape_value().unwrap_or_default().parse().unwrap_or(0.0),
                                    b"x2" => x2 = attr.unescape_value().unwrap_or_default().parse().unwrap_or(0.0),
                                    b"y2" => y2 = attr.unescape_value().unwrap_or_default().parse().unwrap_or(0.0),
                                    _ => {}
                                }
                            }
                            if layer == 20 {
                                board.outline_polygon.push(Point::new(x1, y1));
                                board.outline_polygon.push(Point::new(x2, y2));
                            }
                        }
                        _ => {}
                    }
                }
                Ok(Event::End(ref e)) => {
                    let qname = e.name();
                    let tag = qname.as_ref();
                    if tag == b"package" {
                        in_package = false;
                        let key = format!("{}::{}", current_library, current_package);
                        packages.insert(key.clone(), LibraryPackage { pads: current_pkg_pads.clone() });
                        packages.insert(current_package.clone(), LibraryPackage { pads: current_pkg_pads.clone() });
                    } else if tag == b"signal" {
                        current_signal.clear();
                    }
                }
                Ok(Event::Eof) => break,
                Err(err) => return Err(ParseError::Other(format!("XML error: {}", err))),
                _ => {}
            }
            buf.clear();
        }

        // Instanciate parts and pins
        for elem in elements {
            let pkg_key = format!("{}::{}", elem.library, elem.package);
            let pkg = packages.get(&pkg_key).or_else(|| packages.get(&elem.package));

            let part_side = if elem.mirrored { Side::Bottom } else { Side::Top };
            let part_id = board.parts.len();

            board.parts.push(Part {
                id: part_id,
                name: elem.name.clone(),
                mfg_code: elem.package.clone(),
                side: part_side,
                part_type: PartType::Smd,
                p1: Point::new(elem.x, elem.y),
                p2: Point::new(elem.x, elem.y),
                pin_indices: Vec::new(),
                bounding_box: None,
                ..Default::default()
            });

            if let Some(pkg) = pkg {
                let rot_rad = elem.rot_deg.to_radians();
                let cos_a = rot_rad.cos();
                let sin_a = rot_rad.sin();

                for pad in &pkg.pads {
                    // Mirrored elements flip X first
                    let px = if elem.mirrored { -pad.x } else { pad.x };
                    let py = pad.y;

                    let world_x = elem.x + px * cos_a - py * sin_a;
                    let world_y = elem.y + px * sin_a + py * cos_a;

                    let net = signal_nets
                        .get(&(elem.name.clone(), pad.name.clone()))
                        .cloned()
                        .unwrap_or_default();

                    let pin_side = if !pad.is_smd {
                        Side::Both
                    } else if elem.mirrored {
                        Side::Bottom
                    } else {
                        Side::Top
                    };

                    let pin_id = board.pins.len();
                    board.pins.push(Pin {
                        id: pin_id,
                        part_id,
                        name: pad.name.clone(),
                        pos: Point::new(world_x, world_y),
                        side: pin_side,
                        net_name: net,
                        radius: pad.width.max(pad.height) * 0.5,
                        shape: if pad.is_smd { PadShape::Rect } else { PadShape::Round },
                        width: Some(pad.width),
                        height: Some(pad.height),
                        rotation_deg: pad.rot_deg + elem.rot_deg,
                    });
                    board.parts[part_id].pin_indices.push(pin_id);
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
