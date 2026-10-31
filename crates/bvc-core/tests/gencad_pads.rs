//! GenCAD pad geometry and component outlines on the MS-V375 fixture.

use bvc_core::model::{PadShape, Pin};
use bvc_core::{Board, FormatParser, GenCadParser};
use std::path::PathBuf;

fn fixture() -> Option<Board> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tmp/MS-V375 REV 7.0.cad");
    let Ok(data) = std::fs::read(&path) else {
        eprintln!("skipping: fixture {} not found", path.display());
        return None;
    };
    Some(GenCadParser::new().parse(&data).expect("parse MS-V375 fixture"))
}

fn part_pins<'a>(board: &'a Board, name: &str) -> Vec<&'a Pin> {
    let idx = board.part_by_name[name];
    board.parts[idx].pin_indices.iter().map(|&i| &board.pins[i]).collect()
}

/// Pin positions must not move when pad geometry is added (fingerprint taken on base `0d78513`).
#[test]
fn pin_positions_are_unchanged() {
    let Some(board) = fixture() else { return };
    assert_eq!(board.pins.len(), 7974);
    assert_eq!(board.parts.len(), 1797);
    let sx: f64 = board.pins.iter().map(|p| p.pos.x).sum();
    let sy: f64 = board.pins.iter().map(|p| p.pos.y).sum();
    let sxy: f64 = board.pins.iter().enumerate().map(|(i, p)| (i as f64 + 1.0) * (p.pos.x - 0.5 * p.pos.y)).sum();
    assert!((sx - 13730809.051).abs() < 1e-3, "sx={sx}");
    assert!((sy - 16145910.853).abs() < 1e-3, "sy={sy}");
    assert!((sxy - 32311956609.269512).abs() < 1.0, "sxy={sxy}");
}

/// The PCIe edge connector (CN1, shape CON_FINGER_PEX_164) has long thin fingers, all oriented the same way.
#[test]
fn pcie_fingers_are_long_thin_rectangles() {
    let Some(board) = fixture() else { return };
    let pins = part_pins(&board, "CN1");
    assert_eq!(pins.len(), 164);
    let mut tall = 0;
    for pin in &pins {
        let (w, h) = pin.pad_size().unwrap_or_else(|| panic!("CN1.{} has no pad size", pin.name));
        assert!(matches!(pin.shape, PadShape::Rect | PadShape::Square), "CN1.{} shape {:?}", pin.name, pin.shape);
        // Orientation in board space: the pad's long axis after rotation.
        let (hx, hy) = pin.pad_half_extents().unwrap();
        let ratio = w.max(h) / w.min(h);
        if hy > hx && ratio > 2.0 {
            tall += 1;
        }
    }
    // A couple of special pins (hot-plug detect, GPA1) may be shorter; the bulk must be thin fingers.
    assert!(tall >= 160, "only {tall} of 164 fingers are tall and thin");
}

#[test]
fn pads_carry_physical_sizes_and_rotations() {
    let Some(board) = fixture() else { return };
    let sized = board.pins.iter().filter(|p| p.pad_size().is_some()).count();
    assert!(sized * 100 >= board.pins.len() * 95, "only {sized}/{} pins have a pad size", board.pins.len());
    assert!(board.pins.iter().any(|p| p.shape == PadShape::Round && p.pad_size().is_some()));
    assert!(board.pins.iter().any(|p| p.shape == PadShape::Rect && p.rotation_deg != 0.0));
}

#[test]
fn parts_carry_outlines_that_contain_their_pads() {
    let Some(board) = fixture() else { return };
    let with_outline = board.parts.iter().filter(|p| p.outline.len() >= 3).count();
    assert!(with_outline * 100 >= board.parts.len() * 90, "{with_outline}/{} parts have an outline", board.parts.len());
    for part in &board.parts {
        let Some(bb) = part.bounding_box else { continue };
        for &pi in &part.pin_indices {
            let pin = &board.pins[pi];
            let (hx, hy) = pin.pad_half_extents().unwrap_or((0.0, 0.0));
            assert!(
                pin.pos.x - hx >= bb.min_x - 1e-6 && pin.pos.x + hx <= bb.max_x + 1e-6
                    && pin.pos.y - hy >= bb.min_y - 1e-6 && pin.pos.y + hy <= bb.max_y + 1e-6,
                "{}.{} pad sticks out of the part bounds",
                part.name,
                pin.name
            );
        }
    }
}

