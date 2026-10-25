# Tasks: BoardView High Priority Features & PDF Schematic Integration

## Phase 1: Ergonomics & Navigation (Básica)

- [x] 1.1 **Recent Files History in Preferences**
  - [x] 1.1.1 Add `recent_files: Vec<PathBuf>` to `Preferences` with `#[serde(default)]` and cap at 10 items.
  - [x] 1.1.2 Implement helper `Preferences::add_recent_file(&mut self, path: PathBuf)`.
  - [x] 1.1.3 Write unit tests in `preferences.rs` verifying order, de-duplication, and capping.
  - [x] 1.1.4 Add "Open Recent" menu button in the top toolbar in `crates/bvc-app/src/main.rs`.

- [x] 1.2 **Drag and Drop File Ingestion**
  - [x] 1.2.1 In `BoardViewApp::ui`, read `ctx.input(|i| i.raw.dropped_files.clone())`.
  - [x] 1.2.2 Check extension: if boardview format, load board; if image format, load into `OverlayManager`.
  - [x] 1.2.3 Ensure error message is displayed cleanly in the status bar if format is unrecognized.

- [x] 1.3 **Keyboard Pan & Zoom Navigation**
  - [x] 1.3.1 In `canvas.rs`, capture arrow keys (`Up`, `Down`, `Left`, `Right`) to pan the viewport in world coordinates.
  - [x] 1.3.2 Capture `+` and `-` (including numpad) to scale zoom centered on the current view.
  - [x] 1.3.3 Ensure arrow keys are ignored when a text input (e.g. search box) has focus.

## Phase 2: Reference Measurements (Pro)

- [ ] 2.1 **Core Measurement Model**
  - [ ] 2.1.1 Add `MeasurementRecord` struct in `crates/bvc-core/src/model.rs` (diode drop mV, resistance Ω, voltage V).
  - [ ] 2.1.2 Add `measurements: Option<MeasurementRecord>` to `Pin` and `Net`.
  - [ ] 2.1.3 Write unit tests for serialization and deserialization of boards with measurement records.

- [ ] 2.2 **UI Display & User Editing**
  - [ ] 2.2.1 In `crates/bvc-app/src/side_panel.rs`, add measurement display block in `Pin Details` and `Net Details`.
  - [ ] 2.2.2 Add input fields for the user to record live measured diode mV and voltage.
  - [ ] 2.2.3 Render reference values on pin hover tooltip on the main canvas.

## Phase 3: PDF Schematic Integration & Cross-Probing (Pro)

- [x] 3.1 **PDF Module & Token Indexing**
  - [x] 3.1.1 Create `crates/bvc-app/src/pdf.rs` with document representation and coordinate text indexer referencing `pdfcraft::render::text::search`.
  - [x] 3.1.2 Implement `PdfSchematicIndex` mapping tokens (e.g. `U7400`, `PP3V3_S5`) to `(page_index, rect)`.
  - [x] 3.1.3 Write unit tests verifying case-insensitive multi-page token lookup.

- [x] 3.2 **Split-View Canvas & Cross-Probing**
  - [x] 3.2.1 Add toggle button in toolbar or menu to open and display schematic PDF in a side-by-side split panel.
  - [x] 3.2.2 On selecting a component or net in BoardViewCraft, query `PdfSchematicIndex` and jump to the matching page and rect.
  - [x] 3.2.3 Provide Next/Previous buttons in the PDF toolbar when a net has multiple occurrences.
  - [x] 3.2.4 Clicking a token in the PDF view focuses and highlights the corresponding pin/net in the boardview.

## Phase 4: Verification & Delivery

- [ ] 4.1 Run full workspace verification: `cargo check` and `cargo test --ignore-rust-version`.
- [ ] 4.2 Validate drag & drop, keyboard navigation, and PDF cross-probing interactively.
