//! Vector schematic viewer state and egui canvas renderer.

use bvc_core::schematic::{
    parse_eagle_sch, parse_kicad_sch, SchPoint, SchSheetDef, Schematic,
};
use egui::{Pos2, Vec2};
use std::path::{Path, PathBuf};

#[derive(Default)]
pub struct CadSchematicViewer {
    pub schematic: Option<Schematic>,
    pub current_sheet_idx: usize,
    pub filename: Option<String>,
    pub file_path: Option<PathBuf>,

    // Viewport transform (pan / zoom)
    pub pan: Vec2,
    pub zoom: f32,

    // Selection / Highlighting
    pub selected_symbol: Option<String>, // Symbol ID or ref_des
    pub selected_net: Option<String>,    // Net name
    pub search_input: String,

    // Reverse cross-probing action request
    pub navigate_to_pcb: Option<String>,
}

impl CadSchematicViewer {
    pub fn is_loaded(&self) -> bool {
        self.schematic.is_some()
    }

    pub fn close(&mut self) {
        self.schematic = None;
        self.filename = None;
        self.file_path = None;
        self.selected_symbol = None;
        self.selected_net = None;
        self.search_input.clear();
        self.navigate_to_pcb = None;
    }

    pub fn open_file(&mut self, path: &Path) -> Result<(), String> {
        let content = std::fs::read_to_string(path).map_err(|e| format!("Could not read file: {e}"))?;
        let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("").to_ascii_lowercase();

        let sch = if ext == "kicad_sch" {
            parse_kicad_sch(&content).map_err(|e| format!("KiCad parse error: {e}"))?
        } else if ext == "sch" {
            // Try EAGLE XML parser
            parse_eagle_sch(&content).map_err(|e| format!("EAGLE parse error: {e}"))?
        } else {
            return Err(format!("Unsupported schematic extension: .{ext}"));
        };

        let file_name = path.file_name().and_then(|f| f.to_str()).unwrap_or("Schematic").to_string();
        self.schematic = Some(sch);
        self.current_sheet_idx = 0;
        self.filename = Some(file_name);
        self.file_path = Some(path.to_path_buf());
        self.zoom = 1.0;
        self.pan = Vec2::ZERO;

        Ok(())
    }

    pub fn current_sheet(&self) -> Option<&SchSheetDef> {
        let sch = self.schematic.as_ref()?;
        sch.defs.get(self.current_sheet_idx)
    }

    pub fn cross_probe(&mut self, target: &str) -> bool {
        let Some(sch) = &self.schematic else { return false };
        let target_clean = target.trim();

        // 1. Check if target matches symbol ref
        for (idx, sheet) in sch.defs.iter().enumerate() {
            for sym in &sheet.symbols {
                if sym.ref_default.eq_ignore_ascii_case(target_clean)
                    || sym.instances.values().any(|r| r.eq_ignore_ascii_case(target_clean))
                {
                    self.current_sheet_idx = idx;
                    self.selected_symbol = Some(sym.ref_default.clone());
                    self.selected_net = None;
                    return true;
                }
            }
        }

        // 2. Check if target matches net name
        if let Some(net) = sch.find_net_by_name(target_clean) {
            self.selected_net = Some(net.name.clone());
            self.selected_symbol = None;
            return true;
        }

        false
    }

    /// World schematic coords (mm) to screen screen pixels
    pub fn world_to_screen(&self, pt: SchPoint, center_offset: Pos2) -> Pos2 {
        Pos2::new(
            center_offset.x + (pt.x as f32) * self.zoom + self.pan.x,
            center_offset.y + (pt.y as f32) * self.zoom + self.pan.y,
        )
    }

    /// Screen screen pixels to world schematic coords (mm)
    pub fn screen_to_world(&self, screen_pt: Pos2, center_offset: Pos2) -> SchPoint {
        let wx = (screen_pt.x - center_offset.x - self.pan.x) / self.zoom;
        let wy = (screen_pt.y - center_offset.y - self.pan.y) / self.zoom;
        SchPoint::new(wx as f64, wy as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cad_schematic_viewer_cross_probe() {
        let kicad_data = r#"(kicad_sch (version 20231120) (generator eeschema)
  (uuid "00000000-0000-0000-0000-000000000001")
  (paper "A4")
  (lib_symbols
    (symbol "Device:R" (pin_numbers hide) (pin_names (offset 0))
      (symbol "R_1_1"
        (pin passive line (at 0 3.81 270) (length 1.27) (name "~") (number "1"))
      )
    )
  )
  (wire (pts (xy 50 50) (xy 60 50)) (stroke (width 0)))
  (symbol (lib_id "Device:R") (at 50 50 0) (unit 1)
    (uuid "00000000-0000-0000-0000-000000000002")
    (property "Reference" "R100" (at 52 50 0))
    (property "Value" "10k" (at 52 52 0))
  )
  (label "VCC_CORE" (at 60 50 0))
)"#;
        let dir = std::env::temp_dir().join("bvc_cad_test");
        let _ = std::fs::create_dir_all(&dir);
        let sch_file = dir.join("test.kicad_sch");
        std::fs::write(&sch_file, kicad_data).unwrap();

        let mut viewer = CadSchematicViewer::default();
        let res = viewer.open_file(&sch_file);
        assert!(res.is_ok(), "Failed to open kicad_sch: {:?}", res.err());
        assert!(viewer.is_loaded());

        // Probe component
        assert!(viewer.cross_probe("R100"));
        assert_eq!(viewer.selected_symbol, Some("R100".to_string()));
        assert_eq!(viewer.selected_net, None);

        // Probe net
        assert!(viewer.cross_probe("VCC_CORE"));
        assert_eq!(viewer.selected_net, Some("VCC_CORE".to_string()));
        assert_eq!(viewer.selected_symbol, None);

        // Close
        viewer.close();
        assert!(!viewer.is_loaded());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
