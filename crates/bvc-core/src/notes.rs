//! Technician notes and reference measurements system.
//!
//! Provides structured models for technician observations and physical measurements
//! (voltage, resistance to ground, diode drop, and notes) attached to specific components
//! or pins, with JSON serialization and persistence.

use serde::{Deserialize, Serialize};

/// Target entity for a technician note.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct NoteTarget {
    /// Component reference designator (e.g. "U1", "R10", "C45").
    pub component_name: String,
    /// Optional pin number or label (e.g. "1", "A3", "GND").
    pub pin_name: Option<String>,
}

impl NoteTarget {
    pub fn component(name: impl Into<String>) -> Self {
        Self {
            component_name: name.into(),
            pin_name: None,
        }
    }

    pub fn pin(component: impl Into<String>, pin: impl Into<String>) -> Self {
        Self {
            component_name: component.into(),
            pin_name: Some(pin.into()),
        }
    }

    pub fn display_label(&self) -> String {
        match &self.pin_name {
            Some(pin) => format!("{}.{}", self.component_name, pin),
            None => self.component_name.clone(),
        }
    }
}

/// Physical measurements recorded by technician with test equipment.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct TechnicalMeasurements {
    /// DC or operating voltage reading (e.g. "3.3 V", "1.8 V").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub voltage: Option<String>,
    /// Resistance reading (e.g. "4.7 kΩ", "10 Ω").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resistance: Option<String>,
    /// Diode drop voltage test (e.g. "0.450 V").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diode_drop: Option<String>,
    /// Capacitance reading (e.g. "100 nF", "10 µF").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capacitance: Option<String>,
    /// Auxiliary reading (frequency, temperature, etc.).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub other: Option<String>,
}

impl TechnicalMeasurements {
    pub fn is_empty(&self) -> bool {
        self.voltage.as_ref().map_or(true, |s| s.trim().is_empty())
            && self.resistance.as_ref().map_or(true, |s| s.trim().is_empty())
            && self.diode_drop.as_ref().map_or(true, |s| s.trim().is_empty())
            && self.capacitance.as_ref().map_or(true, |s| s.trim().is_empty())
            && self.other.as_ref().map_or(true, |s| s.trim().is_empty())
    }

    /// Capacitance measurement value.
    pub fn capacitance_value(&self) -> Option<&str> {
        self.capacitance.as_deref()
    }
}

/// A stored technician note entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoardNote {
    pub id: String,
    pub target: NoteTarget,
    pub text: String,
    #[serde(default)]
    pub measurements: TechnicalMeasurements,
    /// ISO 8601 UTC timestamp of last update.
    pub updated_at: String,
}

impl BoardNote {
    pub fn new(target: NoteTarget, text: String, measurements: TechnicalMeasurements) -> Self {
        let id = format!("{}_{}", target.display_label(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis());
        let updated_at = format!("{:?}", std::time::SystemTime::now());
        Self {
            id,
            target,
            text,
            measurements,
            updated_at,
        }
    }
}

/// Collection of notes for a board design.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct BoardNotesStore {
    pub notes: Vec<BoardNote>,
}

impl BoardNotesStore {
    pub fn find_for_component(&self, comp_name: &str) -> Option<&BoardNote> {
        self.notes.iter().find(|n| n.target.component_name == comp_name && n.target.pin_name.is_none())
    }

    pub fn find_for_pin(&self, comp_name: &str, pin_name: &str) -> Option<&BoardNote> {
        self.notes.iter().find(|n| n.target.component_name == comp_name && n.target.pin_name.as_deref() == Some(pin_name))
    }

    pub fn upsert(&mut self, target: NoteTarget, text: String, measurements: TechnicalMeasurements) {
        if let Some(existing) = self.notes.iter_mut().find(|n| n.target == target) {
            existing.text = text;
            existing.measurements = measurements;
            existing.updated_at = format!("{:?}", std::time::SystemTime::now());
        } else {
            self.notes.push(BoardNote::new(target, text, measurements));
        }
    }

