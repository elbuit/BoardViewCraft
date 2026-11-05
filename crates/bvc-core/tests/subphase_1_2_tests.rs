use bvc_core::parser::registry::ParserRegistry;
use bvc_core::parser::zip_container::ZipArchiveReader;

fn make_test_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut zip_data = Vec::new();
    let mut cd_records = Vec::new();

    for (name, data) in entries {
        let local_offset = zip_data.len();
        let name_bytes = name.as_bytes();

        let mut hasher = crc32fast::Hasher::new();
        hasher.update(data);
        let crc = hasher.finalize();

        // Local file header: 30 bytes + name
        zip_data.extend_from_slice(b"PK\x03\x04");
        zip_data.extend_from_slice(&20u16.to_le_bytes()); // version
        zip_data.extend_from_slice(&0u16.to_le_bytes()); // flags
        zip_data.extend_from_slice(&0u16.to_le_bytes()); // method (0 = stored)
        zip_data.extend_from_slice(&0u16.to_le_bytes()); // time
        zip_data.extend_from_slice(&0u16.to_le_bytes()); // date
        zip_data.extend_from_slice(&crc.to_le_bytes());
        zip_data.extend_from_slice(&(data.len() as u32).to_le_bytes()); // comp
        zip_data.extend_from_slice(&(data.len() as u32).to_le_bytes()); // uncomp
        zip_data.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
        zip_data.extend_from_slice(&0u16.to_le_bytes()); // extra len
        zip_data.extend_from_slice(name_bytes);
        zip_data.extend_from_slice(data);

        // Prepare central directory entry: 46 bytes + name
        let mut cd = Vec::new();
        cd.extend_from_slice(b"PK\x01\x02");
        cd.extend_from_slice(&20u16.to_le_bytes()); // ver made by
        cd.extend_from_slice(&20u16.to_le_bytes()); // ver needed
        cd.extend_from_slice(&0u16.to_le_bytes()); // flags
        cd.extend_from_slice(&0u16.to_le_bytes()); // method
        cd.extend_from_slice(&0u16.to_le_bytes()); // time
        cd.extend_from_slice(&0u16.to_le_bytes()); // date
        cd.extend_from_slice(&crc.to_le_bytes());
        cd.extend_from_slice(&(data.len() as u32).to_le_bytes());
        cd.extend_from_slice(&(data.len() as u32).to_le_bytes());
        cd.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
        cd.extend_from_slice(&0u16.to_le_bytes()); // extra
        cd.extend_from_slice(&0u16.to_le_bytes()); // comment
        cd.extend_from_slice(&0u16.to_le_bytes()); // disk
        cd.extend_from_slice(&0u16.to_le_bytes()); // int attr
        cd.extend_from_slice(&0u32.to_le_bytes()); // ext attr
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

    // End of central directory record (22 bytes)
    zip_data.extend_from_slice(b"PK\x05\x06");
    zip_data.extend_from_slice(&0u16.to_le_bytes()); // disk no
    zip_data.extend_from_slice(&0u16.to_le_bytes()); // cd disk
    zip_data.extend_from_slice(&(entries.len() as u16).to_le_bytes()); // entries this disk
    zip_data.extend_from_slice(&(entries.len() as u16).to_le_bytes()); // total entries
    zip_data.extend_from_slice(&(total_cd_len as u32).to_le_bytes());
    zip_data.extend_from_slice(&(cd_offset as u32).to_le_bytes());
    zip_data.extend_from_slice(&0u16.to_le_bytes()); // comment len

    zip_data
}

#[test]
fn test_zip_archive_reader_direct() {
    let sample_cad = b"COMP U1 1 0 0 10.0 20.0 1 0\nC_PIN U1-1 10.0 20.0 0 0 0 0 /VCC\n";
    let zip_bytes = make_test_zip(&[
        ("readme.txt", b"Companion readme file"),
        ("board.cad", sample_cad),
    ]);

    assert!(ZipArchiveReader::is_zip(&zip_bytes));
    let mut reader = ZipArchiveReader::open(&zip_bytes).expect("Failed to open valid ZIP");
    assert_eq!(reader.entries.len(), 2);

    let board_entry = reader.entries.iter().find(|e| e.name == "board.cad").cloned().expect("board.cad missing");
    let extracted = reader.extract(&board_entry).expect("Extraction failed");
    assert_eq!(extracted, sample_cad);
}

#[test]
fn test_registry_transparent_zip_loading() {
    let sample_bv2 = br#"
#Layout#
X,Y,R,Group
0,0,0,1
10,0,0,1
10,10,0,1
0,10,0,1
#Nail#
Nail,X,Y,Type,Grid,TB,Net,NetName
#Pin#
Part,TB,Pin,Name,X,Y,Layer,Netname
U1,(T),1,1,2.0,2.0,1,GND
"#;

    let zip_bytes = make_test_zip(&[
        ("notes.txt", b"Repair notes"),
        ("laptop_motherboard.bv2", sample_bv2),
    ]);

    let registry = ParserRegistry::default();
    let board = registry.parse_file("bundle.zip", &zip_bytes).expect("ZIP parsing failed");

    assert_eq!(board.parts.len(), 1);
    assert_eq!(board.parts[0].name, "U1");
    assert_eq!(board.pins.len(), 1);
    assert_eq!(board.pins[0].net_name, "GND");
}
