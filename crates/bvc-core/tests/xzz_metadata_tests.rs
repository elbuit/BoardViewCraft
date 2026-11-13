//! Repair metadata extracted from XZZ boards (part values, v6 readings).

use bvc_core::parser::registry::ParserRegistry;
use bvc_core::Board;
use std::path::{Path, PathBuf};

fn repo_tmp(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap().join("tmp").join(name)
}

fn load(path: &Path) -> Option<Board> {
    if !path.exists() {
        return None;
    }
    let data = std::fs::read(path).expect("read board file");
    let file_name = path.file_name().unwrap().to_string_lossy().to_string();
    Some(ParserRegistry::default().parse_file(&file_name, &data).expect("parse xzz board"))
}

#[test]
fn xzz_ps5_part_values_come_from_second_label() {
    let Some(board) = load(&repo_tmp("PS5-EDM-010 100889021 PCB layer.pcb")) else {
        return;
    };
    assert_eq!(board.metadata.part_value("N11171"), Some("560"));
    assert_eq!(board.metadata.part_value("n11172"), Some("R22"));
}

#[test]
fn tvw_component_value_is_exposed_as_part_value() {
    let Some(board) = load(&repo_tmp("test1.tvw")) else {
        return;
    };
    assert_eq!(board.metadata.part_value("R1278"), Some("1.8ohm/x"));
}

const HAC_DIR: &str = "/home/toni/Baixades/Game Consoles/Nintendo/SWITCH NS-HAC-CPU-20";

#[test]
fn xzz_v6_resistance_section_yields_pin_readings() {
    let Some(board) = load(&Path::new(HAC_DIR).join("HAC-CPU-20 YiDianTong(Diode value).pcb")) else {
        return;
    };
    let meta = &board.metadata;
    assert_eq!(meta.pin_reading("N409", "33"), Some("318"));
    assert_eq!(meta.pin_reading("N397", "2"), Some("713"));
    assert_eq!(meta.pin_reading("N407", "4"), Some("OL"));
    assert!(meta.pin_reading_count() > 1000, "only {} pin readings", meta.pin_reading_count());

    // Readings must point at pins that exist on the parsed board.
    let n409 = &board.parts[board.part_by_name["N409"]];
    assert!(n409.pin_indices.iter().any(|&i| board.pins[i].name == "33"));
}

#[test]
fn xzz_v6_readings_on_plain_pcb_layer_file() {
    let Some(board) = load(&Path::new(HAC_DIR).join("HAC-CPU-20 PCB layer.pcb")) else {
        return;
    };
    assert_eq!(board.metadata.pin_reading("N1214", "1"), Some("497"));
    assert_eq!(board.metadata.pin_reading("N1215", "1"), Some("557"));
}
