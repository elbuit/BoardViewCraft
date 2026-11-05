//! IPC-2581 XML board parser (`.xml`, `.cvg`).
//!
//! Parses standard open IPC-2581 (revisions B and C) manufacturing XML:
//! - Steps, Components (`<Component refDes="..." packageRef="...">`),
//! - Packages (`<Package>`, `<Pin number="..." x="..." y="...">`),
//! - Logical nets (`<LogicalNet name="...">`, `<PinRef componentRef="..." pin="..."/>`),
//! - Coordinates normalized to millimeters.

use crate::board::Board;
use crate::model::{PadShape, Part, PartType, Pin, Point, Side};
use crate::parser::{FormatParser, ParseError};
use quick_xml::events::Event;
use quick_xml::Reader;
use std::collections::HashMap;

pub struct Ipc2581Parser;

impl Ipc2581Parser {
    pub const fn new() -> Self {
        Self
    }
}

impl Default for Ipc2581Parser {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Default, Clone)]
struct IpcPkgPin {
    number: String,
    x: f64,
    y: f64,
}

#[derive(Default, Clone)]
struct IpcPackage {
    pins: Vec<IpcPkgPin>,
}

#[derive(Default, Clone)]
struct IpcComponent {
    ref_des: String,
    package_ref: String,
    x: f64,
    y: f64,
    rot_deg: f64,
    side: Side,
}

