use bvc_core::parser::registry::ParserRegistry;
use bvc_core::model::Side;

fn make_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut zip_data = Vec::new();
    let mut cd_records = Vec::new();

    for (name, data) in entries {
        let local_offset = zip_data.len();
        let name_bytes = name.as_bytes();

        let mut hasher = crc32fast::Hasher::new();
        hasher.update(data);
        let crc = hasher.finalize();

        zip_data.extend_from_slice(b"PK\x03\x04");
        zip_data.extend_from_slice(&20u16.to_le_bytes());
        zip_data.extend_from_slice(&0u16.to_le_bytes());
        zip_data.extend_from_slice(&0u16.to_le_bytes());
        zip_data.extend_from_slice(&0u16.to_le_bytes());
        zip_data.extend_from_slice(&0u16.to_le_bytes());
        zip_data.extend_from_slice(&crc.to_le_bytes());
        zip_data.extend_from_slice(&(data.len() as u32).to_le_bytes());
        zip_data.extend_from_slice(&(data.len() as u32).to_le_bytes());
        zip_data.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
        zip_data.extend_from_slice(&0u16.to_le_bytes());
        zip_data.extend_from_slice(name_bytes);
        zip_data.extend_from_slice(data);

        let mut cd = Vec::new();
        cd.extend_from_slice(b"PK\x01\x02");
        cd.extend_from_slice(&20u16.to_le_bytes());
        cd.extend_from_slice(&20u16.to_le_bytes());
        cd.extend_from_slice(&0u16.to_le_bytes());
        cd.extend_from_slice(&0u16.to_le_bytes());
        cd.extend_from_slice(&0u16.to_le_bytes());
        cd.extend_from_slice(&0u16.to_le_bytes());
        cd.extend_from_slice(&crc.to_le_bytes());
        cd.extend_from_slice(&(data.len() as u32).to_le_bytes());
        cd.extend_from_slice(&(data.len() as u32).to_le_bytes());
        cd.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
        cd.extend_from_slice(&0u16.to_le_bytes());
        cd.extend_from_slice(&0u16.to_le_bytes());
        cd.extend_from_slice(&0u16.to_le_bytes());
        cd.extend_from_slice(&0u16.to_le_bytes());
        cd.extend_from_slice(&0u32.to_le_bytes());
        cd.extend_from_slice(&(local_offset as u32).to_le_bytes());
        cd.extend_from_slice(name_bytes);
        cd_records.push(cd);
    }

    let cd_offset = zip_data.len();
    let mut total_cd_len = 0;
    for cd in &cd_records {
        zip_data.extend_from_slice(cd);
        total_cd_len += cd.len();
    }

    zip_data.extend_from_slice(b"PK\x05\x06");
    zip_data.extend_from_slice(&0u16.to_le_bytes());
    zip_data.extend_from_slice(&0u16.to_le_bytes());
    zip_data.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    zip_data.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    zip_data.extend_from_slice(&(total_cd_len as u32).to_le_bytes());
    zip_data.extend_from_slice(&(cd_offset as u32).to_le_bytes());
    zip_data.extend_from_slice(&0u16.to_le_bytes());

    zip_data
}

#[test]
fn test_ipc356_parsing() {
    let ipc356_content = "P  UNITS CUST 0\n\
317VCC              U1    -1    D0250PA01X+010000Y+020000X0500Y0500R000S1\n\
327GND              U1    -2         A01X+011000Y+020000X0500Y0500R000S1\n\
327VCC              R1    -1         A02X+020000Y+020000X0400Y0400R090S1\n\
999\n";

    let registry = ParserRegistry::default();
    let board = registry.parse_file("board.ipc", ipc356_content.as_bytes()).expect("IPC-D-356 parsing failed");

    assert_eq!(board.parts.len(), 2);
    assert_eq!(board.pins.len(), 3);

    let u1 = board.parts.iter().find(|p| p.name == "U1").unwrap();
    assert_eq!(u1.side, Side::Both); // because it has through-hole pin 1

    let r1 = board.parts.iter().find(|p| p.name == "R1").unwrap();
    assert_eq!(r1.side, Side::Bottom); // A02 access
}

#[test]
fn test_ipc2581_parsing() {
    let ipc2581_content = r#"<?xml version="1.0" encoding="utf-8"?>
<IPC-2581 revision="B">
<CadHeader units="MILLIMETER"/>
<Step name="default">
  <Package name="SOIC8">
    <Pin number="1" name="1"/>
    <Xform x="-1.5" y="-1.0"/>
    <Pin number="2" name="2"/>
    <Xform x="-1.5" y="1.0"/>
  </Package>
  <Component refDes="U10" packageRef="SOIC8" layerRef="TOP">
    <Xform x="10.0" y="20.0" rotation="0"/>
  </Component>
  <LogicalNet name="VDD_3V3">
    <PinRef componentRef="U10" pin="1"/>
  </LogicalNet>
  <LogicalNet name="GND">
    <PinRef componentRef="U10" pin="2"/>
  </LogicalNet>
</Step>
</IPC-2581>
"#;

    let registry = ParserRegistry::default();
    let board = registry.parse_file("design.xml", ipc2581_content.as_bytes()).expect("IPC-2581 parsing failed");

    assert_eq!(board.parts.len(), 1);
    assert_eq!(board.parts[0].name, "U10");
    assert_eq!(board.pins.len(), 2);

    let vdd_pins: Vec<_> = board.pins.iter().filter(|p| p.net_name == "VDD_3V3").collect();
    assert_eq!(vdd_pins.len(), 1);
    assert_eq!(vdd_pins[0].name, "1");
}

#[test]
fn test_odbpp_parsing() {
    let matrix_content = "STEP { NAME=default }\nLAYER { NAME=comp_+_top TYPE=COMPONENT }\n";
    let profile_content = "OB 0.0 0.0\nOS 2.0 0.0\nOS 2.0 1.0\nOS 0.0 1.0\n";
    let eda_content = "PKG SOT23\nPIN 1 -0.05 -0.05\nPIN 2 0.05 -0.05\n";
    let comp_top_content = "CMP 1 SOT23 1.0 0.5 0.0 N Q1\n";

    let zip_bytes = make_zip(&[
        ("matrix/matrix", matrix_content.as_bytes()),
        ("steps/default/profile", profile_content.as_bytes()),
        ("steps/default/eda/data", eda_content.as_bytes()),
        ("steps/default/layers/comp_+_top", comp_top_content.as_bytes()),
    ]);

    let registry = ParserRegistry::default();
    let board = registry.parse_file("job.tgz", &zip_bytes).expect("ODB++ parsing failed");

    assert_eq!(board.parts.len(), 1);
    assert_eq!(board.parts[0].name, "Q1");
    assert_eq!(board.pins.len(), 2);
    assert_eq!(board.outline_polygon.len(), 4);
}
