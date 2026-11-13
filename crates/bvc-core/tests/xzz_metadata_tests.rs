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
