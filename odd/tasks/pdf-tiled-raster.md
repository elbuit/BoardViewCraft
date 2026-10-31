# Feature: pdf-tiled-raster

## Objective
Adopt pdfcraft's tiled raster cache in BoardViewCraft's PDF schematic viewer so zoomed rendering only rasterizes visible regions.

## Problem / Why
`crates/ui-egui/src/pdf.rs` rasterizes the whole page at zoom scale (up to 8192 px per side). At high zoom every re-render costs tens of megapixels while only the viewport is visible. pdfcraft (same engine, rev fa5f9f8c) switches to tiles above `TILE_THRESHOLD` (`pdfcraft/crates/ui-egui/src/canvas.rs:976`, `canvas/cache.rs`).

## Scope
- Low-res base raster per page (pdfcraft `BASE_SIDE`) always drawn as backdrop.
- Above the tile threshold, request only visible `Tile`s at the wanted scale via `RenderPool`; cache tile textures keyed by (page, scale tag, tile); evict off-scale/off-screen tiles under a byte budget; keep old tiles drawn until replacements arrive.
- Remove the 8192 px whole-page clamp for zoomed rendering.
- Out of scope (T1-T2): background text indexing, search changes, debounce tuning. T3 reopens open/render latency after user report on 2026-10-31.

## Constraints
- Reuse `pdfcraft-render` API (`RenderRequest`, `Tile`, `RenderPool`) at the pinned revision; no new dependencies.
- Keep public behaviour of `PdfSchematicViewer` used by `lib.rs` (cross-probe, search highlight, pan/zoom).

## Delivery strategy
ask-on-risk. Forecast ~350-500 authored lines.

## Tasks
- [x] T1 — Pure tile planning + tile cache module with unit tests (route: delegated writer; trigger: preparation reading of pdfcraft cache, 2+ files)
- [x] T2 — Integrate tiles into `PdfSchematicViewer` and the PDF panel drawing in `lib.rs` (route: delegated writer, same worker)
- [x] T3 — Fix slow PDF open (all cores at 100% for a long time) and slow render of the page a search/cross-probe jumps to; user-reported with `tmp/MS-V375 REV 7.0.pdf` + its `.cad` (route: delegated writer; trigger: runtime diagnosis needs a measurement harness + likely multi-file fix)

- [x] T4 — Slow display of the target page after a search hit / cross-probe jump (user-reported 2026-10-31 after T3; headless T3 run measured 0.15-0.31 s and did not reproduce). Hypothesis: new page has no texture (cache keeps only current±1), so it stays blank until a 2048-px backdrop plus high-zoom tiles finish; pdfcraft shows cached thumbnails/previews instantly (route: delegated writer; trigger: runtime measurement + multi-step fix)

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
  - Commit: `7628b60` feat(pdf): add tile planning and tile texture cache for zoomed schematics.
- T2 done: `PdfSchematicViewer::paint_current_page` replaces `ensure_current_page_texture`; `lib.rs` lays out the page and delegates painting.
  - Raster scale is the exact display scale (`page_rect.width() * ppp / page_width_pt`, includes the panel fit factor); the 8192 px whole-page clamp is gone.
  - Past the threshold: low-res backdrop (preview) first, then only viewport tiles nearest-centre first; old-zoom tiles drawn stretched until replaced; queue replaced only when it changes (pool cancels obsolete in-flight work); results not in the last queue are dropped.
  - Tests: 2 viewer-level fixture tests (written after implementation, no RED) asserting only on-screen tiles at the unclamped scale, and whole-page rendering for small layouts.
  - `cargo test -p bvc-ui-egui`: 52 passed, 0 failed.
  - `cargo clippy -p bvc-ui-egui --release`: no new warnings (pdf.rs 2 -> 1 pre-existing, lib.rs 15 unchanged).
  - `cargo build --release`: success.
  - GUI behaviour (no blank flashes, highlight/centering visually) not verified at runtime: manual check pending.
  - Commit: `06990a0` perf(pdf): render zoomed pages as visible tiles for faster search navigation.
