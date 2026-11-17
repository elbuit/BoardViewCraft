//! OpenBoardData (`.obdata`) parser and board applicator.
//!
//! OpenBoardData is an open-data format providing known-good reference measurements
//! (diode mode drops, voltages, resistances, and repair diagnostic notes)
//! for electronic hardware diagnostics.

use std::collections::HashMap;
use crate::board::Board;
use crate::notes::{BoardNotesStore, NoteTarget, TechnicalMeasurements};

/// A single component entry in `COMPONENTS_DATA`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObComponentEntry {
    pub component_name: String,
    /// Category: 'v' = value, 'p' = package/footprint, 's' = status/DNP, 'r' = rating, 'm' = misc, 'c' = manufacturer code
    pub category: char,
    pub value: String,
    pub comment: String,
}

/// A single net measurement entry in `NETS_DATA`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObNetEntry {
    pub net_name: String,
    /// Value type: 'd' = diode drop, 'v' = voltage, 'r' = resistance, 't' = comment/signal text, 'a' = alias
    pub value_type: char,
    pub value: String,
    pub comment: String,
}

/// Parsed OpenBoardData archive.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OpenBoardData {
    pub header: HashMap<String, String>,
    pub components: Vec<ObComponentEntry>,
    pub nets: Vec<ObNetEntry>,
    pub diagnosis: Vec<String>,
}

impl OpenBoardData {
    /// Parses an OpenBoardData (`.obdata`) text content.
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut data = Self::default();

        #[derive(Copy, Clone, PartialEq, Eq)]
        enum Section {
            None,
            Header,
            Diagnosis,
            Components,
            Nets,
        }

        let mut section = Section::None;

        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            // Section markers
            match trimmed {
                "HEADER_DATA_START" => {
                    section = Section::Header;
                    continue;
                }
                "HEADER_DATA_END" => {
                    section = Section::None;
                    continue;
                }
                "DIAGNOSIS_DATA_START" => {
                    section = Section::Diagnosis;
                    continue;
                }
                "DIAGNOSIS_DATA_END" => {
                    section = Section::None;
                    continue;
                }
                "COMPONENTS_DATA_START" => {
                    section = Section::Components;
                    continue;
                }
                "COMPONENTS_DATA_END" => {
                    section = Section::None;
                    continue;
                }
                "NETS_DATA_START" => {
                    section = Section::Nets;
                    continue;
                }
                "NETS_DATA_END" => {
                    section = Section::None;
                    continue;
                }
                _ => {}
            }

            // Comments
            if trimmed.starts_with('#') || trimmed.starts_with("//") {
                continue;
            }

