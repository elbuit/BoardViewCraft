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

fn make_tar_gz(entries: &[(&str, &[u8])]) -> Vec<u8> {
    use std::io::Write;
    use flate2::write::GzEncoder;
    use flate2::Compression;

    let mut tar_bytes = Vec::new();
    for (name, data) in entries {
        let mut header = [0u8; 512];
        let name_bytes = name.as_bytes();
        let copy_len = name_bytes.len().min(100);
        header[..copy_len].copy_from_slice(&name_bytes[..copy_len]);

        // File mode: 0644
        header[100..107].copy_from_slice(b"0000644");
        // Size in octal ASCII
        let size_octal = format!("{:011o}", data.len());
        header[124..135].copy_from_slice(size_octal.as_bytes());
        // Typeflag '0'
        header[156] = b'0';
        // Magic 'ustar\0'
        header[257..263].copy_from_slice(b"ustar\0");
        header[263..265].copy_from_slice(b"00");

        // Checksum calculation (with checksum field treated as 8 spaces)
        header[148..156].fill(b' ');
        let chksum: u32 = header.iter().map(|&b| b as u32).sum();
        let chksum_octal = format!("{:06o}\0 ", chksum);
        header[148..156].copy_from_slice(chksum_octal.as_bytes());

        tar_bytes.extend_from_slice(&header);
        tar_bytes.extend_from_slice(data);
        let pad = (512 - (data.len() % 512)) % 512;
        if pad > 0 {
            tar_bytes.extend_from_slice(&vec![0u8; pad]);
        }
    }
    // End of archive: 1024 zero bytes
    tar_bytes.extend_from_slice(&[0u8; 1024]);

    // Compress with Gzip
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(&tar_bytes).unwrap();
    encoder.finish().unwrap()
}

#[test]
fn test_odbpp_tar_gz_archive_parsing() {
    let matrix_content = "STEP { NAME=default }\nLAYER { NAME=comp_+_top TYPE=COMPONENT }\n";
    let profile_content = "OB 0.0 0.0\nOS 3.0 0.0\nOS 3.0 2.0\nOS 0.0 2.0\n";
    let eda_content = "PKG DFN8\nPIN 1 -0.1 -0.1\nPIN 2 0.1 -0.1\n";
    let comp_top_content = "CMP 1 DFN8 1.5 1.0 0.0 N U1\n";

    let tgz_bytes = make_tar_gz(&[
        ("matrix/matrix", matrix_content.as_bytes()),
        ("steps/default/profile", profile_content.as_bytes()),
        ("steps/default/eda/data", eda_content.as_bytes()),
        ("steps/default/layers/comp_+_top", comp_top_content.as_bytes()),
    ]);

    let registry = ParserRegistry::default();
    let board = registry.parse_file("design.tar.gz", &tgz_bytes).expect("ODB++ tar.gz parsing failed");

    assert_eq!(board.parts.len(), 1);
    assert_eq!(board.parts[0].name, "U1");
    assert_eq!(board.pins.len(), 2);
    assert_eq!(board.outline_polygon.len(), 4);
}