- Parent spot check: `cargo test -p bvc-ui-egui --release` -> 51 passed, 1 failed (`canvas::tests::bench_dense_net_tessellation`, untouched timing bench; passes when run alone in release and debug — flaky under parallel CPU load from the new fixture tests).
- Review assessment (`--base-ref 3ab889b --committed-only`, untracked `.atl/` excluded): risk medium (`executable_change` lib.rs), `review_due=false`, reason `under_budget`.

- T3 done (route: delegated writer; trigger: runtime diagnosis needed a measurement harness).
  - Measurement: ignored bench `pdf::tests::bench_open_and_cross_probe` (headless egui frames, fixture `tmp/MS-V375 REV 7.0.pdf`), plus a temporary, uncommitted stderr hook in the real GUI (release, ppp 1.5, max texture 8192) with `pidstat`.
  - Root cause: `open_file` extracted the text of all 42 pages eagerly on the UI thread with rayon, building a full `PageRenderer` (full parse) per page. Text extraction is far costlier than rasterizing on this PDF (page 9: 0.72 s, page 4: 0.48 s single-thread vs 13-84 ms per raster), so the UI froze while every core worked. The render queue was not the problem: no thrash measured (2 queue changes, 0 dropped results to first paint; cross-probe 2-7 changes, 0 dropped; no repeated cross-probe from `lib.rs`).
  - Before: `open_file` 0.66 s headless / 0.86 s in the GUI (UI blocked, 302%+193% CPU samples), first page on screen ~0.93 s after open in the GUI.
  - Fix: no eager text. Text layers go through the `RenderPool` as `RequestKind::Text` (one shared parser), queued behind every raster, current page first, at most half the workers at once; pool workers = cores clamped to 2..8 minus one (as pdfcraft). An active search refreshes as texts arrive, keeping the selected hit; a cross-probe whose designator is not indexed yet jumps to its first hit when it arrives. Re-opening clears the active search so the selected part is re-probed.
  - After: `open_file` 0.005 s headless / 0.003 s GUI; first page painted 21 ms (ppp 1) / 49 ms (ppp 2) headless and at the next frame in the GUI; all 42 texts indexed in the background in ~1.2 s on 3 workers (one parser build, 42 text interpretations, 268% CPU sample, UI responsive). Cross-probe to page 5 after indexing: painted 0.16 s (unchanged; GUI 0.15 s before, 0.24 s after, frame-timing noise). Cross-probe issued right after open: jumps at 0.15 s, page painted at 0.31 s with 6/42 pages indexed.
  - pdfcraft reference (`page_timing`, 7 threads): page 1 fit 47 ms, backdrop 31 ms, text 6 ms; BoardViewCraft's first paint now matches it.
  - Regression tests (deterministic, no wall-clock bounds): open defers text and queues the visible raster first with at most `text_slots` text requests behind it; whole-document indexing uses 1 parser build, exactly one text interpretation per page, <= 2 page interpretations, 0 dropped results; a cross-probe before indexing jumps when its hit arrives. `test_ms_v375_instant_search_multiple_occurrences` used a cwd-relative path and was silently skipped; it now uses the manifest-relative fixture and waits for indexing. RED not observed separately (the old `open_file` filled `page_texts` eagerly, which the first test rejects).
  - `cargo test -p bvc-ui-egui`: 55 passed, 0 failed, 1 ignored.
  - `cargo test -p bvc-ui-egui --release`: 55 passed, 0 failed, 1 ignored.
  - `cargo clippy -p bvc-ui-egui --release`: no new warnings (pdf.rs 1 and lib.rs 15, as on base).
  - `cargo build --release`: success.
  - Commit: `66b2fb2` perf(pdf): index schematic text in the render pool instead of blocking open.
  - Not reproduced: a slow cross-probe render on this machine (0.15-0.31 s measured). If it persists for the user, capture the zoom level and display scale when it happens.

