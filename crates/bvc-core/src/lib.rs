//! BoardViewCraft core library.

pub mod board;
pub mod model;
pub mod parser;
pub mod spatial;

pub use board::Board;
pub use model::{Net, NetMembership, Part, PartType, Pin, Point, Rect, Side, Via};
pub use parser::{cad::CadParser, gencad::GenCadParser, registry::ParserRegistry, FormatParser, ParseError};
pub use spatial::{GridCell, SpatialGrid};

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_GENCAD: &str = r#"
$HEADER
GENCAD 1.4
USER "Test GenCAD"
UNITS USER 1000
ORIGIN 0 0
$ENDHEADER

$BOARD
LINE 0 0 100 0
LINE 100 0 100 100
LINE 100 100 0 100
LINE 0 100 0 0
$ENDBOARD

$PADSTACKS
PADSTACK PS_SMD 0.000
PAD PAD1 TOP 0 0
PADSTACK PS_TH 30.000
PAD PAD2 ALL 0 0
$ENDPADSTACKS

$SHAPES
SHAPE SHP_IC
PIN 1 PS_SMD -10.0 -10.0 TOP 0 0
PIN 2 PS_SMD 10.0 -10.0 TOP 0 0
PIN 3 PS_TH 10.0 10.0 TOP 0 0
PIN 4 PS_SMD -10.0 10.0 TOP 0 0
$ENDSHAPES

$COMPONENTS
COMPONENT U1
PLACE 50.0 50.0
LAYER TOP
ROTATION 90.0
SHAPE SHP_IC 0 0
DEVICE TestChip
$ENDCOMPONENTS

$SIGNALS
SIGNAL VCC
NODE U1 1
SIGNAL GND
NODE U1 2
$ENDSIGNALS

$ROUTES
ROUTE GND
VIA PS_TH 40.0 40.0 ALL -2 via1
$ENDROUTES
"#;

    const SAMPLE_CAD: &str = r#"
###Panel Added
COMP U1 1 0 0 10.0 20.0 1 0
COMP R1 2 0 0 15.0 20.0 1 0
COMP C1 3 0 0 12.0 25.0 2 0

C_PIN U1-1 10.0 20.0 0 0 0 0 /VCC
C_PIN U1-2 10.0 21.0 0 0 0 0 /GND
C_PIN U1-3 10.5 20.5 0 0 0 0 /SIG_OUT

C_PIN R1-1 15.0 20.0 0 0 0 0 /SIG_OUT
C_PIN R1-2 16.0 20.0 0 0 0 0 /NET_R

C_PIN C1-1 12.0 25.0 0 0 0 0 /VCC
C_PIN C1-2 13.0 25.0 0 0 0 0 /GND

NET /SIG_OUT
N_VIA 11.0 20.5 0 1 0
N_VIA 14.5 20.5 0 2 0

