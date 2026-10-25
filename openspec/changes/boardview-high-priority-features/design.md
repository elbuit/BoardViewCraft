# Design: BoardView High Priority Features & PDF Schematic Integration

## Technical Approach

Implement the 5 high-priority feature sets with clean-room modular architecture across `bvc-core` and `bvc-app`:
1. **Ergonomics & Navigation**:
   - Integrate drag-and-drop file detection in `BoardViewApp::ui` leveraging `egui::Context::input(|i| i.raw.dropped_files)`.
   - Implement persistent MRU in `Preferences` with atomic write protection and safe fallback for deleted files.
   - Expand `canvas.rs` input handling for arrow-key world panning and `+`/`-` scaling, guarded by keyboard focus checks.
2. **Reference Measurements**:
   - Add optional measurement fields to `Pin`, `Net`, and `Board` in `bvc-core::model`.
   - Surface measurement viewing and inline live editing in `SidePanel` under `Pin Details` and `Net Details`.
3. **PDF Schematic Viewer & Cross-Probing**:
   - Add a lightweight schematic indexing and rendering module referencing `pdfcraft`'s text search (`pdfcraft::render::text::search`) and document abstraction.
   - Render PDF page textures via `egui::ColorImage` into an egui canvas alongside or docked with the boardview canvas.
   - Fast lookup index mapping normalized tokens (`U7400`, `PP3V3_G3H`) to `(page_index, normalized_bbox)`.

## Architecture Decisions

### Decision: Drag & Drop Event Routing
**Choice**: Intercept `egui::Context::input` at the top of the main UI frame in `BoardViewApp::ui`. Classify file extension into Boardview (`.cad`, `.brd`, etc.) vs Image (`.png`, `.jpg`, etc.).
**Alternatives considered**: Canvas-only drop zone.
**Rationale**: Dropping anywhere in the window should naturally load the board or overlay; canvas-only restricts users unnecessarily.

### Decision: In-Process Lightweight PDF Document Abstraction
**Choice**: Create a dedicated module `crates/bvc-app/src/pdf.rs` implementing document loading, page raster caching (via `image` crate or `pdfcraft`'s renderer / `pdfium`), and coordinate token indexing.
**Alternatives considered**: Launching external viewers (SumatraPDF/Acrobat) with IPC.
**Rationale**: Native in-app split-screen with synchronized cross-probing provides an immediate 60 fps experience without external process dependencies.

### Decision: Measurement Persistence
**Choice**: Store technician-entered measurements in a companion JSON file (`<boardname>.measurements.json`) alongside the loaded board or in the application configuration directory.
**Alternatives considered**: Modifying original binary/text boardview files.
**Rationale**: Original boardviews must remain immutable and untouched to prevent corruption.

## Data Flow

### Boardview ↔ Schematic Cross-Probing
```
[ User Clicks Pin / Net on Canvas ]
               │
               ▼
[ BoardViewApp::selected_net / selected_part_idx updated ]
               │
               ▼
[ PdfSchematic::find_token(token) ] ──→ Queries In-Memory Token Index
               │
               ▼
[ Jump to Page(idx) + Focus Rect(x, y, w, h) ]
               │
               ▼
[ PDF Viewport renders page + draws animated highlight border ]
```

### Drag & Drop Flow
```
[ OS File Drop Event ] ──→ egui dropped_files
                               │
            ┌──────────────────┴──────────────────┐
            ▼                                     ▼
     [.cad, .brd, .fz]                    [.png, .jpg, .webp]
            │                                     │
            ▼                                     ▼
  ParserRegistry::parse_file()            OverlayManager::load_from_bytes()
            │                                     │
            ▼                                     ▼
  Canvas fitted & displayed            Overlay alignment active
```

## File Changes

| File | Action | Description |
|------|--------|-------------|
| `crates/bvc-core/src/model.rs` | Modify | Add `MeasurementRecord` and optional fields on `Pin` and `Net`. |
| `crates/bvc-core/src/parser/` | Modify | Ingest diode/voltage metadata where present. |
| `crates/bvc-app/src/preferences.rs` | Modify | Add `recent_files: Vec<PathBuf>` and keybinding definitions. |
| `crates/bvc-app/src/canvas.rs` | Modify | Handle arrow key pan and `+`/`-` zoom. |
| `crates/bvc-app/src/side_panel.rs` | Modify | Display and edit pin/net diode drop and voltage readings. |
| `crates/bvc-app/src/pdf.rs` | Create | PDF page rasterization, token indexer, and split-screen UI renderer. |
| `crates/bvc-app/src/main.rs` | Modify | Ingest dropped files, show Recent Files menu, and split-view layout. |

## Interfaces / Contracts

### Measurements Data Model (`crates/bvc-core/src/model.rs`)
```rust
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MeasurementRecord {
    pub diode_mv: Option<f32>,
    pub resistance_ohm: Option<f64>,
    pub voltage_v: Option<f32>,
    pub user_measured_diode_mv: Option<f32>,
    pub user_measured_voltage_v: Option<f32>,
    pub notes: Option<String>,
}
```

### PDF Schematic Search Index (`crates/bvc-app/src/pdf.rs`)
```rust
pub struct PdfTokenOccurrence {
    pub page_index: usize,
    pub rect: egui::Rect,
}

pub struct PdfSchematicIndex {
    pub tokens: std::collections::HashMap<String, Vec<PdfTokenOccurrence>>,
}

impl PdfSchematicIndex {
    pub fn find(&self, query: &str) -> &[PdfTokenOccurrence] {
        // Fast case-insensitive token lookup
    }
}
```

## Testing Strategy

| Layer | What to Test | Approach |
|---|---|---|
| Unit | Recent files serialization & de-duplication | Unit test in `preferences.rs` verifying 10-item cap and MRU order. |
| Unit | Arrow-key pan & zoom delta computation | Math assertions on viewport center adjustments in `canvas.rs`. |
| Unit | Measurement record calculation & serialization | Test round-trip serialization of `MeasurementRecord` in `model.rs`. |
| Unit | PDF token indexing & search | Unit tests verifying case-insensitive lookup of ICs and nets. |
| Integration | Dropped file format sniffing & dispatch | Simulated dropped file event parsing supported and unsupported files. |

## Threat Matrix
`N/A — no routing, shell, subprocess, VCS/PR automation, executable-file classification, or process-integration boundary.`

## Migration / Rollout
No database migration required. `Preferences` handles missing fields gracefully via `#[serde(default)]`.