- T4 done (route: delegated writer; trigger: GUI runtime measurement + multi-step fix).
  - Measurement (GUI path): temporary, uncommitted stderr trace in `paint_current_page`/`receive` plus a scripted driver in `lib.rs` (env-gated: opens the PDF, then `select_match` / `cross_probe` / zoom steps 3 s apart, or 0.3 s apart right after open), release binary with the `.cad`, ppp 1.5, panel ~415 pt, 16 cores (15 workers) and `taskset -c 0-3` (3 workers). Removed before commit.
  - Single render costs (1 worker, release): rendering is interpret-bound, not pixel-bound. Heavy pages (p3/p5/p9): 320 px thumb 52-68 ms, 640 px preview 70-73 ms, 2048 px backdrop 79-93 ms, whole page at zoom 3 90-94 ms, one 1024 px tile at zoom 8 22-25 ms; text p9 558 ms. A cheap preview rendered on demand is NOT cheaper than the sharp page, so it only helps when prefetched.
  - Root cause: (1) a jumped-to page had no texture at all (cache kept only current+-1), so the panel showed "loading page" until the sharp raster finished (92-173 ms here, a page interpret; proportionally longer on slower CPUs); (2) settle bug: a jump that changes the display scale (cross-probe zoom elevation, page of another size, panel width change) settled on the jump frame via `force_settle`, but on the next frame `settled` was false for SETTLE_SECS, so the visible tiles were dropped from the queue (in-flight ones cancelled) and re-queued 120 ms later: tiled jump tiles arrived at 125-165 ms instead of ~25-60 ms; (3) on a jump, background text jobs kept their workers (up to 0.6 s per job), which on 2-3 core machines can block the only free worker.
  - Before (GUI, after indexing, zoom 8 tiled): first paint 92-173 ms, sharp tiles 125-165 ms; zoom 1-3 whole page: first paint 19-159 ms; during indexing (4 cores) cross-probe painted at 126-167 ms.
  - Fix (mirrors pdfcraft thumbnails + page cache + "visible mains pending" hold): 640 px previews of every page prefetched in the background (search-result pages first, before text; the rest after text; 64 MiB budget, 8 queued at once), drawn when the page has no sharp raster; sharp page rasters kept nearest-first within 128 MiB / 24 pages instead of +-1; while the current page has no raster at all, text and previews are not queued (every worker goes to the target page); with a preview on screen, visible tiles are queued before the backdrop; `settled` is also true when the wanted scale equals the settled one.
  - After (GUI): every jump paints on the jump frame (0-1 ms, preview or cached raster), including a cross-probe 0.74 s after open with 3/42 pages indexed on 3 workers (previews of the first pages were already prefetched); tiled jumps sharp at 61-85 ms (tiles first); whole pages at zoom 3 sharpen at 123-188 ms behind the preview; revisited pages show their cached sharp raster at once.
  - Regression tests (deterministic, no wall-clock thresholds): `test_jump_paints_preview_at_once_and_holds_background_work`, `test_visited_page_keeps_sharp_raster`, `test_cross_probe_into_tiles_keeps_tiles_queued`, `test_previews_prefetched_with_result_pages_first`, `test_admit_by_budget_skips_what_does_not_fit`; `test_open_defers_text_and_queues_visible_page_first` now expects background work held until page 1 is on screen; the shared-parser test allows one preview per page. RED by mutation: reverting the settle fix, the preview paint fallback or the page cache makes the three viewer tests fail. Frame-driven fixture tests are serialized (`FIXTURE_SERIAL`): run in parallel in a debug build they pushed text pages past the pool's 20 s stuck watchdog.
  - `cargo test -p bvc-ui-egui`: 60 passed, 0 failed, 1 ignored.
  - `cargo test -p bvc-ui-egui --release`: 60 passed, 0 failed, 1 ignored.
  - `cargo clippy -p bvc-ui-egui --release`: no new warnings (pdf.rs 1, lib.rs 15, as on base).
  - `cargo build --release`: success.
  - Commit: `920a7a3` perf(pdf): show schematic pages instantly after search and cross-probe jumps.
  - Not verified: behaviour on the user's machine/display (slower CPU, larger panel or ppp 2 where zoom 3 is tiled); memory cost of previews (~49 MB for this 42-page document, capped at 64 MiB).

## Next step
Manual GUI check: zoom/pan on a large schematic (no blank flashes, no tile seams), search highlight and cross-probe centering; confirm open latency and CPU with the user's flow.
