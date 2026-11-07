//! EAGLE `.sch` XML format parser.
//!
//! EAGLE coordinates are in millimeters with Y up; this parser converts to Y down.
//! Connectivity is explicitly declared in `<nets>` sections.

use quick_xml::events::Event;
use quick_xml::reader::Reader;
use std::collections::{BTreeMap, HashMap};

use super::model::*;
use super::connectivity::solve_connectivity;
use crate::parser::ParseError;

#[derive(Default)]
struct EagleDeviceConnect {
    gate: String,
    pin: String,
    pad: String,
}

#[derive(Default)]
struct EagleDevice {
    name: String,
    package: String,
    connects: Vec<EagleDeviceConnect>,
}

#[derive(Default)]
struct EagleDeviceset {
    name: String,
    gates: HashMap<String, String>, // gate_name -> symbol_name
    devices: Vec<EagleDevice>,
}

#[derive(Default)]
struct EaglePinDef {
    name: String,
    x: f64,
    y: f64,
    rot: f64,
    length: f64,
    direction: String,
}

#[derive(Default)]
struct EagleSymbolDef {
    name: String,
    pins: Vec<EaglePinDef>,
    wires: Vec<SchWire>,
}

#[derive(Default)]
struct EaglePart {
    name: String,
    library: String,
    deviceset: String,
    device: String,
    value: String,
}

#[derive(Default)]
struct EagleInstance {
    part: String,
    gate: String,
    x: f64,
    y: f64,
    rot: f64,
    mirror_x: bool,
}

fn parse_rot(rot_str: &str) -> (f64, bool) {
    let mut mirror = false;
    let s = rot_str.trim();
    let s = if let Some(stripped) = s.strip_prefix('M') {
        mirror = true;
        stripped
    } else {
        s
    };
    let s = if let Some(stripped) = s.strip_prefix('S') {
        stripped
    } else {
        s
    };
    let s = if let Some(stripped) = s.strip_prefix('R') {
        stripped
    } else {
        s
    };
    let deg: f64 = s.parse().unwrap_or(0.0);
    (deg, mirror)
}

