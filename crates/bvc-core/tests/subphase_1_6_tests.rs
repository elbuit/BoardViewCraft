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