    pub fn remove(&mut self, target: &NoteTarget) -> bool {
        let prev_len = self.notes.len();
        self.notes.retain(|n| &n.target != target);
        self.notes.len() < prev_len
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

/// Result of comparing a measured value against a reference value with a given tolerance.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MeasurementComparison {
    pub measured_val: f64,
    pub reference_val: f64,
    pub deviation_pct: f64,
    pub is_pass: bool,
}

impl MeasurementComparison {
    /// Formatted status string, e.g. "PASS (±0.0%)", "PASS (+3.2%)", "FAIL (-15.4%)".
    pub fn badge_text(&self) -> String {
        let sign = if self.deviation_pct > 0.0 {
            "+"
        } else if self.deviation_pct < 0.0 {
            ""
        } else {
            "±"
        };
        let status = if self.is_pass { "PASS" } else { "FAIL" };
        format!("{status} ({sign}{:.1}%)", self.deviation_pct)
    }
}

/// Parses an electrical engineering value string (e.g. "0.450V", "450mV", "10k", "4k7", "2R2", "100nF") into a numeric float.
pub fn parse_engineering_value(s: &str) -> Option<f64> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }

    // Clean standard electrical unit suffixes (V, Ω, ohm, F, Hz, A, etc.)
    let mut cleaned = s;
    for suffix in &["ohm", "Ohm", "OHM", "Ω", "V", "v", "F", "f", "Hz", "hz", "A", "a"] {
        if let Some(rest) = cleaned.strip_suffix(suffix) {
            cleaned = rest.trim();
            break;
        }
    }

    if cleaned.is_empty() {
        return None;
    }

    let mut multiplier = 1.0f64;
    let mut num_str = String::new();

    // Check for trailing SI unit prefix (e.g. "10k", "450m", "100n", "10u", "1M")
    if let Some(last) = cleaned.chars().last() {
        if matches!(last, 'p' | 'P' | 'n' | 'N' | 'u' | 'µ' | 'm' | 'k' | 'K' | 'M' | 'G' | 'g') {
            multiplier = match last {
                'p' | 'P' => 1e-12,
                'n' | 'N' => 1e-9,
                'u' | 'µ' => 1e-6,
                'm' => 1e-3,
                'k' | 'K' => 1e3,
                'M' => 1e6,
                'G' | 'g' => 1e9,
                _ => 1.0,
            };
            num_str = cleaned[..cleaned.len() - last.len_utf8()].trim().to_string();
        }
    }

    // If no trailing prefix found, check for embedded prefix notation (e.g. "4k7", "2R2", "1M5")
    if num_str.is_empty() {
        if cleaned.contains(['k', 'K', 'R', 'r', 'M', 'u', 'µ', 'n', 'p']) {
            for c in cleaned.chars() {
                match c {
                    'k' | 'K' => {
                        num_str.push('.');
                        multiplier = 1e3;
                    }
                    'M' => {
                        num_str.push('.');
                        multiplier = 1e6;
                    }
                    'R' | 'r' => {
                        num_str.push('.');
                        multiplier = 1.0;
                    }
                    'u' | 'µ' => {
                        num_str.push('.');
                        multiplier = 1e-6;
                    }
                    'n' | 'N' => {
                        num_str.push('.');
                        multiplier = 1e-9;
                    }
                    'p' | 'P' => {
                        num_str.push('.');
                        multiplier = 1e-12;
                    }
                    _ => num_str.push(c),
                }
            }
        } else {
            num_str = cleaned.to_string();
        }
    }

    let val: f64 = num_str.trim().parse().ok()?;
    Some(val * multiplier)
}

