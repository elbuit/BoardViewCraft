# Delta Specification: Native PDF Rendering and Inspection with pdfcraft-render

## Scope & Delta
Modifies requirement for `pdf-schematic-auto-prompt` capability to mandate in-process, pure Rust PDF rendering and token inspection using `pdfcraft-render` instead of system CLI executables (`pdfinfo`, `pdftoppm`, `pdftotext`).

## Requirements

### Requirement 1: In-Process Document Inspection
The application SHALL inspect PDF document metadata and determine page counts using `pdfcraft_render::inspect(...)` on in-memory bytes or file paths, without spawning `pdfinfo` or external processes.

### Requirement 2: Pure Rust Background Rasterization
The application SHALL render PDF pages into pixel buffers (`egui::ColorImage`) using `pdfcraft_render::PageRenderer` or `pdfcraft_render::RenderPool` in background worker threads. No temporary PNG files or `pdftoppm` invocations SHALL be made.

### Requirement 3: In-Process Text Indexing and Bounding Boxes
The application SHALL extract text tokens and their respective page-relative bounding boxes using `pdfcraft_render::PageText` to index components for cross-probing. Spawning `pdftotext` SHALL be prohibited.

### Requirement 4: Packaging and Runtime Independence
The application binary and packaging (Flatpak, standalone bundles) SHALL NOT require `poppler-utils`, `libpoppler`, or companion CLI binaries installed on the host system to view PDFs or perform cross-probing.

## Scenarios

### Scenario 1: PDF Document Loading
- **Given** a boardview file with an associated `.pdf` schematic.
- **When** the user confirms opening the schematic.
- **Then** `pdfcraft_render::inspect` loads the document in-memory, determining page count and dimensions without spawning `pdfinfo`.

### Scenario 2: Background Page Render
- **Given** a loaded PDF document.
- **When** the active page is requested at a specified DPI.
- **Then** `pdfcraft-render` rasterizes the page in a background thread to raw RGBA bytes and uploads it as an `egui` texture.

### Scenario 3: Component Cross-Probing
- **Given** an indexed PDF schematic with text extracted via `PageText`.
- **When** a user clicks component "C35" in the boardview.
- **Then** the schematic viewer centers page coordinates on "C35" with $\ge 300\%$ zoom if unzoomed, matching previous cross-probing precision.
