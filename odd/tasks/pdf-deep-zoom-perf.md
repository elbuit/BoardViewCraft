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
- [x] T1 — Measure deep-zoom jump cost and compare with pdfcraft (route: delegated writer; trigger: runtime measurement + reading pdfcraft prepares the write) — `cd5e00d`
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

## T1 measurements (Xeon E5-2650 v2, 7 render workers, release, headless)
Harness: `cargo test -p bvc-ui-egui --release bench_deep_zoom_search -- --ignored --nocapture`
(env `BVC_ZOOM`=18, `BVC_QUERY`=U12/R201, `BVC_PPP`, `BVC_PANEL`=WxH, `BVC_EARLY`=1). Search after
text indexing and previews are idle; times from the search call. Headless: no GPU upload is counted.

| scenario (U12, page 5) | scale px/pt | tiles (side, Mpx) | sharp | worker raster | UI convert | max frame | CPU |
|---|---|---|---|---|---|---|---|
| ppp 1, 860x900 panel | 17.5 | 2 (3677, 27.0) | 0.26 s | 480 ms | 124 ms | 137 ms | 0.66 s |
| ppp 2, 860x900 panel | 34.9 | 4 (4096, 67.1) | 0.45 s | 979 ms | 272 ms | 302 ms | 1.28 s |
| ppp 1, 1800x1000 panel | 28.7 | 1 (4096, 16.8) | 0.22 s | 456 ms | 80 ms | 88 ms | 0.56 s |

The visible region is 0.77 Mpx (ppp 1) / 3.1 Mpx (ppp 2), so the tiles rasterize 20-35x the visible pixels.

Single-worker tile cost on this fixture (pages 5 and 34, scales 17.5 and 34.9), ms per tile:
side 512: 10-22, 1024: 12-20, 2048: 17-32, 3677: 62-95, 4096: 77-117. The page interpret itself is
~10 ms; a big tile's cost is its pixels (clear + fill of up to 64 MiB), not the interpret.
pdfcraft's `page_timing` (same engine, built from the pinned rev) on the fixture: four 1024 px tiles
in 10-15 ms wall, the 2048 px backdrop in 21 ms.

Search right after open (`BVC_EARLY=1`): the jump is sharp 0.15-0.2 s after the hit is found, but
the background text indexing and page previews keep all workers busy (~6.9 s CPU, 50 rasters)
until they finish; that is document preparation, not the jump.

Findings vs pdfcraft: same `tile_side` (pdfcraft's canvas.rs `paint_page_tiles`), same tile
ordering, same backdrop and adjacent-page prefetch. The difference on the UI thread is
`texture_image`: BoardViewCraft copies every pixel (`from_rgba_premultiplied`, ~3.3 ns/px, up to
302 ms in one frame here, before the GPU upload); pdfcraft reuses the renderer's buffer zero-copy.
Dominant worker cost at deep zoom: tiles of up to 4096 px whose pixels, not interprets, cost the time.