pub fn parse_eagle_sch(xml_content: &str) -> Result<Schematic, ParseError> {
    let mut reader = Reader::from_str(xml_content);
    reader.config_mut().trim_text(true);

    let mut libraries: HashMap<String, HashMap<String, EagleSymbolDef>> = HashMap::new();
    let mut devicesets: HashMap<String, HashMap<String, EagleDeviceset>> = HashMap::new();
    let mut parts: HashMap<String, EaglePart> = HashMap::new();

    let mut current_lib = String::new();
    let mut current_sym: Option<EagleSymbolDef> = None;
    let mut current_dset: Option<EagleDeviceset> = None;
    let mut current_dev: Option<EagleDevice> = None;

    let mut sheet_instances: Vec<EagleInstance> = Vec::new();
    let mut sheet_wires: Vec<SchWire> = Vec::new();
    let mut sheet_junctions: Vec<SchJunction> = Vec::new();
    let mut sheet_labels: Vec<SchLabel> = Vec::new();
    let mut sheet_graphics: Vec<SchGraphic> = Vec::new();

    let mut declared_nets: Vec<SchDeclaredNet> = Vec::new();
    let mut current_net_name = String::new();
    let mut current_net_pins: Vec<SchNetPinRef> = Vec::new();
    let mut current_net_wires: Vec<String> = Vec::new();

    let mut in_sheet = false;
    let mut in_segment = false;

    let mut wire_counter = 0;

    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                match e.name().as_ref() {
                    b"library" => {
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"name" {
                                current_lib = String::from_utf8_lossy(&attr.value).to_string();
                                libraries.entry(current_lib.clone()).or_default();
                                devicesets.entry(current_lib.clone()).or_default();
                            }
                        }
                    }
                    b"symbol" => {
                        let mut name = String::new();
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"name" {
                                name = String::from_utf8_lossy(&attr.value).to_string();
                            }
                        }
                        current_sym = Some(EagleSymbolDef { name, ..Default::default() });
                    }
                    b"deviceset" => {
                        let mut name = String::new();
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"name" {
                                name = String::from_utf8_lossy(&attr.value).to_string();
                            }
                        }
                        current_dset = Some(EagleDeviceset { name, ..Default::default() });
                    }
                    b"device" => {
                        let mut name = String::new();
                        let mut package = String::new();
                        for attr in e.attributes().flatten() {
                            match attr.key.as_ref() {
                                b"name" => name = String::from_utf8_lossy(&attr.value).to_string(),
                                b"package" => package = String::from_utf8_lossy(&attr.value).to_string(),
                                _ => {}
                            }
                        }
                        current_dev = Some(EagleDevice { name, package, ..Default::default() });
                    }
                    b"sheet" => {
                        in_sheet = true;
                    }
                    b"net" => {
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"name" {
                                current_net_name = String::from_utf8_lossy(&attr.value).to_string();
                            }
                        }
                        current_net_pins.clear();
                        current_net_wires.clear();
                    }
                    b"segment" => {
                        in_segment = true;
                    }
                    _ => {}
                }
            }
            Ok(Event::Empty(ref e)) => {
                match e.name().as_ref() {
                    b"pin" => {
                        if let Some(sym) = current_sym.as_mut() {
                            let mut p = EaglePinDef::default();
                            p.length = 2.54; // default middle
                            for attr in e.attributes().flatten() {
                                match attr.key.as_ref() {
                                    b"name" => p.name = String::from_utf8_lossy(&attr.value).to_string(),
                                    b"x" => p.x = String::from_utf8_lossy(&attr.value).parse().unwrap_or(0.0),
                                    b"y" => p.y = String::from_utf8_lossy(&attr.value).parse().unwrap_or(0.0),
                                    b"rot" => {
                                        let (r, _) = parse_rot(&String::from_utf8_lossy(&attr.value));
                                        p.rot = r;
                                    }
                                    b"length" => {
                                        let l = match attr.value.as_ref() {
                                            b"point" => 0.0,
                                            b"short" => 2.54,
                                            b"middle" => 5.08,
                                            b"long" => 7.62,
                                            _ => 2.54,
                                        };
                                        p.length = l;
                                    }
                                    b"direction" => p.direction = String::from_utf8_lossy(&attr.value).to_string(),
                                    _ => {}
                                }
                            }
                            sym.pins.push(p);
                        }
                    }
                    b"gate" => {
                        if let Some(dset) = current_dset.as_mut() {
                            let mut g_name = String::new();
                            let mut s_name = String::new();
                            for attr in e.attributes().flatten() {
                                match attr.key.as_ref() {
                                    b"name" => g_name = String::from_utf8_lossy(&attr.value).to_string(),
                                    b"symbol" => s_name = String::from_utf8_lossy(&attr.value).to_string(),
                                    _ => {}
                                }
                            }
                            dset.gates.insert(g_name, s_name);
                        }
                    }
                    b"connect" => {
                        if let Some(dev) = current_dev.as_mut() {
                            let mut gate = String::new();
                            let mut pin = String::new();
                            let mut pad = String::new();
                            for attr in e.attributes().flatten() {
                                match attr.key.as_ref() {
                                    b"gate" => gate = String::from_utf8_lossy(&attr.value).to_string(),
                                    b"pin" => pin = String::from_utf8_lossy(&attr.value).to_string(),
                                    b"pad" => pad = String::from_utf8_lossy(&attr.value).to_string(),
                                    _ => {}
                                }
                            }
                            dev.connects.push(EagleDeviceConnect { gate, pin, pad });
                        }
                    }
                    b"part" => {
                        let mut p = EaglePart::default();
                        for attr in e.attributes().flatten() {
                            match attr.key.as_ref() {
                                b"name" => p.name = String::from_utf8_lossy(&attr.value).to_string(),
                                b"library" => p.library = String::from_utf8_lossy(&attr.value).to_string(),
                                b"deviceset" => p.deviceset = String::from_utf8_lossy(&attr.value).to_string(),
                                b"device" => p.device = String::from_utf8_lossy(&attr.value).to_string(),
                                b"value" => p.value = String::from_utf8_lossy(&attr.value).to_string(),
                                _ => {}
                            }
                        }
                        parts.insert(p.name.clone(), p);
                    }
                    b"instance" => {
                        if in_sheet {
                            let mut inst = EagleInstance::default();
                            for attr in e.attributes().flatten() {
                                match attr.key.as_ref() {
                                    b"part" => inst.part = String::from_utf8_lossy(&attr.value).to_string(),
                                    b"gate" => inst.gate = String::from_utf8_lossy(&attr.value).to_string(),
                                    b"x" => inst.x = String::from_utf8_lossy(&attr.value).parse().unwrap_or(0.0),
                                    b"y" => inst.y = String::from_utf8_lossy(&attr.value).parse().unwrap_or(0.0),
                                    b"rot" => {
                                        let (r, m) = parse_rot(&String::from_utf8_lossy(&attr.value));
                                        inst.rot = r;
                                        inst.mirror_x = m;
                                    }
                                    _ => {}
                                }
                            }
                            sheet_instances.push(inst);
                        }
                    }
                    b"wire" => {
                        let mut x1 = 0.0;
                        let mut y1 = 0.0;
                        let mut x2 = 0.0;
                        let mut y2 = 0.0;
                        let mut width = 0.15;
                        for attr in e.attributes().flatten() {
                            match attr.key.as_ref() {
                                b"x1" => x1 = String::from_utf8_lossy(&attr.value).parse().unwrap_or(0.0),
                                b"y1" => y1 = String::from_utf8_lossy(&attr.value).parse().unwrap_or(0.0),
                                b"x2" => x2 = String::from_utf8_lossy(&attr.value).parse().unwrap_or(0.0),
                                b"y2" => y2 = String::from_utf8_lossy(&attr.value).parse().unwrap_or(0.0),
                                b"width" => width = String::from_utf8_lossy(&attr.value).parse().unwrap_or(0.15),
                                _ => {}
                            }
                        }
                        // Note: EAGLE Y is pointing up, invert to Y down
                        let pt1 = SchPoint::new(x1, -y1);
                        let pt2 = SchPoint::new(x2, -y2);

                        if in_segment {
                            wire_counter += 1;
                            let wid = format!("w{}", wire_counter);
                            sheet_wires.push(SchWire { id: wid.clone(), a: pt1, b: pt2 });
                            current_net_wires.push(wid);
                        } else if let Some(sym) = current_sym.as_mut() {
                            sym.wires.push(SchWire { id: format!("w{}", sym.wires.len() + 1), a: pt1, b: pt2 });
                        } else if in_sheet {
                            sheet_graphics.push(SchGraphic::Polyline {
                                points: vec![pt1, pt2],
                                width,
                                filled: false,
                            });
                        }
                    }
                    b"pinref" => {
                        if in_segment {
                            let mut part = String::new();
                            let mut gate = String::new();
                            let mut pin = String::new();
                            for attr in e.attributes().flatten() {
                                match attr.key.as_ref() {
                                    b"part" => part = String::from_utf8_lossy(&attr.value).to_string(),
                                    b"gate" => gate = String::from_utf8_lossy(&attr.value).to_string(),
                                    b"pin" => pin = String::from_utf8_lossy(&attr.value).to_string(),
                                    _ => {}
                                }
                            }
                            let sym_id = format!("{}:{}", part, gate);
                            current_net_pins.push(SchNetPinRef {
                                instance_path: "".to_string(),
                                def_id: "sheet:1".to_string(),
                                symbol_id: sym_id,
                                pin_id: pin,
                            });
                        }
                    }
                    b"junction" => {
                        let mut x = 0.0;
                        let mut y = 0.0;
                        for attr in e.attributes().flatten() {
                            match attr.key.as_ref() {
                                b"x" => x = String::from_utf8_lossy(&attr.value).parse().unwrap_or(0.0),
                                b"y" => y = String::from_utf8_lossy(&attr.value).parse().unwrap_or(0.0),
                                _ => {}
                            }
                        }
                        sheet_junctions.push(SchJunction {
                            id: format!("j{}", sheet_junctions.len() + 1),
                            at: SchPoint::new(x, -y),
                        });
                    }
                    b"label" => {
                        let mut x = 0.0;
                        let mut y = 0.0;
                        let mut rot = 0.0;
                        for attr in e.attributes().flatten() {
                            match attr.key.as_ref() {
                                b"x" => x = String::from_utf8_lossy(&attr.value).parse().unwrap_or(0.0),
                                b"y" => y = String::from_utf8_lossy(&attr.value).parse().unwrap_or(0.0),
                                b"rot" => {
                                    let (r, _) = parse_rot(&String::from_utf8_lossy(&attr.value));
                                    rot = r;
                                }
                                _ => {}
                            }
                        }
                        sheet_labels.push(SchLabel {
                            id: format!("l{}", sheet_labels.len() + 1),
                            kind: SchLabelKind::Local,
                            text: current_net_name.clone(),
                            at: SchPoint::new(x, -y),
                            angle: rot,
                        });
                    }
                    _ => {}
                }
            }
            Ok(Event::End(ref e)) => {
                match e.name().as_ref() {
                    b"symbol" => {
                        if let Some(sym) = current_sym.take() {
                            if let Some(lib_symbols) = libraries.get_mut(&current_lib) {
                                lib_symbols.insert(sym.name.clone(), sym);
                            }
                        }
                    }
                    b"device" => {
                        if let Some(dev) = current_dev.take() {
                            if let Some(dset) = current_dset.as_mut() {
                                dset.devices.push(dev);
                            }
                        }
                    }
                    b"deviceset" => {
                        if let Some(dset) = current_dset.take() {
                            if let Some(lib_dsets) = devicesets.get_mut(&current_lib) {
                                lib_dsets.insert(dset.name.clone(), dset);
                            }
                        }
                    }
                    b"net" => {
                        if !current_net_name.is_empty() {
                            declared_nets.push(SchDeclaredNet {
                                name: current_net_name.clone(),
                                pins: current_net_pins.clone(),
                                wire_ids: current_net_wires.clone(),
                            });
                        }
                    }
                    b"segment" => {
                        in_segment = false;
                    }
                    b"sheet" => {
                        in_sheet = false;
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(ParseError::SyntaxError {
                    line: 0,
                    message: format!("XML error: {e}"),
                });
            }
            _ => {}
        }
        buf.clear();
    }

    // Build SchSymbols from placed instances
    let mut sheet_symbols = Vec::new();
    let mut bounds = SchBounds::empty();

    for inst in &sheet_instances {
        let part = match parts.get(&inst.part) {
            Some(p) => p,
            None => continue,
        };

        let lib_symbols = match libraries.get(&part.library) {
            Some(s) => s,
            None => continue,
        };

        let lib_dsets = match devicesets.get(&part.library) {
            Some(d) => d,
            None => continue,
        };

        let dset = match lib_dsets.get(&part.deviceset) {
            Some(d) => d,
            None => continue,
        };

        let sym_name = match dset.gates.get(&inst.gate) {
            Some(s) => s,
            None => continue,
        };

        let sym_def = match lib_symbols.get(sym_name) {
            Some(s) => s,
            None => continue,
        };

        // Find connects in device
        let dev = dset.devices.iter().find(|d| d.name == part.device)
            .or_else(|| dset.devices.first());

        // Transform symbol coordinates
        // In EAGLE, symbol origin is placed at (inst.x, inst.y), rotated and mirrored
        let rad = (-inst.rot).to_radians(); // Counter-clockwise in Y-down screen
        let cos_r = rad.cos();
        let sin_r = rad.sin();

        let transform = |x: f64, y: f64| -> SchPoint {
            let mx = if inst.mirror_x { -x } else { x };
            let my = -y; // EAGLE Y is up, transform to screen Y down
            let rx = mx * cos_r - my * sin_r;
            let ry = mx * sin_r + my * cos_r;
            SchPoint::new(inst.x + rx, -inst.y + ry)
        };

        let mut placed_pins = Vec::new();
        for p in &sym_def.pins {
            let tip = transform(p.x, p.y);
            // Pin direction vector
            let p_len = p.length;
            let (dx, dy) = match p.rot as i64 % 360 {
                0 => (-p_len, 0.0),
                90 => (0.0, p_len),
                180 => (p_len, 0.0),
                270 => (0.0, -p_len),
                _ => (-p_len, 0.0),
            };
            let body = transform(p.x + dx, p.y + dy);

            // Find pad mapping
            let pad_num = dev.and_then(|d| {
                d.connects.iter().find(|c| c.gate == inst.gate && c.pin == p.name).map(|c| c.pad.clone())
            }).unwrap_or_else(|| p.name.clone());

            let p_type = match p.direction.as_str() {
                "in" => SchPinType::Input,
                "out" => SchPinType::Output,
                "io" => SchPinType::Bidirectional,
                "pwr" | "sup" => SchPinType::PowerIn,
                "pas" => SchPinType::Passive,
                "hiz" => SchPinType::TriState,
                "nc" => SchPinType::NoConnect,
                _ => SchPinType::Unspecified,
            };

            placed_pins.push(SchPin {
                id: p.name.clone(),
                number: pad_num,
                name: p.name.clone(),
                at: tip,
                body,
                pin_type: p_type,
                hidden: false,
                unit: 1,
                implicit_net: None,
            });
            bounds.union_point(tip);
            bounds.union_point(body);
        }

        let mut placed_graphics = Vec::new();
        for w in &sym_def.wires {
            let pa = transform(w.a.x, -w.a.y);
            let pb = transform(w.b.x, -w.b.y);
            placed_graphics.push(SchGraphic::Polyline {
                points: vec![pa, pb],
                width: 0.15,
                filled: false,
            });
            bounds.union_point(pa);
            bounds.union_point(pb);
        }

        let sym_id = format!("{}:{}", inst.part, inst.gate);
        sheet_symbols.push(SchSymbol {
            id: sym_id,
            lib_id: format!("{}:{}:{}", part.library, part.deviceset, part.device),
            ref_default: inst.part.clone(),
            instances: BTreeMap::new(),
            value: part.value.clone(),
            footprint: dev.map(|d| d.package.clone()).unwrap_or_default(),
            unit: 1,
            unit_count: 1,
            at: SchPoint::new(inst.x, -inst.y),
            rotation: inst.rot,
            mirror_x: inst.mirror_x,
            mirror_y: false,
            pins: placed_pins,
            graphics: placed_graphics,
            fields: vec![
                SchField {
                    name: "Reference".to_string(),
                    value: inst.part.clone(),
                    at: Some(SchPoint::new(inst.x, -inst.y - 2.5)),
                    angle: 0.0,
                    hidden: false,
                },
                SchField {
                    name: "Value".to_string(),
                    value: part.value.clone(),
                    at: Some(SchPoint::new(inst.x, -inst.y + 2.5)),
                    angle: 0.0,
                    hidden: false,
                },
            ],
            power_net: None,
            virtual_symbol: false,
            dnp: false,
            bounds,
        });
    }

    for w in &sheet_wires {
        bounds.union_point(w.a);
        bounds.union_point(w.b);
    }

    let sheet_def = SchSheetDef {
        id: "sheet:1".to_string(),
        name: "Sheet 1".to_string(),
        file: "".to_string(),
        title: "".to_string(),
        symbols: sheet_symbols,
        wires: sheet_wires,
        buses: Vec::new(),
        bus_entries: Vec::new(),
        junctions: sheet_junctions,
        no_connects: Vec::new(),
        labels: sheet_labels,
        graphics: sheet_graphics,
        bounds,
    };

    let instance = SchSheetInstance {
        path: "".to_string(),
        def_id: "sheet:1".to_string(),
        name: "Sheet 1".to_string(),
        page: "1".to_string(),
    };

    let mut schematic = Schematic {
        format: SchematicFormat::EagleSch,
        format_label: "EAGLE Schematic".to_string(),
        name: "Schematic".to_string(),
        defs: vec![sheet_def],
        root_def_id: "sheet:1".to_string(),
        instances: vec![instance],
        declared_nets,
        nets: Vec::new(),
        diagnostics: Vec::new(),
    };

    solve_connectivity(&mut schematic);
    Ok(schematic)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_basic_eagle_sch() {
        let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<eagle version="9.6.2">
  <drawing>
    <schematic>
      <libraries>
        <library name="resistor">
          <symbols>
            <symbol name="R-US">
              <wire x1="-2.54" y1="0" x2="2.54" y2="0" width="0.1524" layer="94"/>
              <pin name="1" x="-5.08" y="0" length="short" direction="pas"/>
              <pin name="2" x="5.08" y="0" length="short" direction="pas" rot="R180"/>
            </symbol>
          </symbols>
          <devicesets>
            <deviceset name="R-US_">
              <gates>
                <gate name="G$1" symbol="R-US" x="0" y="0"/>
              </gates>
              <devices>
                <device name="0805" package="R0805">
                  <connects>
                    <connect gate="G$1" pin="1" pad="1"/>
                    <connect gate="G$1" pin="2" pad="2"/>
                  </connects>
                </device>
              </devices>
            </deviceset>
          </devicesets>
        </library>
      </libraries>
      <parts>
        <part name="R1" library="resistor" deviceset="R-US_" device="0805" value="10k"/>
      </parts>
      <sheets>
        <sheet>
          <instances>
            <instance part="R1" gate="G$1" x="25.4" y="25.4"/>
          </instances>
          <nets>
            <net name="N$1" class="0">
              <segment>
                <pinref part="R1" gate="G$1" pin="1"/>
                <wire x1="20.32" y1="25.4" x2="10.16" y2="25.4" width="0.1524" layer="91"/>
              </segment>
            </net>
          </nets>
        </sheet>
      </sheets>
    </schematic>
  </drawing>
</eagle>"#;

        let res = parse_eagle_sch(xml);
        assert!(res.is_ok(), "Failed to parse EAGLE: {:?}", res.err());
        let sch = res.unwrap();
        assert_eq!(sch.defs.len(), 1);
        let sheet = &sch.defs[0];
        assert_eq!(sheet.symbols.len(), 1);
        assert_eq!(sheet.symbols[0].ref_default, "R1");
        assert_eq!(sheet.symbols[0].pins.len(), 2);
        assert_eq!(sheet.symbols[0].pins[0].number, "1");
        assert_eq!(sheet.wires.len(), 1);

        // Net connectivity
        let net = sch.find_net_by_name("N$1");
        assert!(net.is_some());
        let net = net.unwrap();
        assert_eq!(net.members.len(), 1);
        assert_eq!(net.members[0].ref_des, "R1");
        assert_eq!(net.members[0].pin_number, "1");
    }
}
