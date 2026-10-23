//! Comprehensive integration and unit tests for multi-format boardview support.

use bvc_core::parser::registry::ParserRegistry;
use bvc_core::parser::brd::BrdParser;
use bvc_core::parser::tvw::TvwParser;
use bvc_core::parser::bdv::BdvParser;
use bvc_core::parser::xzz::XzzParser;
use bvc_core::model::Side;
use std::fs;
use std::path::Path;

const FIXTURES_DIR: &str = "/media/no_raid/devel/boarview/wrench-board/tests/board/fixtures";

#[test]
fn test_fixture_minimal_brd() {
    let path = Path::new(FIXTURES_DIR).join("minimal.brd");
    if let Ok(data) = fs::read(&path) {
        let registry = ParserRegistry::default();
        let board = registry.parse_file("minimal.brd", &data).expect("parse minimal.brd");
        assert!(!board.parts.is_empty());
        assert!(!board.pins.is_empty());
    }
}

#[test]
fn test_fixture_bilayer_minimal_brd() {
    let path = Path::new(FIXTURES_DIR).join("bilayer_minimal.brd");
    if let Ok(data) = fs::read(&path) {
        let registry = ParserRegistry::default();
        let board = registry.parse_file("bilayer_minimal.brd", &data).expect("parse bilayer_minimal.brd");
        assert!(!board.parts.is_empty());
        assert!(!board.pins.is_empty());
    }
}

#[test]
fn test_fixture_brd2_form_cad() {
    let path = Path::new(FIXTURES_DIR).join("brd2_form.cad");
    if let Ok(data) = fs::read(&path) {
        let registry = ParserRegistry::default();
        let board = registry.parse_file("brd2_form.cad", &data).expect("parse brd2_form.cad");
        assert!(!board.parts.is_empty());
        assert!(!board.pins.is_empty());
    }
}

#[test]
fn test_fixture_brd_subst_min() {
    let path = Path::new(FIXTURES_DIR).join("brd_subst_min.brd");
    if let Ok(data) = fs::read(&path) {
        let registry = ParserRegistry::default();
        let board = registry.parse_file("brd_subst_min.brd", &data).expect("parse brd_subst_min.brd");
        assert!(!board.parts.is_empty());
        assert!(!board.pins.is_empty());
    }
}

#[test]
fn test_fixture_minimal_bdv_both_variants() {
    let registry = ParserRegistry::default();

    // 1. Per-byte historical variant
    let path_byte = Path::new(FIXTURES_DIR).join("minimal.bdv");
    if let Ok(data) = fs::read(&path_byte) {
        let board = registry.parse_file("minimal.bdv", &data).expect("parse minimal.bdv");
        assert!(!board.parts.is_empty());
        assert!(!board.pins.is_empty());
    }

    // 2. Per-line field variant
    let path_real = Path::new(FIXTURES_DIR).join("bdv_real_min.bdv");
    if let Ok(data) = fs::read(&path_real) {
        let board = registry.parse_file("bdv_real_min.bdv", &data).expect("parse bdv_real_min.bdv");
        assert!(!board.parts.is_empty());
        assert!(!board.pins.is_empty());
    }
}

#[test]
fn test_fixture_minimal_tvw() {
    let path = Path::new(FIXTURES_DIR).join("minimal.tvw");
    if let Ok(data) = fs::read(&path) {
        let registry = ParserRegistry::default();
        let board = registry.parse_file("minimal.tvw", &data).expect("parse minimal.tvw");
        assert!(!board.parts.is_empty());
        assert!(!board.pins.is_empty());
    }
}

#[test]
fn test_fixture_dialects() {
    let registry = ParserRegistry::default();

    // minimal.bv
    let path_bv = Path::new(FIXTURES_DIR).join("minimal.bv");
    if let Ok(data) = fs::read(&path_bv) {
        let board = registry.parse_file("minimal.bv", &data).expect("parse minimal.bv");
        assert!(!board.parts.is_empty());
    }

    // minimal.cad
    let path_cad = Path::new(FIXTURES_DIR).join("minimal.cad");
    if let Ok(data) = fs::read(&path_cad) {
        let board = registry.parse_file("minimal.cad", &data).expect("parse minimal.cad");
        assert!(!board.parts.is_empty());
    }

    // minimal.cst
    let path_cst = Path::new(FIXTURES_DIR).join("minimal.cst");
    if let Ok(data) = fs::read(&path_cst) {
        let board = registry.parse_file("minimal.cst", &data).expect("parse minimal.cst");
        assert!(!board.parts.is_empty());
    }

    // minimal.f2b
    let path_f2b = Path::new(FIXTURES_DIR).join("minimal.f2b");
    if let Ok(data) = fs::read(&path_f2b) {
        let board = registry.parse_file("minimal.f2b", &data).expect("parse minimal.f2b");
        assert!(!board.parts.is_empty());
    }

    // minimal.gr
    let path_gr = Path::new(FIXTURES_DIR).join("minimal.gr");
    if let Ok(data) = fs::read(&path_gr) {
        let board = registry.parse_file("minimal.gr", &data).expect("parse minimal.gr");
        assert!(!board.parts.is_empty());
    }

    // tsict_combined.asc
    let path_asc = Path::new(FIXTURES_DIR).join("tsict_combined.asc");
    if let Ok(data) = fs::read(&path_asc) {
        let board = registry.parse_file("tsict_combined.asc", &data).expect("parse tsict_combined.asc");
        assert!(!board.parts.is_empty());
    }
}

