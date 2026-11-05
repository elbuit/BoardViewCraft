use bvc_core::parser::registry::ParserRegistry;
use bvc_core::model::Side;

#[test]
fn test_bv2_parsing() {
    let bv2_content = r#"
#Layout#
X,Y,R,Group
0,0,0,1
5,0,0,1
5,3,0,1
0,3,0,1
#Nail#
Nail,X,Y,Type,Grid,TB,Net,NetName
$1,1.5,1.0,0,G1,(T),1,VCC
#Pin#
Part,TB,Pin,Name,X,Y,Layer,Netname
U1,(T),1,VCC,0.5,0.5,1,VCC
U1,(T),2,GND,0.5,1.5,1,GND
R1,(B),1,1,2.0,2.0,2,GND
"#;

    let registry = ParserRegistry::default();
    let board = registry.parse_file("board.bv2", bv2_content.as_bytes()).expect("BV2 parsing failed");

    assert_eq!(board.outline_polygon.len(), 4);
    assert_eq!(board.pins.len(), 4); // 1 TP + 3 IC pins
    assert_eq!(board.parts.len(), 3); // TP:$1, U1, R1

    let u1 = board.parts.iter().find(|p| p.name == "U1").expect("U1 missing");
    assert_eq!(u1.side, Side::Top);
    assert_eq!(u1.pin_indices.len(), 2);

    let r1 = board.parts.iter().find(|p| p.name == "R1").expect("R1 missing");
    assert_eq!(r1.side, Side::Bottom);
}

#[test]
fn test_pinlist_csv_parsing() {
    let csv_content = r#"
RefDes,Pin,X(mm),Y(mm),Side,Net
C1,1,10.0,20.0,Top,GND
C1,2,12.0,20.0,Top,VCC_3V3
R5,1,15.5,25.0,Bottom,VCC_3V3
"#;

    let registry = ParserRegistry::default();
    let board = registry.parse_file("parts.csv", csv_content.as_bytes()).expect("CSV pinlist failed");

    assert_eq!(board.parts.len(), 2);
    assert_eq!(board.pins.len(), 3);

    let gnd_pins: Vec<_> = board.pins.iter().filter(|p| p.net_name == "GND").collect();
    assert_eq!(gnd_pins.len(), 1);
    let part_c1 = &board.parts[gnd_pins[0].part_id];
    assert_eq!(part_c1.name, "C1");
}

#[test]
fn test_samsung_cad_parsing() {
    let cad_content = r#"
###Panel Added: synthetic sample
COMP  U100  PN-100  0  0  1.000  2.000  1  0
COMP  R100  PN-200  0  0  3.000  2.000  2  0
C_PIN U100-1  1.000  2.000  0  0  0  X  /VCC_CORE
C_PIN U100-2  1.100  2.000  0  0  0  X  /GND
C_PIN R100-1  3.000 -2.000  0  0  0  X  /GND
NET   /GND
N_VIA 1.250  2.250  X  1  0
"#;

    let registry = ParserRegistry::default();
    let board = registry.parse_file("samsung.cad", cad_content.as_bytes()).expect("Samsung CAD failed");

    assert_eq!(board.parts.len(), 2);
    assert_eq!(board.pins.len(), 3);
    assert_eq!(board.vias.len(), 1);

    let u100 = board.parts.iter().find(|p| p.name == "U100").unwrap();
    assert_eq!(u100.side, Side::Top);
}

#[test]
fn test_mentor_neutral_parsing() {
    let neu_content = r#"
B_UNITS mm
BOARD
COMP C10 0 100nF 0402 10 20 Top 0
C_PIN C10-1 10.0 20.0 1 Top 0 1 GND
C_PIN C10-2 11.0 20.0 1 Top 0 1 VDD
"#;

    let registry = ParserRegistry::default();
    let board = registry.parse_file("design.neu", neu_content.as_bytes()).expect("Mentor Neutral failed");

    assert_eq!(board.parts.len(), 1);
    assert_eq!(board.pins.len(), 2);
    assert_eq!(board.pins[0].net_name, "GND");
    assert_eq!(board.pins[1].net_name, "VDD");
}

#[test]
fn test_tebo_ict_parsing() {
    let ict_content = r#"
!Tebo-ict v3.0
units inches ;
OUTLINE
0.0, 0.0
4.0, 0.0
4.0, 2.0
0.0, 2.0 ;
0.5,0.5 U1.1 TOP MANDATORY ;
0.5,1.0 U1.2 TOP MANDATORY ;
"#;

    let registry = ParserRegistry::default();
    let board = registry.parse_file("fixture.ict", ict_content.as_bytes()).expect("Tebo ICT failed");

    assert_eq!(board.outline_polygon.len(), 4);
    assert_eq!(board.parts.len(), 1);
    assert_eq!(board.pins.len(), 2);
    let u1 = &board.parts[board.pins[0].part_id];
    assert_eq!(u1.name, "U1");
    assert_eq!(board.pins[0].name, "1");
}
