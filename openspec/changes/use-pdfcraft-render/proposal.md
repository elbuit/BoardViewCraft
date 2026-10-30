# Proposal: Use pdfcraft-render for PDF Rendering and Inspection

## Intent

Eliminate external process and system tool dependencies (`pdfinfo`, `pdftoppm`, `pdftotext` from `poppler-tools`) in BoardViewCraft by replacing them with the pure Rust native `pdfcraft-render` crate from the sibling `pdfcraft` project.

Currently, BoardViewCraft relies on external child processes for:
1. Document inspection & page counts (`pdfinfo`)
2. Word bounding-box tokenization for component cross-probing (`pdftotext -bbox`)
3. Page rasterization into PNG images (`pdftoppm`)

Invoking CLI subprocesses introduces overhead, cross-platform packaging friction (e.g. bundling poppler binaries in Flatpak/Windows/macOS), and fragile process pipe communication. Transitioning to `pdfcraft-render` provides in-process, pure Rust memory rendering and text indexing.

## Scope

### In Scope
- Add `pdfcraft-render` dependency (or sibling reference/vendoring) to `crates/ui-egui`.
- Replace `pdfinfo` document metadata and page count retrieval with `pdfcraft_render::inspect(...)`.
- Replace `pdftoppm` rasterization background threads with in-process `PageRenderer` / `RenderPool` from `pdfcraft-render` rendering directly into raw pixel buffers (`Pixels`).
- Replace `pdftotext -bbox` XML parsing with `pdfcraft_render::PageText` native text extraction and token bounding boxes.
- Simplify packaging manifests (e.g. remove poppler CMake building in Flatpak bundle).
- Maintain existing cross-probing behavior, adaptive zoom, centering, and UI toolbar controls.

### Out of Scope
- Porting editing, annotation, or cryptographic signing features from `pdfcraft`.
- Rewriting BoardViewCraft's CAD/boardview parsing engine.
- Adding PDF export capabilities to BoardViewCraft.

## Capabilities

### New Capabilities
- None

### Modified Capabilities
- `pdf-schematic-auto-prompt`: Updates the underlying engine for PDF metadata inspection, rasterization, and text indexing to pure Rust `pdfcraft-render` without external CLI tools (`pdfinfo`, `pdftoppm`, `pdftotext`).

## Approach

1. Configure workspace dependencies to link `pdfcraft-render` (via path `../pdfcraft/crates/render` or shared library repository).
2. Refactor [`crates/ui-egui/src/pdf.rs`](file:///media/no_raid/devel/boarview/BoardViewCraft/crates/ui-egui/src/pdf.rs):
   - Replace `Command::new("pdfinfo")` with `pdfcraft_render::inspect(&bytes)`.
   - Replace `Command::new("pdftoppm")` with `pdfcraft_render::PageRenderer` or `RenderPool` rasterizing to RGBA pixel arrays, then load into `egui::ColorImage`.
   - Replace `Command::new("pdftotext")` with `PageText` tokenization to populate `SchematicIndex` with component coordinates and page rects.
3. Remove poppler external binary checks or fallbacks in `packaging/linux/flatpak/`.
4. Run regression tests against fixture schematics to verify cross-probing, zoom, and raster accuracy.

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `Cargo.toml` | Modified | Add `pdfcraft-render` dependency |
| `crates/ui-egui/Cargo.toml` | Modified | Reference `pdfcraft-render` |
| `crates/ui-egui/src/pdf.rs` | Modified | Replace subprocess invocation with native `pdfcraft-render` calls |
| `packaging/linux/flatpak/io.github.elbuit.BoardViewCraft.bundle.yml` | Modified | Remove `poppler-tools` module and dependencies |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Dependency coupling between sibling local workspaces | Med | Use git revision/path with fallback or vendoring, aligning with `craft-libs` convention |
| Performance difference between native rasterizer and pdftoppm | Low | `pdfcraft-render` uses `hayro` and multithreaded `vello_cpu` optimized with SIMD |
| Coordinate space discrepancy in bounding boxes | Med | Map PDF point coordinates directly to normalized canvas coordinates, covered by focused unit tests |

## Rollback Plan

Revert git commit on `main` to restore `poppler-tools` subprocess execution in `crates/ui-egui/src/pdf.rs`.

## Dependencies

- `pdfcraft-render` (v0.15.0 from `pdfcraft`)
- Rust 1.95+

## Success Criteria

- [ ] All PDF rasterization executes entirely in-process without spawning `pdftoppm`.
- [ ] Text indexing and cross-probe token search function without spawning `pdftotext` or `pdfinfo`.
- [ ] Cross-probing a component from boardview centers and zooms properly in the schematic.
- [ ] `cargo test --workspace` passes without requiring `poppler-utils` installed on the host.
