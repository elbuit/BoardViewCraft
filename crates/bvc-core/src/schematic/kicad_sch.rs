//! KiCad 6-9 `.kicad_sch` S-Expression format parser.
//!
//! KiCad coordinates in schematic are in millimeters, with Y down on screen.
//! Symbol definitions in library are Y-up with pin orientations pointing into symbol body.

use std::collections::{BTreeMap, HashMap};

use super::model::*;
use super::connectivity::solve_connectivity;
use crate::parser::kicad::{parse_sexpr, SExpr};
use crate::parser::ParseError;

#[derive(Default, Clone)]
struct LibPin {
    number: String,
    name: String,
    pin_type: SchPinType,
    at: SchPoint,
    length: f64,
    angle: f64,
    unit: u32,
    hide: bool,
}

#[derive(Default, Clone)]
struct LibSymbol {
    pins: Vec<LibPin>,
    polylines: Vec<Vec<SchPoint>>,
    rects: Vec<(SchPoint, SchPoint)>,
    circles: Vec<(SchPoint, f64)>,
    power: bool,
}

fn find_child<'a, 'b>(nodes: &'b [SExpr<'a>], key: &str) -> Option<&'b [SExpr<'a>]> {
    for node in nodes {
        if let SExpr::List(list) = node {
            if let Some(head) = list.first().and_then(|h| h.as_atom()) {
                if head == key {
                    return Some(list.as_slice());
                }
            }
        }
    }
    None
}

fn find_all_children<'a, 'b>(nodes: &'b [SExpr<'a>], key: &str) -> Vec<&'b [SExpr<'a>]> {
    let mut out = Vec::new();
    for node in nodes {
        if let SExpr::List(list) = node {
            if let Some(head) = list.first().and_then(|h| h.as_atom()) {
                if head == key {
                    out.push(list.as_slice());
                }
            }
        }
    }
    out
}

fn parse_point(list: &[SExpr]) -> Option<SchPoint> {
    // (xy X Y) or (at X Y [angle])
    let x = list.get(1)?.as_atom()?.parse::<f64>().ok()?;
    let y = list.get(2)?.as_atom()?.parse::<f64>().ok()?;
    Some(SchPoint::new(x, y))
}

fn parse_pin_type(s: &str) -> SchPinType {
    match s {
        "input" => SchPinType::Input,
        "output" => SchPinType::Output,
        "bidirectional" => SchPinType::Bidirectional,
        "tri_state" => SchPinType::TriState,
        "passive" => SchPinType::Passive,
        "free" => SchPinType::Free,
        "unspecified" => SchPinType::Unspecified,
        "power_in" => SchPinType::PowerIn,
        "power_out" => SchPinType::PowerOut,
        "open_collector" => SchPinType::OpenCollector,
        "open_emitter" => SchPinType::OpenEmitter,
        "no_connect" => SchPinType::NoConnect,
        _ => SchPinType::Unspecified,
    }
}