            match section {
                Section::Header => {
                    let mut parts = trimmed.splitn(2, |c: char| c.is_whitespace());
                    if let Some(key) = parts.next() {
                        let val = parts.next().unwrap_or("").trim();
                        data.header.insert(key.to_string(), val.to_string());
                    }
                }
                Section::Diagnosis => {
                    data.diagnosis.push(trimmed.to_string());
                }
                Section::Components => {
                    // Syntax: <ComponentRef> <CategoryChar> <Value> [Comment]
                    let tokens: Vec<&str> = trimmed.split_whitespace().collect();
                    if tokens.len() >= 3 {
                        let comp_name = tokens[0].to_string();
                        let category = tokens[1].chars().next().unwrap_or('m');
                        let value = tokens[2].to_string();
                        let comment = if tokens.len() > 3 {
                            unquote(&tokens[3..].join(" "))
                        } else {
                            String::new()
                        };
                        data.components.push(ObComponentEntry {
                            component_name: comp_name,
                            category,
                            value,
                            comment,
                        });
                    }
                }
                Section::Nets => {
                    // Syntax: <NetName> <ValueTypeChar> <Value> [Comment]
                    let tokens: Vec<&str> = trimmed.split_whitespace().collect();
                    if tokens.len() >= 3 {
                        let net_name = tokens[0].to_string();
                        let value_type = tokens[1].chars().next().unwrap_or('t');
                        let value = tokens[2].to_string();
                        let comment = if tokens.len() > 3 {
                            unquote(&tokens[3..].join(" "))
                        } else {
                            String::new()
                        };
                        data.nets.push(ObNetEntry {
                            net_name,
                            value_type,
                            value,
                            comment,
                        });
                    }
                }
                Section::None => {}
            }
        }

        Ok(data)
    }

    /// Applies the parsed OpenBoardData entries to a `Board` and optionally its `BoardNotesStore`.
    ///
    /// Returns the total count of applied measurements and metadata entries.
    pub fn apply_to_board(
        &self,
        board: &mut Board,
        mut notes_store: Option<&mut BoardNotesStore>,
    ) -> usize {
        let mut count = 0;

        // 1. Component values into board.metadata
        for comp in &self.components {
            if comp.category == 'v' && !comp.value.is_empty() {
                board.metadata.set_part_value(&comp.component_name, &comp.value);
                count += 1;
            }
        }

        // 2. Net readings and voltages into board.metadata
        let mut net_measurements: HashMap<String, TechnicalMeasurements> = HashMap::new();
        let mut net_comments: HashMap<String, String> = HashMap::new();

        for entry in &self.nets {
            let net_upper = entry.net_name.trim().to_ascii_uppercase();
            if net_upper.is_empty() {
                continue;
            }

            match entry.value_type {
                'd' => {
                    board.metadata.set_net_reading(&entry.net_name, &entry.value);
                    let m = net_measurements.entry(net_upper.clone()).or_default();
                    let val = entry.value.trim();
                    let diode_str = if val.eq_ignore_ascii_case("OL") || val.ends_with('V') || val.ends_with('v') {
                        val.to_string()
                    } else {
                        format!("{val} V")
                    };
                    m.diode_drop = Some(diode_str);
                    count += 1;
                }
                'r' => {
                    board.metadata.set_net_reading(&entry.net_name, &entry.value);
                    let m = net_measurements.entry(net_upper.clone()).or_default();
                    m.resistance = Some(entry.value.clone());
                    count += 1;
                }
                'v' => {
                    board.metadata.set_net_voltage(&entry.net_name, &entry.value);
                    let m = net_measurements.entry(net_upper.clone()).or_default();
                    let val = entry.value.trim();
                    let volt_str = if val.ends_with('V') || val.ends_with('v') {
                        val.to_string()
                    } else {
                        format!("{val} V")
                    };
                    m.voltage = Some(volt_str);
                    count += 1;
                }
                't' => {
                    board.metadata.set_net_signal(&entry.net_name, &entry.value);
                    let note_text = if !entry.comment.is_empty() {
                        format!("{}: {}", entry.value, entry.comment)
                    } else {
                        entry.value.clone()
                    };
                    net_comments.insert(net_upper.clone(), note_text);
                    count += 1;
                }
                _ => {}
            }

            if !entry.comment.is_empty() && !net_comments.contains_key(&net_upper) {
                net_comments.insert(net_upper.clone(), entry.comment.clone());
            }
        }

        // 3. Propagate net measurements to notes_store pins
        if let Some(store) = notes_store.as_mut() {
            for pin in &board.pins {
                let pin_net_upper = pin.net_name.trim().to_ascii_uppercase();
                if pin_net_upper.is_empty() {
                    continue;
                }
                if let Some(meas) = net_measurements.get(&pin_net_upper) {
                    let part_name = board.parts.get(pin.part_id).map(|p| p.name.as_str()).unwrap_or("");
                    if !part_name.is_empty() && !pin.name.is_empty() {
                        let target = NoteTarget::pin(part_name, &pin.name);
                        let comment = net_comments.get(&pin_net_upper).cloned().unwrap_or_default();
                        store.upsert(target, comment, meas.clone());
                    }
                }
            }
        }

        count
    }
}