#[test]
fn test_synthetic_subst_brd_roundtrip() {
    let plain = "str_length: 100\nvar_data: 4 1 2 1\nFormat:\n0 0\n100 0\n100 100\n0 100\nParts:\nU1 5 2\nPins:\n10 10 0 1 VCC\n20 20 0 1 GND\nNails:\n1 10 10 1 VCC\n";
    // Encode with bitwise closed form inverse
    let mut encoded = vec![0x23, 0xe2, 0x63, 0x28];
    for &b in plain.as_bytes() {
        if b == b'\r' || b == b'\n' || b == 0 {
            encoded.push(b);
        } else {
            let inv = !b;
            let original = ((inv << 6) & 0xC0) | ((inv >> 2) & 0x3F);
            encoded.push(original);
        }
    }

    let decoded = BrdParser::decode_subst(&encoded);
    assert_eq!(&decoded[4..], plain.as_bytes());

    let registry = ParserRegistry::default();
    let board = registry.parse_file("test.brd", &encoded).expect("parse subst encoded brd");
    assert_eq!(board.parts.len(), 1);
    assert_eq!(board.parts[0].name, "U1");
    assert_eq!(board.pins.len(), 2);
    assert_eq!(board.pins[0].net_name, "VCC");
}

#[test]
fn test_synthetic_tvw_rotation_cipher() {
    let plain = "var_data: 0 1 1 0\nParts:\nU1 5 1\nPins:\n10 10 0 1 VCC\n";
    // Encode with +3 digit, +10 alpha
    let mut encoded = Vec::new();
    for &b in plain.as_bytes() {
        if (b'0'..=b'9').contains(&b) {
            encoded.push(((b - b'0' + 3) % 10) + b'0');
        } else if (b'a'..=b'z').contains(&b) {
            encoded.push(((b - b'a' + 10) % 26) + b'a');
        } else if (b'A'..=b'Z').contains(&b) {
            encoded.push(((b - b'A' + 10) % 26) + b'A');
        } else {
            encoded.push(b);
        }
    }

    let deobf = TvwParser::deobfuscate_ascii(&encoded);
    assert_eq!(String::from_utf8_lossy(&deobf), plain);

    let registry = ParserRegistry::default();
    let board = registry.parse_file("board.tvw", &encoded).expect("parse tvw ascii");
    assert_eq!(board.parts.len(), 1);
    assert_eq!(board.pins.len(), 1);
}

#[test]
fn test_synthetic_bdv_schedules() {
    // 1. Per-byte schedule
    let plain = "var_data: 0 1 1 0\nParts:\nU1 5 1\nPins:\n10 10 0 1 VCC\n";
    let mut enc_byte = Vec::new();
    let mut key: i32 = 160;
    for &b in plain.as_bytes() {
        if b == 10 || b == 13 {
            enc_byte.push(b);
            continue;
        }
        enc_byte.push(((key - (b as i32)) & 0xFF) as u8);
        if key >= 285 { key = 159; } else { key += 1; }
    }
    let dec_byte = BdvParser::decode_per_byte(&enc_byte);
    assert_eq!(String::from_utf8_lossy(&dec_byte), plain);

    // 2. Per-line field schedule starting with dd:
    let field_plain = "<<format.asc>>\n1.0 2.0 0.0\n<<pins.asc>>\nPart U1 (T)\n1 1 1.0 2.0 1 VCC\n";
    let mut enc_line = Vec::new();
    let mut lkey: i32 = 160;
    for &b in field_plain.as_bytes() {
        if b == 10 || b == 13 {
            enc_line.push(b);
            if b == 10 {
                lkey += 1;
                if lkey > 285 { lkey = 159; }
            }
            continue;
        }
        enc_line.push(((lkey - (b as i32)) & 0xFF) as u8);
    }
    assert!(enc_line.starts_with(b"dd:"));
    let dec_line = BdvParser::decode_per_line(&enc_line);
    assert_eq!(String::from_utf8_lossy(&dec_line), field_plain);

    let registry = ParserRegistry::default();
    let board = registry.parse_file("test.bdv", &enc_line).expect("parse bdv field variant");
    assert_eq!(board.parts.len(), 1);
    assert_eq!(board.parts[0].name, "U1");
    assert_eq!(board.pins.len(), 1);
    assert_eq!(board.pins[0].net_name, "VCC");
}

