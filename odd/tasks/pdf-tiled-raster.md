# Feature: pdf-tiled-raster

## Objective
Adopt pdfcraft's tiled raster cache in BoardViewCraft's PDF schematic viewer so zoomed rendering only rasterizes visible regions.

## Problem / Why
`crates/ui-egui/src/pdf.rs` rasterizes the whole page at zoom scale (up to 8192 px per side). At high zoom every re-render costs tens of megapixels while only the viewport is visible. pdfcraft (same engine, rev fa5f9f8c) switches to tiles above `TILE_THRESHOLD` (`pdfcraft/crates/ui-egui/src/canvas.rs:976`, `canvas/cache.rs`).

## Scope
- Low-res base raster per page (pdfcraft `BASE_SIDE`) always drawn as backdrop.
- Above the tile threshold, request only visible `Tile`s at the wanted scale via `RenderPool`; cache tile textures keyed by (page, scale tag, tile); evict off-scale/off-screen tiles under a byte budget; keep old tiles drawn until replacements arrive.
- Remove the 8192 px whole-page clamp for zoomed rendering.
- Out of scope: background text indexing, search changes, debounce tuning.

## Constraints
- Reuse `pdfcraft-render` API (`RenderRequest`, `Tile`, `RenderPool`) at the pinned revision; no new dependencies.
- Keep public behaviour of `PdfSchematicViewer` used by `lib.rs` (cross-probe, search highlight, pan/zoom).

## Delivery strategy
ask-on-risk. Forecast ~350-500 authored lines.

## Tasks
- [x] T1 — Pure tile planning + tile cache module with unit tests (route: delegated writer; trigger: preparation reading of pdfcraft cache, 2+ files)
- [ ] T2 — Integrate tiles into `PdfSchematicViewer` and the PDF panel drawing in `lib.rs` (route: delegated writer, same worker)

## Acceptance criteria
- At zoom where page side * scale > threshold, only tiles intersecting the viewport are requested.
- Base raster stays visible while tiles load; no blank flashes on zoom/pan.
- Search highlight and cross-probe centering still correct.
- `cargo test -p bvc-ui-egui` and `cargo clippy -p bvc-ui-egui --release` pass; `cargo build --release` succeeds.

## Checks
- `cargo test -p bvc-ui-egui`
- `cargo clippy -p bvc-ui-egui --release -- -D warnings` (if clean on base)
- `cargo build --release`

## Progress
- Branch `feat/pdf-tiled-raster` created from `3ab889b`.
- T1 done: `crates/ui-egui/src/pdf/tiles.rs` (pure tile planning + generic `TileCache`, 13 unit tests). RED observed first (12/13 failing on `todo!()` stubs), then GREEN.
  - `cargo test -p bvc-ui-egui`: 50 passed, 0 failed.
  - `cargo clippy -p bvc-ui-egui --release`: no new warnings (base is not clean: 2 pre-existing collapsible-if in pdf.rs, so `-D warnings` not applicable).
  - Commit: see `git log` (feat(pdf): add tile planning and tile texture cache).
