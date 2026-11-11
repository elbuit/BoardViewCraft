use bvc_core::parser::registry::ParserRegistry;

#[test]
fn test_gerber_rejection_diagnostic() {
    let gerber_sample = b"%FSLAX24Y24*%\n%MOIN*%\n%ADD10C,0.010*%\nD10*\nX001000Y002000D03*\nM02*";
    let registry = ParserRegistry::default();
    let err = registry.parse_file("layer.gbr", gerber_sample).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("Gerber RS-274X detected"), "Expected Gerber error, got: {}", msg);
}

#[test]
fn test_pads_binary_rejection_diagnostic() {
    let mut pads_sample = vec![0u8; 64];
    pads_sample[0] = 0x00;
    pads_sample[1] = 0xff;
    pads_sample[2] = 0x26;
    pads_sample[3] = 0x20;

    let registry = ParserRegistry::default();
    let err = registry.parse_file("design.pcb", &pads_sample).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("PADS native SDB database detected"), "Expected PADS error, got: {}", msg);
}

#[test]
fn test_vs2_rejection_diagnostic() {
    let vs2_sample = b"$VS2\nCODE 100\nPLN 1\nCENTER POS 100 200\n";
    let registry = ParserRegistry::default();
    let err = registry.parse_file("assembly.lst", vs2_sample).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("CASTw VS2 assembly listing detected"), "Expected VS2 error, got: {}", msg);
}

#[test]
fn test_brd_v1_rejection_diagnostic() {
    let mut brd_sample = vec![0u8; 32];
    brd_sample[0..8].copy_from_slice(b"BRD_V1.0");

    let registry = ParserRegistry::default();
    let err = registry.parse_file("protected.brd", &brd_sample).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("BRD_V1.0 encoded boardview detected"), "Expected BRD_V1 error, got: {}", msg);
}

#[test]
fn test_xzz_ps5_sub_pcb_parsing() {
    let sub_pcb_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("tmp/PS5-EDM-010 SUB PCB layer.pcb");

    if !sub_pcb_path.exists() {
        return;
    }

    let data = std::fs::read(&sub_pcb_path).expect("read sub pcb");
    let registry = ParserRegistry::default();
    let board = registry.parse_file("PS5-EDM-010 SUB PCB layer.pcb", &data).expect("parse xzz sub pcb");

    assert_eq!(board.parts.len(), 208);
    let top_parts = board.parts.iter().filter(|p| p.side == bvc_core::Side::Top).count();
    let bot_parts = board.parts.iter().filter(|p| p.side == bvc_core::Side::Bottom).count();
    assert_eq!(top_parts, 110);
    assert_eq!(bot_parts, 98);

    assert!(!board.outline_polygon.is_empty(), "Outline polygon must not be empty");
    assert!(board.outline_polygon.len() >= 4, "Outline polygon must have at least 4 vertices");
    assert!(!board.pins.is_empty(), "Pins must not be empty");
    assert!(!board.vias.is_empty(), "Vias must not be empty");

    let parts_with_footprint = board.parts.iter().filter(|p| !p.mfg_code.is_empty()).count();
    assert!(parts_with_footprint > 0, "Expected parts to have footprint mfg_code");

    let pins_with_size = board.pins.iter().filter(|p| p.width.is_some() && p.height.is_some()).count();
    assert!(pins_with_size > 0, "Expected pins to have physical pad dimensions");

    let bbox = board.bounding_box.expect("Board must have a bounding box");
    assert!(bbox.width() > 0.0 && bbox.height() > 0.0);
    // Board is normalized so origin is at >= 0
    assert!(bbox.min_x >= -1.0 && bbox.min_y >= -1.0);
}

#[test]
fn test_xzz_ps5_main_pcb_parsing() {
    let main_pcb_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("tmp/PS5-EDM-010 100889021 PCB layer.pcb");

    if !main_pcb_path.exists() {
        return;
    }

    let data = std::fs::read(&main_pcb_path).expect("read main pcb");
    let registry = ParserRegistry::default();
    let board = registry.parse_file("PS5-EDM-010 100889021 PCB layer.pcb", &data).expect("parse xzz main pcb");

    assert_eq!(board.parts.len(), 5950);
    let top_parts = board.parts.iter().filter(|p| p.side == bvc_core::Side::Top).count();
    let bot_parts = board.parts.iter().filter(|p| p.side == bvc_core::Side::Bottom).count();
    assert!(top_parts > 1000, "Must have top parts, got: {}", top_parts);
    assert!(bot_parts > 1000, "Must have bottom parts, got: {}", bot_parts);

    assert!(!board.outline_polygon.is_empty(), "Outline polygon must not be empty");
    assert!(board.outline_polygon.len() >= 500, "Main board outline has 600+ points, got: {}", board.outline_polygon.len());
    assert!(board.pins.len() > 10000, "Pins count: {}", board.pins.len());
    assert!(board.vias.len() > 1000, "Vias count: {}", board.vias.len());

    let parts_with_footprint = board.parts.iter().filter(|p| !p.mfg_code.is_empty()).count();
    assert!(parts_with_footprint > 5000, "Main board parts with footprint: {}", parts_with_footprint);
    let pins_with_size = board.pins.iter().filter(|p| p.width.is_some() && p.height.is_some()).count();
    assert!(pins_with_size > 10000, "Main board pins with physical size: {}", pins_with_size);

    let bbox = board.bounding_box.expect("Board must have a bounding box");
    assert!(bbox.width() > 0.0 && bbox.height() > 0.0);
    assert!(bbox.min_x >= -1.0 && bbox.min_y >= -1.0);
}
