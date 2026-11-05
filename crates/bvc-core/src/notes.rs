//! Technician notes and reference measurements system.
//!
//! Provides structured models for technician observations and physical measurements
//! (voltage, resistance to ground, diode drop, and notes) attached to specific components
//! or pins, with JSON serialization and persistence.

use serde::{Deserialize, Serialize};

/// Target entity for a technician note.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
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
    /// Auxiliary reading (frequency, temperature, etc.).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub other: Option<String>,
}

impl TechnicalMeasurements {
    pub fn is_empty(&self) -> bool {
        self.voltage.as_ref().map_or(true, |s| s.trim().is_empty())
            && self.resistance.as_ref().map_or(true, |s| s.trim().is_empty())
            && self.diode_drop.as_ref().map_or(true, |s| s.trim().is_empty())
            && self.other.as_ref().map_or(true, |s| s.trim().is_empty())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_notes_store_lifecycle_and_json() {
        let mut store = BoardNotesStore::default();
        let target = NoteTarget::pin("U1", "3");

        let measurements = TechnicalMeasurements {
            voltage: Some("3.3 V".into()),
            resistance: Some("4.7 kΩ".into()),
            diode_drop: Some("0.450 V".into()),
            other: None,
        };

        store.upsert(target.clone(), "VIN line OK".into(), measurements.clone());
        assert_eq!(store.notes.len(), 1);

        let note = store.find_for_pin("U1", "3").expect("find note");
        assert_eq!(note.text, "VIN line OK");
        assert_eq!(note.measurements.voltage.as_deref(), Some("3.3 V"));

        let json = store.to_json().expect("to json");
        let decoded = BoardNotesStore::from_json(&json).expect("from json");
        assert_eq!(decoded.notes.len(), 1);
        assert_eq!(decoded.notes[0].target, target);

        assert!(store.remove(&target));
        assert!(store.find_for_pin("U1", "3").is_none());
    }
}
