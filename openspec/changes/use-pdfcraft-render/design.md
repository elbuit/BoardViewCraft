# Design: Native PDF Rendering and Inspection with pdfcraft-render

## Architecture Overview

Replace external Poppler utilities (`pdfinfo`, `pdftoppm`, `pdftotext`) with direct in-process calls to `pdfcraft-render`.

```
                        ┌───────────────────────────────────┐
                        │          BoardViewCraft           │
                        │        (crates/ui-egui)           │
                        └─────────────────┬─────────────────┘
                                          │
                        ┌─────────────────▼─────────────────┐
                        │          pdfcraft-render          │
                        │       (v0.15.0 pure Rust)         │
                        └───────┬───────────────────┬───────┘
                                │                   │
                ┌───────────────▼───────┐   ┌───────▼───────────────┐
                │     PageRenderer      │   │       PageText        │
                │  - Page count         │   │  - find_opts(...)     │
                │  - render(Pixels)     │   │  - glyph_rects(...)   │
                │  - In-process raster  │   │  - Cross-probe index  │
                └───────────────────────┘   └───────────────────────┘
```

## Technical Components & Flow

### 1. Document Loading and Page Count
- Instead of executing `pdfinfo path`:
  - Read bytes via `std::fs::read(&path)` or memory map.
  - Instantiate `PageRenderer::new(bytes.into(), RenderConfig::default())`.
  - Obtain total page count instantly via `renderer.page_count()`.

### 2. Page Rasterization
- When `PdfViewer::request_page(page, dpi)` is called:
  - Submit request to worker thread with `bytes` (shared via `Arc<Vec<u8>>`).
  - Calculate `scale = dpi as f32 / 72.0`.
  - Issue `renderer.render(RenderRequest { page: page - 1, scale, kind: RequestKind::Pixels, ..Default::default() })`.
  - Extract `rendered.rgba` (`Pixels` implements `Deref<Target = [u8]>`).
  - Convert directly into `egui::ColorImage::from_rgba_unmultiplied([w, h], &rendered.rgba)` (or premultiplied) and register as an `egui::TextureHandle`.
  - Zero disk I/O, zero PNG encoding/decoding overhead.

### 3. Cross-Probing & Text Token Indexing
- Background indexer thread calls `renderer.filtered_text(page_idx, TextFilter::All)` or `renderer.render(RenderRequest { kind: RequestKind::Text, .. })`.
- For each token/word in `PageText`:
  - `page_text.find_opts(needle, false, true)` or iterate words.
  - Calculate normalized bounding boxes relative to page dimensions `[x_min, y_min, x_max, y_max]`.
  - Populate `SchematicIndex` with `(component_name, page, rect)`.
  - Matches the exact existing cross-probe coordinate lookup and 300% zoom centering logic.

### 4. Dependency Configuration
- In root `Cargo.toml`:
  ```toml
  [workspace.dependencies]
  pdfcraft-render = { path = "../pdfcraft/crates/render", version = "0.15.0" }
  ```
- In `crates/ui-egui/Cargo.toml`:
  ```toml
  [dependencies]
  pdfcraft-render = { workspace = true }
  ```

### 5. Packaging Updates
- Remove `poppler-tools` module from `packaging/linux/flatpak/io.github.elbuit.BoardViewCraft.bundle.yml`.

## Tradeoffs & Verification
- **Speed**: Significant performance gain by eliminating subprocess fork/exec and PNG filesystem I/O.
- **Portability**: Fully self-contained pure Rust binary on Linux, macOS, and Windows.
- **Verification**: `cargo test --workspace` and manual fixture verification for cross-probing accuracy.
