use bvc_core::parser::registry::ParserRegistry;
use bvc_core::model::Side;

#[test]
fn test_eagle_xml_parsing() {
    let eagle_xml = r#"<?xml version="1.0" encoding="utf-8"?>
<!DOCTYPE eagle SYSTEM "eagle.dtd">
<eagle version="9.6.2">
<drawing>
<board>
<libraries>
<library name="resistor">
<packages>
<package name="R0805">
<smd name="1" x="-0.95" y="0" dx="1.0" dy="1.3" layer="1"/>
<smd name="2" x="0.95" y="0" dx="1.0" dy="1.3" layer="1"/>
</package>
</packages>
</library>
</libraries>
<elements>
<element name="R1" library="resistor" package="R0805" x="10.0" y="20.0" rot="R90"/>
<element name="R2" library="resistor" package="R0805" x="30.0" y="20.0" rot="MR0"/>
</elements>
<signals>
<signal name="VCC">
<contactref element="R1" pad="1"/>
<contactref element="R2" pad="1"/>
</signal>
<signal name="GND">
<contactref element="R1" pad="2"/>
</signal>
</signals>
</board>
</drawing>
</eagle>
"#;

    let registry = ParserRegistry::default();
    let board = registry.parse_file("redboard.brd", eagle_xml.as_bytes()).expect("EAGLE parsing failed");

    assert_eq!(board.parts.len(), 2);
    assert_eq!(board.pins.len(), 4);

    let r1 = board.parts.iter().find(|p| p.name == "R1").unwrap();
    assert_eq!(r1.side, Side::Top);

    let r2 = board.parts.iter().find(|p| p.name == "R2").unwrap();
    assert_eq!(r2.side, Side::Bottom);

    let vcc_pins: Vec<_> = board.pins.iter().filter(|p| p.net_name == "VCC").collect();
    assert_eq!(vcc_pins.len(), 2);
}

#[test]
fn test_easyeda_standard_parsing() {
    let easyeda_std_json = r#"{
  "head": {
    "docType": "3"
  },
  "shape": [
    "LIB~10~20~package`0805~0~~id1~0#@$TEXT~P~10~20~0~0~C1#@$PAD~RECT~9~20~1~1~1~GND~1~0~#@$PAD~RECT~11~20~1~1~1~VCC~2~0~",
    "TRACK~1~10~0~0 0 100 0 100 100 0 100"
  ]
}"#;

    let registry = ParserRegistry::default();
    let board = registry.parse_file("project.json", easyeda_std_json.as_bytes()).expect("EasyEDA Standard failed");

    assert_eq!(board.parts.len(), 1);
    assert_eq!(board.parts[0].name, "C1");
    assert_eq!(board.pins.len(), 2);
    assert_eq!(board.pins[0].net_name, "GND");
    assert_eq!(board.pins[1].net_name, "VCC");
    assert_eq!(board.outline_polygon.len(), 4);
}

#[test]
fn test_easyeda_pro_line_records_parsing() {
    let pro_content = r#"
["DOCTYPE","PCB","1.8"]
["COMPONENT","comp_1","","TOP",1000,2000,0,{},0]
["ATTR","attr_1","","comp_1","TOP",1000,2000,"Designator","U1"]
["PAD","pad_1","","VCC","TOP","1",900,2000,0,0,"RECT",50,50]
["PAD","pad_2","","GND","TOP","2",1100,2000,0,0,"RECT",50,50]
["LINE","line_1","","","OUTLINE",0,0,5000,0]
["LINE","line_2","","","OUTLINE",5000,0,5000,5000]
"#;

    let registry = ParserRegistry::default();
    let board = registry.parse_file("board.epcb", pro_content.as_bytes()).expect("EasyEDA Pro parsing failed");

    assert_eq!(board.parts.len(), 1);
    assert_eq!(board.parts[0].name, "U1");
    assert_eq!(board.pins.len(), 2);
    assert_eq!(board.outline_polygon.len(), 4);
}