fn unquote(s: &str) -> String {
    let trimmed = s.trim();
    if (trimmed.starts_with('\'') && trimmed.ends_with('\'') && trimmed.len() >= 2)
        || (trimmed.starts_with('"') && trimmed.ends_with('"') && trimmed.len() >= 2)
    {
        trimmed[1..trimmed.len() - 1].to_string()
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::Board;
    use crate::model::{Part, Pin, Side};

    #[test]
    fn test_parse_openboarddata_sample() {
        let sample = r#"
HEADER_DATA_START
OBDATA_V002 https://openboarddata.org
TIMESTAMP 1769849076
BOARDPATH Switch OLED
HEADER_DATA_END

COMPONENTS_DATA_START
### Component Category Value Comment
C1 v 100nF
R10 v 10k
U7 v MAX77620
COMPONENTS_DATA_END

NETS_DATA_START
### Network Valuetype Value Comment
Net1 d 0.433 ''
Net4 d OL ''
PPBUS_G3H v 12.6 'Main system rail'
PP3V3_S5 r 1.2M
NETS_DATA_END
"#;

        let obdata = OpenBoardData::parse(sample).expect("parse obdata");
        assert_eq!(obdata.header.get("TIMESTAMP").map(String::as_str), Some("1769849076"));
        assert_eq!(obdata.components.len(), 3);
        assert_eq!(obdata.nets.len(), 4);

        let mut board = Board::new();
        board.parts.push(Part { name: "U7".into(), ..Default::default() });
        board.pins.push(Pin {
            part_id: 0,
            name: "1".into(),
            net_name: "Net1".into(),
            side: Side::Top,
            ..Default::default()
        });
        board.pins.push(Pin {
            part_id: 0,
            name: "4".into(),
            net_name: "Net4".into(),
            side: Side::Top,
            ..Default::default()
        });

        let mut notes_store = BoardNotesStore::default();
        let count = obdata.apply_to_board(&mut board, Some(&mut notes_store));
        assert!(count >= 5);

        // Check metadata
        assert_eq!(board.metadata.part_value("U7"), Some("MAX77620"));
        assert_eq!(board.metadata.net_reading("Net1"), Some("0.433"));
        assert_eq!(board.metadata.net_reading("Net4"), Some("OL"));
        assert_eq!(board.metadata.net_voltage("PPBUS_G3H"), Some("12.6"));

        // Check notes store
        let note1 = notes_store.find_for_pin("U7", "1").expect("note for U7.1");
        assert_eq!(note1.measurements.diode_drop.as_deref(), Some("0.433 V"));

        let note4 = notes_store.find_for_pin("U7", "4").expect("note for U7.4");
        assert_eq!(note4.measurements.diode_drop.as_deref(), Some("OL"));
    }

    #[test]
    fn test_parse_real_switch_oled_obdata() {
        let path = std::path::Path::new("../../tmp/Switch OLED-HEG-CPU-01 PCB layer.obdata");
        let alt_path = std::path::Path::new("tmp/Switch OLED-HEG-CPU-01 PCB layer.obdata");
        let actual_path = if path.exists() {
            path
        } else if alt_path.exists() {
            alt_path
        } else {
            return;
        };

        let content = std::fs::read_to_string(actual_path).expect("read real obdata");
        let obdata = OpenBoardData::parse(&content).expect("parse real obdata");
        assert!(obdata.components.len() > 100);
        assert!(obdata.nets.len() > 100);

        let mut board = Board::new();
        let mut notes_store = BoardNotesStore::default();
        let count = obdata.apply_to_board(&mut board, Some(&mut notes_store));
        assert!(count > 100);
        assert_eq!(board.metadata.net_reading("Net1"), Some("0.433"));
        assert_eq!(board.metadata.net_reading("Net4"), Some("OL"));
    }
}
