# Feature: pdf-deep-zoom-perf

## Objective
Make the schematic PDF show the visible region quickly and with low CPU at very deep zoom (e.g. 1800 %), using pdfcraft as the reference implementation for PDF rendering and handling.

## Problem / Why
User report (2026-10-31, v0.1.6): a component search at 1800 % PDF zoom finds the hit at once, but the region takes long to render and CPU usage is high.
Unmeasured hypotheses from code reading:
- `tiles::tile_side` (`crates/ui-egui/src/pdf/tiles.rs:51`) reaches 4096 px at scale >= 24 px/pt (1800 % * 96/72), so each tile is ~16.7 Mpx / 64 MiB while the PDF panel shows ~1 Mpx; 2-4 tiles per viewport means 30-60x the visible pixels rasterized.
- `texture_image` (`crates/ui-egui/src/pdf.rs:58`) copies every pixel through `ColorImage::from_rgba_premultiplied` on the UI thread; pdfcraft converts zero-copy with `bytemuck::try_cast_vec` (`pdfcraft/crates/ui-egui/src/canvas/cache.rs:711`).
- Background text indexing may compete for the same pool workers right after open.

## Reference
pdfcraft at the pinned rev fa5f9f8c: `~/.cargo/git/checkouts/pdfcraft-f9365ca77a7f2011/fa5f9f8/crates/ui-egui/src/canvas.rs`, `canvas/cache.rs`. Adopt its approach unless a measurement shows a reason not to.

## Scope
- Measure the deep-zoom search jump (raster time per tile, UI-thread conversion/upload time, CPU) on `tmp/MS-V375 REV 7.0.pdf`.
- Zero-copy texture conversion as pdfcraft.
- Compare pdfcraft's deep-zoom behaviour (tile sizing, heavy-page holds, request ordering) and adopt what makes the difference.
- Out of scope: board canvas, search semantics.

## Constraints
- No new dependencies beyond what pdfcraft already uses (`bytemuck` if already in the tree).
- Keep `PdfSchematicViewer` behaviour used by `lib.rs`.

## Delivery strategy
ask-on-risk. Forecast ~150-350 authored lines.

## Tasks
- [ ] T1 — Measure deep-zoom jump cost and compare with pdfcraft (route: delegated writer; trigger: runtime measurement + reading pdfcraft prepares the write)
- [ ] T2 — Zero-copy tile/page texture conversion as pdfcraft (route: same delegated writer)
- [ ] T3 — Fix the dominant measured cause at deep zoom (tile sizing / holds / ordering per pdfcraft) (route: same delegated writer)

## Acceptance criteria
- At 1800 % after a search jump, the visible region appears markedly faster than on v0.1.6, with before/after numbers recorded here.
- CPU time spent for the jump is reduced, with numbers recorded.
- Tests pass; no regressions in tiled rendering tests.

## Checks
- `cargo test -p bvc-ui-egui --release`
- `cargo clippy -p bvc-ui-egui --release -- -D warnings` (if clean on base)
- `cargo build --release`

## Progress
- Branch `feat/pdf-deep-zoom-perf` from `main` (3ad7b36, v0.1.6).