pub fn parse_kicad_sch(content: &str) -> Result<Schematic, ParseError> {
    let nodes = parse_sexpr(content)?;
    let root = nodes.first().ok_or_else(|| ParseError::SyntaxError {
        line: 0,
        message: "Empty S-Expression schematic".to_string(),
    })?;

    let root_list = match root {
        SExpr::List(l) if l.first().and_then(|h| h.as_atom()) == Some("kicad_sch") => l.as_slice(),
        _ => {
            return Err(ParseError::SyntaxError {
                line: 0,
                message: "Expected (kicad_sch ...) root element".to_string(),
            });
        }
    };

    // 1. Parse lib_symbols
    let mut lib_symbols: HashMap<String, LibSymbol> = HashMap::new();
    if let Some(lib_syms_node) = find_child(root_list, "lib_symbols") {
        for sym_node in find_all_children(&lib_syms_node[1..], "symbol") {
            let lib_id = match sym_node.get(1).and_then(|a| a.as_atom()) {
                Some(id) => id.to_string(),
                None => continue,
            };

            let power = find_child(&sym_node[1..], "power").is_some();
            let mut lib_sym = LibSymbol { power, ..Default::default() };

            // Units and body parts are nested inside symbol nodes: (symbol "LIB_ID_1_1" ...)
            let sub_symbols = find_all_children(&sym_node[1..], "symbol");
            let parse_units = if sub_symbols.is_empty() {
                vec![&sym_node[1..]]
            } else {
                sub_symbols.into_iter().map(|s| &s[1..]).collect()
            };

            for unit_slice in parse_units {
                // Pins
                for pin_node in find_all_children(unit_slice, "pin") {
                    let pin_type = pin_node.get(1).and_then(|a| a.as_atom()).map(parse_pin_type).unwrap_or(SchPinType::Unspecified);
                    let mut pin_at = SchPoint::ZERO;
                    let mut pin_len = 2.54;
                    let mut pin_angle = 0.0;
                    if let Some(at_node) = find_child(&pin_node[1..], "at") {
                        if let Some(pt) = parse_point(at_node) {
                            pin_at = pt;
                        }
                        if let Some(ang) = at_node.get(3).and_then(|a| a.as_atom()).and_then(|s| s.parse::<f64>().ok()) {
                            pin_angle = ang;
                        }
                    }
                    if let Some(len_node) = find_child(&pin_node[1..], "length") {
                        if let Some(l) = len_node.get(1).and_then(|a| a.as_atom()).and_then(|s| s.parse::<f64>().ok()) {
                            pin_len = l;
                        }
                    }
                    let mut pin_num = String::new();
                    if let Some(num_node) = find_child(&pin_node[1..], "number") {
                        if let Some(n) = num_node.get(1).and_then(|a| a.as_atom()) {
                            pin_num = n.to_string();
                        }
                    }
                    let mut pin_name = String::new();
                    if let Some(name_node) = find_child(&pin_node[1..], "name") {
                        if let Some(n) = name_node.get(1).and_then(|a| a.as_atom()) {
                            pin_name = n.to_string();
                        }
                    }

                    lib_sym.pins.push(LibPin {
                        number: pin_num,
                        name: pin_name,
                        pin_type,
                        at: pin_at,
                        length: pin_len,
                        angle: pin_angle,
                        unit: 1,
                        hide: false,
                    });
                }

                // Polylines
                for poly_node in find_all_children(unit_slice, "polyline") {
                    if let Some(pts_node) = find_child(&poly_node[1..], "pts") {
                        let mut poly = Vec::new();
                        for xy_node in find_all_children(&pts_node[1..], "xy") {
                            if let Some(pt) = parse_point(xy_node) {
                                poly.push(pt);
                            }
                        }
                        if !poly.is_empty() {
                            lib_sym.polylines.push(poly);
                        }
                    }
                }

                // Rectangles
                for rect_node in find_all_children(unit_slice, "rectangle") {
                    let start = find_child(&rect_node[1..], "start").and_then(parse_point);
                    let end = find_child(&rect_node[1..], "end").and_then(parse_point);
                    if let (Some(s), Some(e)) = (start, end) {
                        lib_sym.rects.push((s, e));
                    }
                }

                // Circles
                for circle_node in find_all_children(unit_slice, "circle") {
                    let center = find_child(&circle_node[1..], "center").and_then(parse_point);
                    let radius = find_child(&circle_node[1..], "radius")
                        .and_then(|r| r.get(1))
                        .and_then(|a| a.as_atom())
                        .and_then(|s| s.parse::<f64>().ok());
                    if let (Some(c), Some(r)) = (center, radius) {
                        lib_sym.circles.push((c, r));
                    }
                }
            }

            lib_symbols.insert(lib_id, lib_sym);
        }
    }

    // 2. Parse sheet wires
    let mut wires = Vec::new();
    let mut wire_counter = 0;
    for wire_node in find_all_children(&root_list[1..], "wire") {
        if let Some(pts_node) = find_child(&wire_node[1..], "pts") {
            let xy_nodes = find_all_children(&pts_node[1..], "xy");
            if xy_nodes.len() >= 2 {
                if let (Some(a), Some(b)) = (parse_point(xy_nodes[0]), parse_point(xy_nodes[1])) {
                    wire_counter += 1;
                    wires.push(SchWire {
                        id: format!("w{}", wire_counter),
                        a,
                        b,
                    });
                }
            }
        }
    }

    // 3. Parse junctions
    let mut junctions = Vec::new();
    let mut j_counter = 0;
    for j_node in find_all_children(&root_list[1..], "junction") {
        if let Some(at_node) = find_child(&j_node[1..], "at") {
            if let Some(pt) = parse_point(at_node) {
                j_counter += 1;
                junctions.push(SchJunction {
                    id: format!("j{}", j_counter),
                    at: pt,
                });
            }
        }
    }

    // 4. Parse labels
    let mut labels = Vec::new();
    let mut l_counter = 0;
    for (tag, kind) in &[
        ("label", SchLabelKind::Local),
        ("global_label", SchLabelKind::Global),
        ("hierarchical_label", SchLabelKind::Hierarchical),
    ] {
        for l_node in find_all_children(&root_list[1..], tag) {
            let text = l_node.get(1).and_then(|a| a.as_atom()).unwrap_or("").to_string();
            let mut at = SchPoint::ZERO;
            let mut angle = 0.0;
            if let Some(at_node) = find_child(&l_node[1..], "at") {
                if let Some(pt) = parse_point(at_node) {
                    at = pt;
                }
                if let Some(ang) = at_node.get(3).and_then(|a| a.as_atom()).and_then(|s| s.parse::<f64>().ok()) {
                    angle = ang;
                }
            }
            l_counter += 1;
            labels.push(SchLabel {
                id: format!("l{}", l_counter),
                kind: *kind,
                text,
                at,
                angle,
            });
        }
    }

    // 5. Parse placed symbol instances
    let mut symbols = Vec::new();
    let mut sheet_bounds = SchBounds::empty();

    for sym_node in find_all_children(&root_list[1..], "symbol") {
        let lib_id = match find_child(&sym_node[1..], "lib_id").and_then(|l| l.get(1)).and_then(|a| a.as_atom()) {
            Some(id) => id.to_string(),
            None => continue,
        };

        let mut sym_at = SchPoint::ZERO;
        let mut sym_rot = 0.0;
        if let Some(at_node) = find_child(&sym_node[1..], "at") {
            if let Some(pt) = parse_point(at_node) {
                sym_at = pt;
            }
            if let Some(ang) = at_node.get(3).and_then(|a| a.as_atom()).and_then(|s| s.parse::<f64>().ok()) {
                sym_rot = ang;
            }
        }

        let mirror = find_child(&sym_node[1..], "mirror").and_then(|m| m.get(1)).and_then(|a| a.as_atom());
        let mirror_x = mirror == Some("x");
        let mirror_y = mirror == Some("y");

        let unit = find_child(&sym_node[1..], "unit")
            .and_then(|u| u.get(1))
            .and_then(|a| a.as_atom())
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(1);

        let uuid = find_child(&sym_node[1..], "uuid")
            .and_then(|u| u.get(1))
            .and_then(|a| a.as_atom())
            .unwrap_or("")
            .to_string();

        let dnp = find_child(&sym_node[1..], "dnp").is_some();

        // Properties: Reference, Value, Footprint
        let mut ref_des = String::new();
        let mut val = String::new();
        let mut footprint = String::new();
        let mut fields = Vec::new();

        for prop_node in find_all_children(&sym_node[1..], "property") {
            let key = prop_node.get(1).and_then(|a| a.as_atom()).unwrap_or("");
            let value = prop_node.get(2).and_then(|a| a.as_atom()).unwrap_or("");
            match key {
                "Reference" => ref_des = value.to_string(),
                "Value" => val = value.to_string(),
                "Footprint" => footprint = value.to_string(),
                _ => {}
            }
            fields.push(SchField {
                name: key.to_string(),
                value: value.to_string(),
                at: None,
                angle: 0.0,
                hidden: false,
            });
        }

        // Instances mapping
        let mut instances = BTreeMap::new();
        if let Some(inst_node) = find_child(&sym_node[1..], "instances") {
            if let Some(project_node) = find_child(&inst_node[1..], "project") {
                for path_node in find_all_children(&project_node[1..], "path") {
                    let p_str = path_node.get(1).and_then(|a| a.as_atom()).unwrap_or("");
                    if let Some(r_node) = find_child(&path_node[1..], "reference") {
                        if let Some(r) = r_node.get(1).and_then(|a| a.as_atom()) {
                            instances.insert(p_str.to_string(), r.to_string());
                        }
                    }
                }
            }
        }

        // Resolve library geometry
        let lib_def = lib_symbols.get(&lib_id);
        let power_net = if lib_def.map(|l| l.power).unwrap_or(false) {
            Some(val.clone())
        } else {
            None
        };

        // Coordinate transformation for pins and graphics
        // Order: (1) FLIP_Y: [1, 0, 0, -1]
        //        (2) ROTATION (ccw in Y-down):
        //            rot 0:   [ 1,  0,  0,  1]
        //            rot 90:  [ 0,  1, -1,  0]
        //            rot 180: [-1,  0,  0, -1]
        //            rot 270: [ 0, -1,  1,  0]
        //        (3) MIRROR: x -> [1, 0, 0, -1], y -> [-1, 0, 0, 1]
        let rot_int = ((sym_rot.round() as i64 % 360) + 360) % 360;
        let rot_m: [f64; 4] = match rot_int {
            90 => [0.0, 1.0, -1.0, 0.0],
            180 => [-1.0, 0.0, 0.0, -1.0],
            270 => [0.0, -1.0, 1.0, 0.0],
            _ => [1.0, 0.0, 0.0, 1.0],
        };
        // mul(rot_m, [1, 0, 0, -1])
        let r_flip = [
            rot_m[0] * 1.0 + rot_m[1] * 0.0,
            rot_m[0] * 0.0 + rot_m[1] * -1.0,
            rot_m[2] * 1.0 + rot_m[3] * 0.0,
            rot_m[2] * 0.0 + rot_m[3] * -1.0,
        ];
        let mir_m: [f64; 4] = if mirror_x {
            [1.0, 0.0, 0.0, -1.0]
        } else if mirror_y {
            [-1.0, 0.0, 0.0, 1.0]
        } else {
            [1.0, 0.0, 0.0, 1.0]
        };
        // final m = mul(mir_m, r_flip)
        let mat = [
            mir_m[0] * r_flip[0] + mir_m[1] * r_flip[2],
            mir_m[0] * r_flip[1] + mir_m[1] * r_flip[3],
            mir_m[2] * r_flip[0] + mir_m[3] * r_flip[2],
            mir_m[2] * r_flip[1] + mir_m[3] * r_flip[3],
        ];

        let transform = |lx: f64, ly: f64| -> SchPoint {
            let px = sym_at.x + mat[0] * lx + mat[1] * ly;
            let py = sym_at.y + mat[2] * lx + mat[3] * ly;
            // Snap to 0.0001 mm
            let snap = |v: f64| (v * 10000.0).round() / 10000.0;
            SchPoint::new(snap(px), snap(py))
        };

        let mut placed_pins = Vec::new();
        let mut placed_graphics = Vec::new();
        let mut sym_bounds = SchBounds::empty();

        if let Some(lib) = lib_def {
            for pin in &lib.pins {
                let pin_tip = transform(pin.at.x, pin.at.y);
                // In KiCad, pin line extends along angle into body
                let p_rad = pin.angle.to_radians();
                let bx = pin.at.x + pin.length * p_rad.cos();
                let by = pin.at.y + pin.length * p_rad.sin();
                let pin_body = transform(bx, by);

                placed_pins.push(SchPin {
                    id: format!("{}#{}", uuid, pin.number),
                    number: pin.number.clone(),
                    name: pin.name.clone(),
                    at: pin_tip,
                    body: pin_body,
                    pin_type: pin.pin_type,
                    hidden: pin.hide,
                    unit: pin.unit,
                    implicit_net: None,
                });
                sym_bounds.union_point(pin_tip);
                sym_bounds.union_point(pin_body);
                sheet_bounds.union_point(pin_tip);
                sheet_bounds.union_point(pin_body);
            }

            for poly in &lib.polylines {
                let pts: Vec<SchPoint> = poly.iter().map(|p| transform(p.x, p.y)).collect();
                for pt in &pts {
                    sym_bounds.union_point(*pt);
                    sheet_bounds.union_point(*pt);
                }
                placed_graphics.push(SchGraphic::Polyline {
                    points: pts,
                    width: 0.15,
                    filled: false,
                });
            }

            for (start, end) in &lib.rects {
                let s = transform(start.x, start.y);
                let e = transform(end.x, end.y);
                let min = SchPoint::new(s.x.min(e.x), s.y.min(e.y));
                let max = SchPoint::new(s.x.max(e.x), s.y.max(e.y));
                sym_bounds.union_point(min);
                sym_bounds.union_point(max);
                sheet_bounds.union_point(min);
                sheet_bounds.union_point(max);
                placed_graphics.push(SchGraphic::Rect {
                    min,
                    max,
                    width: 0.15,
                    filled: false,
                });
            }

            for (center, radius) in &lib.circles {
                let c = transform(center.x, center.y);
                sym_bounds.union_point(SchPoint::new(c.x - radius, c.y - radius));
                sym_bounds.union_point(SchPoint::new(c.x + radius, c.y + radius));
                sheet_bounds.union_point(SchPoint::new(c.x - radius, c.y - radius));
                sheet_bounds.union_point(SchPoint::new(c.x + radius, c.y + radius));
                placed_graphics.push(SchGraphic::Circle {
                    center: c,
                    radius: *radius,
                    width: 0.15,
                    filled: false,
                });
            }
        }

        if sym_bounds.is_empty() {
            sym_bounds.union_point(sym_at);
        }

        let sym_id = if uuid.is_empty() { format!("sym_{}", symbols.len() + 1) } else { uuid };
        symbols.push(SchSymbol {
            id: sym_id,
            lib_id,
            ref_default: ref_des,
            instances,
            value: val,
            footprint,
            unit,
            unit_count: 1,
            at: sym_at,
            rotation: sym_rot,
            mirror_x,
            mirror_y,
            pins: placed_pins,
            graphics: placed_graphics,
            fields,
            power_net,
            virtual_symbol: false,
            dnp,
            bounds: sym_bounds,
        });
    }

    for w in &wires {
        sheet_bounds.union_point(w.a);
        sheet_bounds.union_point(w.b);
    }

    let sheet_def = SchSheetDef {
        id: "root".to_string(),
        name: "Root".to_string(),
        file: "".to_string(),
        title: "".to_string(),
        symbols,
        wires,
        buses: Vec::new(),
        bus_entries: Vec::new(),
        junctions,
        no_connects: Vec::new(),
        labels,
        graphics: Vec::new(),
        bounds: sheet_bounds,
    };

    let instance = SchSheetInstance {
        path: "".to_string(),
        def_id: "root".to_string(),
        name: "Root".to_string(),
        page: "1".to_string(),
    };

    let mut schematic = Schematic {
        format: SchematicFormat::KiCadSch,
        format_label: "KiCad Schematic (v6-v9)".to_string(),
        name: "Schematic".to_string(),
        defs: vec![sheet_def],
        root_def_id: "root".to_string(),
        instances: vec![instance],
        declared_nets: Vec::new(),
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
    fn test_parse_basic_kicad_sch() {
        let content = r#"(kicad_sch (version 20231120) (generator eeschema)
  (uuid "00000000-0000-0000-0000-000000000001")
  (paper "A4")
  (lib_symbols
    (symbol "Device:R" (pin_numbers hide) (pin_names (offset 0))
      (symbol "R_0_1"
        (rectangle (start -1.016 -2.54) (end 1.016 2.54) (stroke (width 0.254)) (fill (type none)))
      )
      (symbol "R_1_1"
        (pin passive line (at 0 3.81 270) (length 1.27) (name "~") (number "1"))
        (pin passive line (at 0 -3.81 90) (length 1.27) (name "~") (number "2"))
      )
    )
  )
  (wire (pts (xy 100 103.81) (xy 100 120)) (stroke (width 0)))
  (symbol (lib_id "Device:R") (at 100 100 0) (unit 1)
    (uuid "00000000-0000-0000-0000-000000000002")
    (property "Reference" "R1" (at 102 100 0))
    (property "Value" "10k" (at 102 102 0))
  )
  (label "VOUT" (at 100 120 0))
)"#;

        let res = parse_kicad_sch(content);
        assert!(res.is_ok(), "Failed to parse kicad_sch: {:?}", res.err());
        let sch = res.unwrap();
        assert_eq!(sch.defs.len(), 1);
        let sheet = &sch.defs[0];
        assert_eq!(sheet.symbols.len(), 1);
        assert_eq!(sheet.symbols[0].ref_default, "R1");
        assert_eq!(sheet.symbols[0].value, "10k");
        assert_eq!(sheet.wires.len(), 1);
        assert_eq!(sheet.labels.len(), 1);
        assert_eq!(sheet.labels[0].text, "VOUT");


        // Solved nets
        let net = sch.find_net_by_name("VOUT");
        assert!(net.is_some(), "VOUT net should be found");
        let net = net.unwrap();
        assert_eq!(net.name, "VOUT");
        assert!(!net.members.is_empty(), "Pin 2 of R1 should be connected to VOUT");
        assert_eq!(net.members[0].ref_des, "R1");
    }
}
