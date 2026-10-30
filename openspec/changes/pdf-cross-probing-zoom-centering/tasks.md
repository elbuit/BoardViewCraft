# Tasks: PDF Cross-Probing Zoom and Centering

## Phase 1: Implementation
- [ ] 1.1 In `crates/ui-egui/src/pdf.rs`, update `PdfSchematicViewer::cross_probe` to set `self.zoom = 2.0` when `self.zoom <= 1.0` and preserve `self.zoom` when `self.zoom > 1.0`.
- [ ] 1.2 Verify centering calculation in `crates/ui-egui/src/lib.rs` correctly applies `self.pdf_viewer.zoom` in `effective_scale` for `pan_offset`.

## Phase 2: Verification
- [ ] 2.1 Add unit tests for `cross_probe` zoom escalation under both `<= 1.0` and `> 1.0` conditions.
- [ ] 2.2 Run `cargo test --workspace` and verify the entire test suite passes without regressions.
