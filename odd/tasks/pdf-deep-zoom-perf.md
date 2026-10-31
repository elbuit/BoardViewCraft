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
- [x] T2 — Zero-copy tile/page texture conversion as pdfcraft (route: same delegated writer) — `dece3ab`; RED `test_texture_image_reuses_buffer` observed failing before the change
- [x] T3 — Fix the dominant measured cause at deep zoom (tile sizing / holds / ordering per pdfcraft) (route: same delegated writer) — `c671e9a`; RED `deep_zoom_tiles_are_no_larger_than_the_viewport_needs` and the 1024 px assertion in `test_zoomed_page_requests_only_visible_tiles_at_exact_scale` observed failing without the cap

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

## T2 results (`dece3ab`, same harness, U12)

| scenario | sharp before → after | UI convert | max frame | CPU |
|---|---|---|---|---|
| ppp 1, 860x900 | 0.26 → 0.15 s | 124 → 0.0 ms | 137 → 8 ms | 0.66 → 0.57 s |
| ppp 2, 860x900 | 0.45 → 0.19 s | 272 → 0.1 ms | 302 → 20 ms | 1.28 → 0.86 s |
| ppp 1, 1800x1000 | 0.22 → 0.14 s | 80 → 0.0 ms | 88 → 9 ms | 0.56 → 0.49 s |

## T3 results (`c671e9a`): viewport tile cap
Change: `tiles::viewport_tile_cap` caps the tile side at the power of two covering the viewport's
long side in device pixels (1024..=4096); `visible_tiles` applies it after pdfcraft's `tile_side`.
Deviation from pdfcraft (measured reason, see T1): at deep zoom pdfcraft's side reaches 4096 px
and a tile's cost is its pixels (~10 ms interpret vs 80-115 ms per 4096 px tile on this fixture).
Any viewport spans at most 2 x 2 tiles, so a heavy page still costs at most four parallel
interprets; a page shown whole is unaffected (cap does not bind).

| scenario | tiles before → after | sharp: v0.1.6 → T2 → T3 | worker raster | max frame | CPU: v0.1.6 → T3 |
|---|---|---|---|---|---|
| U12 ppp 1, 860x900 | 2x3677 (27.0 Mpx) → 4x1024 (4.2 Mpx) | 0.26 → 0.15 → 0.065 s | 480 → 399 ms | 137 → 1.4 ms | 0.66 → 0.43 s |
| U12 ppp 2, 860x900 | 4x4096 (67.1 Mpx) → 4x2048 (16.8 Mpx) | 0.45 → 0.19 → 0.089 s | 979 → 574 ms | 302 → 4.7 ms | 1.28 → 0.61 s |
| U12 ppp 1, 1800x1000 | 1x4096 (16.8 Mpx) → 4x2048 (16.8 Mpx) | 0.22 → 0.14 → 0.093 s | 456 → 481 ms | 88 → 6.2 ms | 0.56 → 0.52 s |
| R201 ppp 1, 860x900 | 1x3677 (13.5 Mpx) → 4x1024 (4.2 Mpx) | 0.16 → — → 0.077 s | 208 → 230 ms | 79 → 5.1 ms | 0.32 → 0.28 s |

The remaining worker time after the jump is the 2048 px backdrop of the hit page and of its two
neighbours (pdfcraft prefetches the adjacent rows the same way), now queued after the visible tiles.

## Remaining / follow-ups
- GPU upload cost is not measured headless; it scales with the uploaded pixels, now 6x fewer at ppp 1.
- Searching right after open: background text indexing and page previews keep the workers busy for
  several seconds (~6.9 s CPU); out of this feature's scope.
- Clippy `-D warnings` is not clean on base (bvc-core and two pre-existing pdf.rs lints); no new warnings.

## Status
- T1 `cd5e00d`, T2 `dece3ab`, T3 `c671e9a`. Next: user check in the real app at 1800 %.
- Review assess (`--base-ref 3ad7b36 --committed-only`): risk medium, review_due false (under_budget); pending in the slice.
- Native review (consent granted, lens reliability): approved and acknowledged for 3ad7b36..7ec0ff4. Advisory R3-001 (non-blocking): `test_texture_image_mismatched_size_copies` (pdf.rs:843) does not exercise the copy fallback of `texture_image`; follow-up.
- Build profiles adopted from pdfcraft (`[profile.dev.package."*"]` opt 2, dev opt 1, release thin LTO + 1 CGU): commit on this branch; dev tests 102 pass; release build 2m05s.
