use bvc_core::parser::registry::ParserRegistry;
use bvc_core::model::Side;

#[test]
fn test_altium_ascii_parsing() {
    let altium_ascii = r#"|RECORD=Board|KIND=Protel_Advanced_PCB|
|RECORD=Component|SOURCEDESIGNATOR=U1|PATTERN=SOIC8|LAYER=TopLayer|X=1000mil|Y=2000mil|ROTATION=0|
|RECORD=Component|SOURCEDESIGNATOR=R1|PATTERN=0805|LAYER=BottomLayer|X=3000mil|Y=2000mil|ROTATION=90|
|RECORD=Pad|NAME=1|COMPONENT=U1|NET=VCC|LAYER=1|X=950mil|Y=2000mil|XSIZE=60mil|YSIZE=40mil|
|RECORD=Pad|NAME=2|COMPONENT=U1|NET=GND|LAYER=1|X=1050mil|Y=2000mil|XSIZE=60mil|YSIZE=40mil|
|RECORD=Pad|NAME=1|COMPONENT=R1|NET=VCC|LAYER=32|X=3000mil|Y=2000mil|XSIZE=40mil|YSIZE=40mil|
"#;

    let registry = ParserRegistry::default();
    let board = registry.parse_file("board.PcbDoc", altium_ascii.as_bytes()).expect("Altium parsing failed");

    assert_eq!(board.parts.len(), 2);
    assert_eq!(board.pins.len(), 3);

    let u1 = board.parts.iter().find(|p| p.name == "U1").unwrap();
    assert_eq!(u1.side, Side::Top);

    let r1 = board.parts.iter().find(|p| p.name == "R1").unwrap();
    assert_eq!(r1.side, Side::Bottom);

    let vcc_pins: Vec<_> = board.pins.iter().filter(|p| p.net_name == "VCC").collect();
    assert_eq!(vcc_pins.len(), 2);
}

#[test]
fn test_allegro_binary_magic_sniffing_and_parsing() {
    let mut allegro_bytes = vec![0u8; 512];
    // Layout 17.2 magic (0x140400)
    allegro_bytes[0] = 0x00;
    allegro_bytes[1] = 0x04;
    allegro_bytes[2] = 0x14;
    allegro_bytes[3] = 0x00;

    // Embed component strings
    let u1_str = b"\0U1\0R10\0C5\0";
    allegro_bytes[64..64 + u1_str.len()].copy_from_slice(u1_str);

    let registry = ParserRegistry::default();
    let board = registry.parse_file("allegro_design.brd", &allegro_bytes).expect("Allegro parsing failed");

    assert_eq!(board.parts.len(), 3);
    assert_eq!(board.parts[0].name, "U1");
    assert_eq!(board.parts[1].name, "R10");
    assert_eq!(board.parts[2].name, "C5");
}

#[test]
fn test_jet4_bv_binary_magic_and_parsing() {
    let mut jet_bytes = vec![0u8; 1024];
    // Magic: [0, 1, 0, 0] + "Standard Jet DB\0"
    jet_bytes[0] = 0x00;
    jet_bytes[1] = 0x01;
    jet_bytes[2] = 0x00;
    jet_bytes[3] = 0x00;
    jet_bytes[4..20].copy_from_slice(b"Standard Jet DB\0");

    let comp_names = b"\0U100\0R200\0";
    jet_bytes[64..64 + comp_names.len()].copy_from_slice(comp_names);

    let registry = ParserRegistry::default();
    let board = registry.parse_file("board.bv", &jet_bytes).expect("Jet BV parsing failed");

    assert_eq!(board.parts.len(), 2);
    assert_eq!(board.parts[0].name, "U100");
    assert_eq!(board.parts[1].name, "R200");
}