#[test]
fn test_synthetic_fz_zlib_decompression() {
    use flate2::write::ZlibEncoder;
    use flate2::Compression;
    use std::io::Write;

    let payload = r#"
UNIT:millimeter
F!REFDES
A!REFDES!INS_CODE!SYM_NAME!SYM_MIRROR!SYM_ROTATE!
S!R1!0!0402!NO!0!
S!C1!0!0402!YES!0!
F!NET_NAME
A!NET_NAME!REFDES!PIN_NUMBER!PIN_NAME!PIN_X!PIN_Y!TEST_POINT!RADIUS!
S!VCC!R1!0!1!10.0!20.0!!0.5!
S!GND!C1!0!1!15.0!25.0!!0.5!
F!TESTVIA
A!TESTVIA!NET_NAME!REFDES!PIN_NUMBER!PIN_NAME!VIA_X!VIA_Y!TEST_POINT!RADIUS!
S!TESTVIA!VCC!TP1!0!1!12.0!22.0!!0.4!
"#;

    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(payload.as_bytes()).unwrap();
    let compressed = encoder.finish().unwrap();

    let mut data = vec![0u8; 4]; // 4-byte decompressed size placeholder
    data.extend_from_slice(&compressed);

    let registry = ParserRegistry::default();
    let board = registry.parse_file("board.fz", &data).expect("parse fz_zlib");
    assert_eq!(board.parts.len(), 2);
    assert_eq!(board.parts[0].name, "R1");
    assert_eq!(board.parts[0].side, Side::Top);
    assert_eq!(board.parts[1].name, "C1");
    assert_eq!(board.parts[1].side, Side::Bottom);
    assert_eq!(board.pins.len(), 2);
    assert_eq!(board.vias.len(), 1);
}

#[test]
fn test_des_implementation_vectors() {
    // Known DES test vector:
    // Ronald L. Rivest test vector:
    // input = 0x9474B8E8C73BCA7D
    // key = 0x9474B8E8C73BCA7D
    // des(input, key, encrypt) -> 0x8DA744E0C94E5E17
    let input = 0x9474B8E8C73BCA7D_u64;
    let key = 0x9474B8E8C73BCA7D_u64;
    let enc = XzzParser::des(input, key, false);
    assert_eq!(enc, 0x8DA744E0C94E5E17_u64);
    let dec = XzzParser::des(enc, key, true);
    assert_eq!(dec, input);
}

#[test]
fn test_two_stage_sniffing_priority() {
    let registry = ParserRegistry::default();

    // GenCAD content with ambiguous extension .cad
    const GENCAD_SAMPLE: &str = "$HEADER\nGENCAD 1.4\nUSER \"Test\"\n$ENDHEADER\n$BOARD\n$ENDBOARD\n";
    let board = registry.parse_file("board.cad", GENCAD_SAMPLE.as_bytes()).expect("parse gencad as .cad");
    // GenCAD parser succeeded
    assert_eq!(board.parts.len(), 0);

    // BRD2 content with ambiguous extension .cad
    let brd2_cad = "BRDOUT: 0 100 100\nNETS: 1\n1 VCC\nPARTS: 1\nU1 0 0 10 10 0 1\nPINS: 1\n5 5 1 1\nNAILS: 0\n";
    let board2 = registry.parse_file("board.cad", brd2_cad.as_bytes()).expect("parse brd2 as .cad");
    assert_eq!(board2.parts.len(), 1);
    assert_eq!(board2.pins.len(), 1);
}

#[test]
fn test_real_test1_tvw_production_binary() {
    let path = Path::new("/media/no_raid/devel/boarview/test1.tvw");
    if let Ok(data) = fs::read(&path) {
        let registry = ParserRegistry::default();
        let board = registry.parse_file("test1.tvw", &data).expect("parse test1.tvw production binary");
        assert!(board.parts.len() > 1000, "Expected >1000 parts, got {}", board.parts.len());
        assert!(board.pins.len() > 1000, "Expected >1000 pins, got {}", board.pins.len());
        assert!(!board.nets.is_empty(), "Expected non-empty net list");
    }
}

