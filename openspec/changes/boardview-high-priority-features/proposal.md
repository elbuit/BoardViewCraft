# Proposal: BoardView High Priority Features & PDF Schematic Integration

## Intent

Implement the high-priority functional specifications identified in `Especificacion_Pendientes_Alta_Prioridad.md`. This elevates BoardViewCraft from a pure boardview inspection tool to a full-fledged hardware repair diagnostic suite by:
1. Streamlining daily workflow ergonomics (drag & drop, recent files, keyboard navigation, configurable keybindings).
2. Implementing reference measurements (diode mode in mV, resistance in Ω, voltage in V) for component pins and nets.
3. Introducing integrated PDF schematic viewing and bidirectional cross-probing between PCB boardview layout and PDF schematics (leveraging architecture and rendering patterns from `pdfcraft`).

## Scope

### In Scope
- **File Management & Ergonomics (FMT-06, FMT-07)**:
  - Drag & Drop file opening on the main window canvas (`egui::DroppedFile`).
  - Persistent recent files history with a quick-access menu in the top bar.
- **Canvas Keyboard Navigation (VIS-01, VIS-02, COM-01)**:
  - Smooth pan with arrow keys (`Up`, `Down`, `Left`, `Right`).
  - Zoom in/out via keyboard (`+` / `-` and numpad equivalents).
  - Configurable shortcut action bindings in Preferences with clean defaults.
- **Reference Measurements (MED-01, MED-02, MED-04)**:
  - Model extensions in `bvc-core` for pin/net measurement metadata (diode drop mV, resistance Ω, voltage V).
  - Inspection and user editing of live measurements in the side panel.
  - Parsers extracting embedded diode/voltage values when present in files.
- **Integrated PDF Schematic Viewer & Cross-probing (SCH-01 .. SCH-07)**:
  - Integrate PDF document rendering and text extraction referencing `pdfcraft` (crates `pdfcraft-render` / `pdfcraft-engine` or lightweight engine integration).
  - Split-view / dock layout for simultaneous PCB and Schematic viewing.
  - Bidirectional cross-probing: clicking a component/net in BoardView highlights its location on the schematic; selecting text in PDF navigates to the board element.

### Out of Scope
- Cloud database synchronization of boardviews or community telemetry (deferred to later Pro releases).
- Full OCR on raster-only scanned schematics (deferred to post-v1).
- Direct drawing / freehand annotation canvas layers (ANO-04).

## Capabilities

### New Capabilities
- `drag-and-drop-loading`: Canvas and window drag-and-drop support for opening boardview files and overlay images.
- `recent-files-menu`: Persistent MRU (Most Recently Used) list with quick-open menu and missing-file handling.
- `keyboard-pan-zoom-shortcuts`: Keyboard-driven view manipulation and user-customizable keybinding preferences.
- `reference-measurements`: Pin/net reference values (diode mode, impedance, voltage) and user measurement input.
- `pdf-schematic-integration`: Embedded PDF rendering, text coordinate indexing, split-screen display, and bidirectional cross-probing between schematic and boardview.

### Modified Capabilities
- `None`

## Approach

1. **Ergonomics & Navigation**:
   - Capture `ctx.input(|i| i.raw.dropped_files.clone())` in `BoardViewApp::ui`. Check for supported boardview extensions or overlay image formats.
   - Extend `Preferences` with `recent_files: Vec<PathBuf>` (capped at 10 items) and a keybinding map `HashMap<Action, KeyBinding>`.
   - Add pan/zoom keyboard handling inside `canvas.rs` when no text input is focused.
2. **Measurement Layer**:
   - Enhance `bvc-core::model` with `MeasurementData { diode_mv: Option<f32>, resistance_ohm: Option<f64>, voltage_v: Option<f32>, notes: Option<String> }`.
   - Allow user annotations/measurements to persist in sidecar JSON or user settings (`.bvc-session.json`).
   - Expose diode mode and voltage fields in `SidePanel` under `Pin Details` and `Net Details`.
3. **PDF Schematic Architecture (Reference: `pdfcraft`)**:
   - Study `pdfcraft` architecture (`crates/engine`, `crates/render`, `crates/ui-egui`).
   - Embed a dedicated `PdfView` panel inside `bvc-app` (or workspace crate `bvc-pdf`).
   - Extract page text spans with bounding boxes `(page_idx, Rect, text)`. Build a search index for designators (e.g., `U7400`, `R204`) and net names (`PP3V3_G3H`).
   - On selecting an entity in BoardViewCraft, query the PDF index and jump to the matching page and bounding box with visual highlighting.

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `crates/bvc-core/src/model.rs` | Modified | Add measurement metadata structures to `Pin`, `Net`, and `Board`. |
| `crates/bvc-core/src/parser/` | Modified | Read embedded measurements where supported (e.g. FZ / CAD / BDV). |
| `crates/bvc-app/src/preferences.rs` | Modified | Store recent files list and custom keyboard shortcut bindings. |
| `crates/bvc-app/src/canvas.rs` | Modified | Arrow key pan and `+`/`-` zoom processing. |
| `crates/bvc-app/src/main.rs` | Modified | Drag & drop file ingestion, recent files UI menu, split view routing. |
| `crates/bvc-app/src/side_panel.rs` | Modified | Measurements display and user entry fields in pin/net details. |
| `crates/bvc-pdf/` (or `bvc-app/src/pdf/`) | New | PDF document loading, text extraction, page rendering, and cross-probing. |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| PDF rendering dependency size and compile times | Medium | Use modular crate architecture referencing `pdfcraft` backend patterns or lightweight engine abstractions. |
| PDF text extraction misses non-standard font encodings | Low | Normalize whitespace, hyphens, and casing; provide fallback text search. |
| Key conflict between canvas navigation and side panel input fields | Low | Guard shortcut handling with `ui.ctx().egui_wants_keyboard_input()`. |

## Rollback Plan

- Feature flag or modular crate structure (`bvc-pdf`): if the PDF viewer introduces platform-specific toolchain issues, it can be disabled via cargo feature without breaking core boardview rendering.
- Revert commits to `main` branch cleanly via git since all file modifications are modularized.

## Dependencies

- Reference workspace: `/media/no_raid/devel/boarview/pdfcraft` for PDF parsing and rendering patterns.
- External crates: `pdfcraft` crates or compatible Rust PDF rendering libraries.

## Success Criteria

- [ ] Dropping a supported boardview or image onto the window opens it immediately.
- [ ] Recent files list remembers the last opened boards and restores them in 1 click.
- [ ] Pan (arrows) and zoom (`+`/`-`) operate smoothly at 60 fps without mouse interaction.
- [ ] Diode mode and reference voltages display on pin hover/selection and can be edited.
- [ ] Associated schematic PDF loads in split view; clicking a net/component in boardview navigates directly to its location on the schematic page.