/// Compares a technician measurement against a reference value given a tolerance percentage.
///
/// Features auto-normalization between V and mV for diode-drop readings (e.g. 450 vs 0.450).
pub fn compare_measurement(measured: &str, reference: &str, tolerance_pct: f32) -> Option<MeasurementComparison> {
    let mut m = parse_engineering_value(measured)?;
    let mut r = parse_engineering_value(reference)?;

    if r.abs() < 1e-9 {
        return None;
    }

    // Auto-normalize diode drop readings between V and mV:
    // If reference is around 200..900 (mV) and measured is 0.2..0.9 (V), scale measured to mV
    if r > 10.0 && m < 2.0 && m > 0.0 {
        m *= 1000.0;
    } else if r < 2.0 && r > 0.0 && m > 10.0 {
        r *= 1000.0;
    }

    let deviation_pct = ((m - r) / r) * 100.0;
    let is_pass = deviation_pct.abs() <= (tolerance_pct as f64);

    Some(MeasurementComparison {
        measured_val: m,
        reference_val: r,
        deviation_pct,
        is_pass,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_engineering_value() {
        assert_eq!(parse_engineering_value("0.450V"), Some(0.450));
        assert_eq!(parse_engineering_value("450mV"), Some(0.450));
        assert_eq!(parse_engineering_value("450 m"), Some(0.450));
        assert_eq!(parse_engineering_value("10k"), Some(10000.0));
        assert_eq!(parse_engineering_value("4.7 kΩ"), Some(4700.0));
        assert_eq!(parse_engineering_value("4k7"), Some(4700.0));
        assert_eq!(parse_engineering_value("2R2"), Some(2.2));
        assert!((parse_engineering_value("100nF").unwrap() - 100e-9).abs() < 1e-15);
        assert_eq!(parse_engineering_value("3.3 V"), Some(3.3));
        assert_eq!(parse_engineering_value("OL"), None);
        assert_eq!(parse_engineering_value(""), None);
    }

    #[test]
    fn test_compare_measurement_tolerance() {
        // Within 10% tolerance (PASS)
        let comp = compare_measurement("0.460V", "0.450V", 10.0).expect("comparison");
        assert!(comp.is_pass);
        assert!((comp.deviation_pct - 2.222).abs() < 0.01);
        assert_eq!(comp.badge_text(), "PASS (+2.2%)");

        // Negative deviation within tolerance
        let comp_neg = compare_measurement("0.430", "0.450", 10.0).expect("comparison");
        assert!(comp_neg.is_pass);
        assert_eq!(comp_neg.badge_text(), "PASS (-4.4%)");

        // Exceeding 10% tolerance (FAIL)
        let comp_fail = compare_measurement("0.600V", "0.450V", 10.0).expect("comparison");
        assert!(!comp_fail.is_pass);
        assert_eq!(comp_fail.badge_text(), "FAIL (+33.3%)");

        // Auto-normalize mV vs V
        let comp_auto = compare_measurement("0.450V", "450", 10.0).expect("auto-normalize comparison");
        assert!(comp_auto.is_pass);
        assert_eq!(comp_auto.badge_text(), "PASS (±0.0%)");
    }

    #[test]
    fn test_notes_store_lifecycle_and_json() {
        let mut store = BoardNotesStore::default();
        let target = NoteTarget::pin("U1", "3");

        let measurements = TechnicalMeasurements {
            voltage: Some("3.3 V".into()),
            resistance: Some("4.7 kΩ".into()),
            diode_drop: Some("0.450 V".into()),
            capacitance: Some("100 nF".into()),
            other: None,
        };

        store.upsert(target.clone(), "VIN line OK".into(), measurements.clone());
        assert_eq!(store.notes.len(), 1);

        let note = store.find_for_pin("U1", "3").expect("find note");
        assert_eq!(note.text, "VIN line OK");
        assert_eq!(note.measurements.voltage.as_deref(), Some("3.3 V"));
        assert_eq!(note.measurements.diode_drop.as_deref(), Some("0.450 V"));
        assert_eq!(note.measurements.capacitance.as_deref(), Some("100 nF"));

        let json = store.to_json().expect("to json");
        let decoded = BoardNotesStore::from_json(&json).expect("from json");
        assert_eq!(decoded.notes.len(), 1);
        assert_eq!(decoded.notes[0].target, target);

        assert!(store.remove(&target));
        assert!(store.find_for_pin("U1", "3").is_none());
    }
}