NET /VCC
N_VIA 10.0 22.0 0 1 0
"#;

    #[test]
    fn test_cad_parsing() {
        let registry = ParserRegistry::default();
        let board = registry
            .parse_file("test.cad", SAMPLE_CAD.as_bytes())
            .expect("parse sample cad");

        assert_eq!(board.parts.len(), 3);
        assert_eq!(board.parts[0].name, "U1");
        assert_eq!(board.parts[0].side, Side::Top);
        assert_eq!(board.parts[1].name, "R1");
        assert_eq!(board.parts[1].side, Side::Top);
        assert_eq!(board.parts[2].name, "C1");
        assert_eq!(board.parts[2].side, Side::Bottom);

        assert_eq!(board.pins.len(), 7);
        // Multiplier is 1000.0: 10.0 -> 10000.0
        assert_eq!(board.pins[0].pos.x, 10000.0);
        assert_eq!(board.pins[0].pos.y, 20000.0);
        assert_eq!(board.pins[0].net_name, "VCC");
        assert_eq!(board.pins[0].side, Side::Top);

        // Pin 6 is C1-2 on bottom side
        assert_eq!(board.pins[6].side, Side::Bottom);
        assert_eq!(board.pins[6].net_name, "GND");

        // Vias check
        assert_eq!(board.vias.len(), 3);
        assert_eq!(board.vias[0].net_name, "SIG_OUT");
        assert_eq!(board.vias[0].side, Side::Top);
        assert_eq!(board.vias[1].net_name, "SIG_OUT");
        assert_eq!(board.vias[1].side, Side::Bottom);
        assert_eq!(board.vias[2].net_name, "VCC");

        // Net indexing check
        let sig_out = board.get_net("SIG_OUT").expect("SIG_OUT exists");
        assert_eq!(sig_out.pin_indices.len(), 2); // U1-3 and R1-1
        assert_eq!(sig_out.via_indices.len(), 2);

        let vcc = board.get_net("VCC").expect("VCC exists");
        assert_eq!(vcc.pin_indices.len(), 2); // U1-1 and C1-1
        assert_eq!(vcc.via_indices.len(), 1);

        // Outline generation check
        assert!(!board.outline_polygon.is_empty());
        let min_x = board.pins.iter().map(|p| p.pos.x).fold(f64::INFINITY, f64::min) - 20.0;
        let max_x = board.pins.iter().map(|p| p.pos.x).fold(f64::NEG_INFINITY, f64::max) + 20.0;
        assert_eq!(board.outline_polygon[0].x, min_x);
        assert_eq!(board.outline_polygon[1].x, max_x);

        // Part lookup check
        let part_u1 = board.find_part_by_name("u1").expect("find u1");
        assert_eq!(part_u1.name, "U1");
        assert_eq!(part_u1.pin_indices.len(), 3);
    }

    #[test]
    fn test_spatial_lookups() {
        let registry = ParserRegistry::default();
        let board = registry
            .parse_file("test.cad", SAMPLE_CAD.as_bytes())
            .expect("parse sample cad");

        // Pin at 10000, 20000 on Top
        let found = board.find_pin_near(Point::new(10002.0, 19998.0), Side::Top, 15.0);
        assert_eq!(found, Some(0));

        // Invisible on bottom view
        let not_found = board.find_pin_near(Point::new(10002.0, 19998.0), Side::Bottom, 15.0);
        assert_eq!(not_found, None);

        // Via at 11000, 20500 on Top
        let via_found = board.find_via_near(Point::new(11001.0, 20501.0), Side::Top, 10.0);
        assert_eq!(via_found, Some(0));
    }

    #[test]
    fn test_gencad_parsing_sample() {
        let registry = ParserRegistry::default();
        let board = registry
            .parse_file("test.cad", SAMPLE_GENCAD.as_bytes())
            .expect("parse sample gencad");

        assert_eq!(board.parts.len(), 1);
        let part = &board.parts[0];
        assert_eq!(part.name, "U1");
        // U1 has a ThroughHole pin (PS_TH with drill_size 30.0 > 0), so its part_type is ThroughHole
        // and its mounting_side is Side::Both (matching OpenBoardView).
        assert_eq!(part.part_type, PartType::ThroughHole);
        assert_eq!(part.side, Side::Both);

        // 4 pins
        assert_eq!(board.pins.len(), 4);
        // Pin 1: rot 90 around (50, 50).
        // Original rel pos: (-10, -10). Rotated 90 deg: x' = -10*cos(90) - (-10)*sin(90) = 10, y' = -10*sin(90) + (-10)*cos(90) = -10.
        // Pin 1 pos: (50 + 10, 50 - 10) = (60, 40).
        let pin1 = &board.pins[0];
        assert_eq!(pin1.name, "1");
        assert!((pin1.pos.x - 60.0).abs() < 1e-3);
        assert!((pin1.pos.y - 40.0).abs() < 1e-3);
        assert_eq!(pin1.net_name, "VCC");

        // Pin 3 is drilled (padstack drill_size > 0), so it should have side Side::Both
        let pin3 = &board.pins[2];
        assert_eq!(pin3.name, "3");
        assert_eq!(pin3.side, Side::Both);

        // 1 via
        assert_eq!(board.vias.len(), 1);
        assert_eq!(board.vias[0].net_name, "GND");
        assert!((board.vias[0].pos.x - 40.0).abs() < 1e-3);
        assert!((board.vias[0].pos.y - 40.0).abs() < 1e-3);

        // Outline polygon from LINEs: (0, 0) to (100, 100)
        assert_eq!(board.outline_polygon.len(), 5);
        assert_eq!(board.outline_polygon[0], Point::new(0.0, 0.0));
        assert_eq!(board.outline_polygon[2], Point::new(100.0, 100.0));
    }

    #[test]
    fn test_gencad_parsing_real_file() {
        let file_path = "/media/no_raid/devel/boarview/MS-V375 REV 7.0.cad";
        if !std::path::Path::new(file_path).exists() {
            eprintln!("File {file_path} does not exist, skipping real file test.");
            return;
        }

        let data = std::fs::read(file_path).expect("read MS-V375 REV 7.0.cad");
        let registry = ParserRegistry::default();
        let board = registry
            .parse_file("MS-V375 REV 7.0.cad", &data)
            .expect("parse MS-V375 REV 7.0.cad");

        assert!(board.parts.len() > 1000, "Expected > 1000 parts, got {}", board.parts.len());
        assert!(board.pins.len() > 2000, "Expected > 2000 pins, got {}", board.pins.len());
        assert!(board.vias.len() > 5000, "Expected > 5000 vias, got {}", board.vias.len());
        assert!(board.nets.len() > 1000, "Expected > 1000 nets, got {}", board.nets.len());
        assert!(!board.outline_polygon.is_empty(), "Expected non-empty outline");

        let bbox = board.bounding_box.expect("bounding box");
        assert!(bbox.width() > 100.0);
        assert!(bbox.height() > 100.0);

        println!("Net count: {}", board.nets.len());
        if let Some(net) = board.nets.get("IFPC_TXC*") {
            println!("FOUND NET IFPC_TXC* with {} pins, {} vias", net.pin_indices.len(), net.via_indices.len());
            for &pidx in &net.pin_indices {
                let pin = &board.pins[pidx];
                let part = &board.parts[pin.part_id];
                println!("  Pin {}: part={} pin={} pos=({}, {}) side={:?}", pidx, part.name, pin.name, pin.pos.x, pin.pos.y, pin.side);
            }
            for &vidx in &net.via_indices {
                let via = &board.vias[vidx];
                println!("  Via {}: pos=({}, {}) side={:?}", vidx, via.pos.x, via.pos.y, via.side);
            }
        }
    }

    #[test]
    fn test_net_membership_fast_path() {
        let registry = ParserRegistry::default();
        let board = registry
            .parse_file("test.cad", SAMPLE_CAD.as_bytes())
            .expect("parse sample cad");

        let membership_vcc = board.net_membership("VCC");
        assert_eq!(membership_vcc.total_nodes, 3); // 2 pins, 1 via

        // Fast O(1) membership check must match string equality exactly
        for (idx, pin) in board.pins.iter().enumerate() {
            let expected = pin.net_name == "VCC";
            assert_eq!(membership_vcc.contains_pin(idx), expected);
        }

        for (idx, via) in board.vias.iter().enumerate() {
            let expected = via.net_name == "VCC";
            assert_eq!(membership_vcc.contains_via(idx), expected);
        }

        let membership_sig = board.net_membership("SIG_OUT");
        assert_eq!(membership_sig.total_nodes, 4); // 2 pins, 2 vias
        for (idx, pin) in board.pins.iter().enumerate() {
            let expected = pin.net_name == "SIG_OUT";
            assert_eq!(membership_sig.contains_pin(idx), expected);
        }
        for (idx, via) in board.vias.iter().enumerate() {
            let expected = via.net_name == "SIG_OUT";
            assert_eq!(membership_sig.contains_via(idx), expected);
        }

        let membership_empty = board.net_membership("NONEXISTENT");
        assert_eq!(membership_empty.total_nodes, 0);
        assert!(!membership_empty.is_high_density());
        for idx in 0..board.pins.len() {
            assert!(!membership_empty.contains_pin(idx));
        }
        for idx in 0..board.vias.len() {
            assert!(!membership_empty.contains_via(idx));
        }
    }

    #[test]
    fn test_spatial_grid_rebuild_and_query() {
        let registry = ParserRegistry::default();
        let mut board = registry
            .parse_file("test.cad", SAMPLE_CAD.as_bytes())
            .expect("parse sample cad");

        assert!(!board.spatial_grid.is_empty());
        assert_eq!(board.spatial_grid.query_pins_in_rect(&board.bounding_box.unwrap()).len(), board.pins.len());
        assert_eq!(board.spatial_grid.query_vias_in_rect(&board.bounding_box.unwrap()).len(), board.vias.len());

        // Add pins and trigger rebuild_indices
        board.pins.push(Pin {
            id: 99,
            part_id: 0,
            name: "NEW".into(),
            pos: Point::new(50000.0, 50000.0),
            side: Side::Top,
            net_name: "NEW_NET".into(),
            radius: 10.0,
        });
        board.rebuild_indices();

        let new_found = board.spatial_grid.query_pins_in_rect(&Rect {
            min_x: 49000.0,
            min_y: 49000.0,
            max_x: 51000.0,
            max_y: 51000.0,
        });
        assert!(new_found.contains(&7)); // 8th pin (index 7)
    }
}