impl FormatParser for Ipc2581Parser {
    fn name(&self) -> &'static str {
        "IPC-2581"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["xml", "cvg"]
    }

    fn verify_format(&self, data: &[u8]) -> bool {
        let sample = if data.len() > 4096 { &data[..4096] } else { data };
        let text = String::from_utf8_lossy(sample);
        text.contains("<IPC-2581") || text.contains("xmlns=\"http://webstds.ipc.org/2581\"") || text.contains("<ipc-2581")
    }

    fn parse(&self, data: &[u8]) -> Result<Board, ParseError> {
        let mut reader = Reader::from_reader(data);
        reader.config_mut().trim_text(true);

        let mut board = Board::new();
        let mut packages: HashMap<String, IpcPackage> = HashMap::new();
        let mut components: Vec<IpcComponent> = Vec::new();
        let mut pin_nets: HashMap<(String, String), String> = HashMap::new(); // (refdes, pin) -> net

        let mut current_package = String::new();
        let mut current_pkg_pins = Vec::new();
        let mut in_package = false;
        let mut current_net = String::new();

        let mut unit_scale = 1.0; // default mm

        let mut buf = Vec::new();

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                    let qname = e.name();
                    let tag = qname.as_ref();
                    match tag {
                        b"CadHeader" | b"Units" => {
                            for attr in e.attributes().flatten() {
                                if attr.key.as_ref() == b"units" {
                                    let u = String::from_utf8_lossy(&attr.value).to_uppercase();
                                    if u.contains("INCH") {
                                        unit_scale = 25.4;
                                    } else if u.contains("MICRON") {
                                        unit_scale = 0.001;
                                    } else if u.contains("MILLIMETER") || u.contains("MM") {
                                        unit_scale = 1.0;
                                    }
                                }
                            }
                        }
                        b"Package" => {
                            in_package = true;
                            current_pkg_pins.clear();
                            for attr in e.attributes().flatten() {
                                if attr.key.as_ref() == b"name" {
                                    current_package = String::from_utf8_lossy(&attr.value).to_string();
                                }
                            }
                        }
                        b"Pin" if in_package => {
                            let mut pin = IpcPkgPin::default();
                            for attr in e.attributes().flatten() {
                                match attr.key.as_ref() {
                                    b"number" | b"name" => pin.number = String::from_utf8_lossy(&attr.value).to_string(),
                                    _ => {}
                                }
                            }
                            current_pkg_pins.push(pin);
                        }
                        b"Xform" if in_package => {
                            // Location of pin inside package
                            if let Some(pin) = current_pkg_pins.last_mut() {
                                for attr in e.attributes().flatten() {
                                    match attr.key.as_ref() {
                                        b"x" => pin.x = String::from_utf8_lossy(&attr.value).parse().unwrap_or(0.0) * unit_scale,
                                        b"y" => pin.y = String::from_utf8_lossy(&attr.value).parse().unwrap_or(0.0) * unit_scale,
                                        _ => {}
                                    }
                                }
                            }
                        }
                        b"Component" => {
                            let mut comp = IpcComponent { side: Side::Top, ..Default::default() };
                            for attr in e.attributes().flatten() {
                                match attr.key.as_ref() {
                                    b"refDes" => comp.ref_des = String::from_utf8_lossy(&attr.value).to_string(),
                                    b"packageRef" => comp.package_ref = String::from_utf8_lossy(&attr.value).to_string(),
                                    b"layerRef" => {
                                        let l = String::from_utf8_lossy(&attr.value).to_uppercase();
                                        if l.contains("BOT") {
                                            comp.side = Side::Bottom;
                                        }
                                    }
                                    _ => {}
                                }
                            }
                            components.push(comp);
                        }
                        b"Xform" if !in_package => {
                            if let Some(comp) = components.last_mut() {
                                for attr in e.attributes().flatten() {
                                    match attr.key.as_ref() {
                                        b"x" => comp.x = String::from_utf8_lossy(&attr.value).parse().unwrap_or(0.0) * unit_scale,
                                        b"y" => comp.y = String::from_utf8_lossy(&attr.value).parse().unwrap_or(0.0) * unit_scale,
                                        b"rotation" => comp.rot_deg = String::from_utf8_lossy(&attr.value).parse().unwrap_or(0.0),
                                        _ => {}
                                    }
                                }
                            }
                        }
                        b"LogicalNet" => {
                            for attr in e.attributes().flatten() {
                                if attr.key.as_ref() == b"name" {
                                    current_net = String::from_utf8_lossy(&attr.value).to_string();
                                }
                            }
                        }
                        b"PinRef" => {
                            let mut comp_ref = String::new();
                            let mut pin_ref = String::new();
                            for attr in e.attributes().flatten() {
                                match attr.key.as_ref() {
                                    b"componentRef" => comp_ref = String::from_utf8_lossy(&attr.value).to_string(),
                                    b"pin" => pin_ref = String::from_utf8_lossy(&attr.value).to_string(),
                                    _ => {}
                                }
                            }
                            if !comp_ref.is_empty() && !pin_ref.is_empty() && !current_net.is_empty() {
                                pin_nets.insert((comp_ref, pin_ref), current_net.clone());
                            }
                        }
                        _ => {}
                    }
                }
                Ok(Event::End(ref e)) => {
                    let qname = e.name();
                    let tag = qname.as_ref();
                    if tag == b"Package" {
                        in_package = false;
                        packages.insert(current_package.clone(), IpcPackage { pins: current_pkg_pins.clone() });
                    } else if tag == b"LogicalNet" {
                        current_net.clear();
                    }
                }
                Ok(Event::Eof) => break,
                Err(err) => return Err(ParseError::Other(format!("IPC-2581 XML parse error: {}", err))),
                _ => {}
            }
            buf.clear();
        }

        // Build Board Parts & Pins
        for comp in components {
            let pkg = packages.get(&comp.package_ref);
            let part_id = board.parts.len();

            board.parts.push(Part {
                id: part_id,
                name: comp.ref_des.clone(),
                mfg_code: comp.package_ref.clone(),
                side: comp.side,
                part_type: PartType::Smd,
                p1: Point::new(comp.x, comp.y),
                p2: Point::new(comp.x, comp.y),
                pin_indices: Vec::new(),
                bounding_box: None,
                rotation_deg: comp.rot_deg,
                ..Default::default()
            });

            if let Some(pkg) = pkg {
                let rot_rad = comp.rot_deg.to_radians();
                let cos_a = rot_rad.cos();
                let sin_a = rot_rad.sin();

                for p in &pkg.pins {
                    let px = if comp.side == Side::Bottom { -p.x } else { p.x };
                    let py = p.y;
                    let wx = comp.x + px * cos_a - py * sin_a;
                    let wy = comp.y + px * sin_a + py * cos_a;

                    let net = pin_nets
                        .get(&(comp.ref_des.clone(), p.number.clone()))
                        .cloned()
                        .unwrap_or_default();

                    let pin_id = board.pins.len();
                    board.pins.push(Pin {
                        id: pin_id,
                        part_id,
                        name: p.number.clone(),
                        pos: Point::new(wx, wy),
                        side: comp.side,
                        net_name: net,
                        radius: 0.4,
                        shape: PadShape::Rect,
                        width: Some(0.8),
                        height: Some(0.8),
                        rotation_deg: comp.rot_deg,
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
