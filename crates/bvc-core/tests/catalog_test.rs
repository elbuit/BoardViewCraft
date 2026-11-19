use bvc_core::parser::registry::ParserRegistry;
use bvc_core::obdata::OpenBoardData;
use std::fs;
use std::path::Path;

#[test]
fn test_all_catalog_formats() {
    let dir = Path::new("/media/no_raid/devel/boarview/tmp/formats");
    let registry = ParserRegistry::default();

    let files = vec![
        "MS-V375_sample.cad",
        "MacBook_A2179_subst.brd",
        "MacBook_Air_820-00165.brd",
        "MacBook_Air_820-00165.obdata",
        "MacBook_Air_820-00165.pcb",
        "PS5_EDM010_sub_xzz.pcb",
        "Switch_HAC-CPU-20.obdata",
        "Switch_HAC-CPU-20.pcb",
        "Switch_Lite_HDH-CPU-02.obdata",
        "Switch_Lite_HDH-CPU-02.pcb",
        "Switch_OLED_HEG01_xzz.obdata",
        "Switch_OLED_HEG01_xzz.pcb",
        "iPad_Air3_820-01529.obdata",
        "iPad_Air3_820-01529.pcb",
        "sample.dsn",
        "sample.kicad_pcb",
        "sample_allegro_binary.brd",
        "sample_altium.PcbDoc",
        "sample_bv2.bv2",
        "sample_bvr3.bvr",
        "sample_cpd.cpd",
        "sample_eagle.brd",
        "sample_easyeda_pro.epcb",
        "sample_easyeda_std.json",
        "sample_flexbv.brd",
        "sample_ipc2581.xml",
        "sample_jet_binary.bv",
        "sample_mentor.neu",
        "sample_odbpp.tgz",
        "sample_pinlist.csv",
        "sample_pinlist.tsv",
        "sample_plain.bdv",
        "sample_samsung.cad",
        "sample_tebo.ict",
        "sample_test.cst",
        "sample_test.ipc",
        "sample_tsict.asc",
        "sample_zlib.fz",
        "test1_production.tvw",
    ];

    let mut failed = Vec::new();
    let mut ok_count = 0;

    for fname in &files {
        let p = dir.join(fname);
        let data = match fs::read(&p) {
            Ok(d) => d,
            Err(e) => {
                failed.push(format!("{}: I/O read error: {}", fname, e));
                continue;
            }
        };

        if fname.ends_with(".obdata") {
            let text = String::from_utf8_lossy(&data);
            match OpenBoardData::parse(&text) {
                Ok(ob) => {
                    println!("OK (obdata): {} -> {} components, {} nets, {} diag", fname, ob.components.len(), ob.nets.len(), ob.diagnosis.len());
                    ok_count += 1;
                }
                Err(e) => {
                    failed.push(format!("{}: OBDATA parse failed: {:?}", fname, e));
                }
            }
        } else {
            match registry.parse_file(fname, &data) {
                Ok(board) => {
                    println!(
                        "OK: {} -> {} parts, {} pins, {} vias, {} outline points",
                        fname,
                        board.parts.len(),
                        board.pins.len(),
                        board.vias.len(),
                        board.outline_polygon.len()
                    );
                    ok_count += 1;
                }
                Err(e) => {
                    failed.push(format!("{}: parse failed: {:?}", fname, e));
                }
            }
        }
    }

    println!("\nSUMMARY: {} OK, {} FAILED", ok_count, failed.len());
    for f in &failed {
        eprintln!("FAIL: {}", f);
    }
    assert!(failed.is_empty(), "Some files failed to parse: {:?}", failed);
}
