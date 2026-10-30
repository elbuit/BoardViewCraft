# Tasks: Native PDF Rendering and Inspection with pdfcraft-render

- [ ] 1. Add `pdfcraft-render` dependency
  - Add `pdfcraft-render = { path = "../pdfcraft/crates/render", version = "0.15.0" }` to root `Cargo.toml`.
  - Add `pdfcraft-render = { workspace = true }` to `crates/ui-egui/Cargo.toml`.
  - Verify workspace compilation with `cargo check -p bvc-ui-egui`.

- [ ] 2. Refactor document loading & page count inspection
  - Replace `Command::new("pdfinfo")` in `crates/ui-egui/src/pdf.rs` with `pdfcraft_render::PageRenderer::new(bytes, RenderConfig::default())`.
  - Store `bytes: Arc<Vec<u8>>` in `PdfViewer` on document load.
  - Set total page count directly from `renderer.page_count()`.

- [ ] 3. Refactor page rasterization to in-process worker
  - In `crates/ui-egui/src/pdf.rs`, remove `pdftoppm` subprocess execution and tempfile generation.
  - Run `renderer.render(RenderRequest { page, scale, kind: RequestKind::Pixels, .. })` in the rasterizer background thread.
  - Convert `RenderedPage::rgba` into `egui::ColorImage::from_rgba_unmultiplied([w, h], &page.rgba)`.

- [ ] 4. Refactor text indexing for cross-probing
  - In `crates/ui-egui/src/pdf.rs`, remove `pdftotext -bbox` subprocess execution.
  - Iterate pages using `renderer.filtered_text(page_idx, TextFilter::All)` to build `PageText`.
  - Tokenize words and query bounding boxes with `page_text.word_at(...)` and `page_text.glyph_rects(...)` to populate `SchematicIndex`.

- [ ] 5. Clean up packaging & verify tests
  - Remove `poppler-tools` module from `packaging/linux/flatpak/io.github.elbuit.BoardViewCraft.bundle.yml`.
  - Run `cargo test --workspace` and verify cross-probing and PDF test fixtures pass cleanly.
