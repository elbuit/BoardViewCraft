# Feature: board-tabs-layout

## Objective
Replace the main area with tabs: **Top**, **Bottom**, **Both** and **PDF**.
- Top / Bottom: one board view of that side.
- Both: two separate, non-overlapping views (top side and bottom side). Layout: Vertical (top above bottom), Horizontal (top left, bottom right) or Auto (rotated board wider than tall -> vertical; otherwise horizontal).
- PDF: the schematic viewer, no longer a side split panel.
- Only the active tab renders; board meshes are cached per side so switching tabs (e.g. Top -> Both) reuses them.

## Problem / Why
User request 2026-11-01. Today one canvas shows one side (or both overlapped) and the PDF shares the window as a left split panel. The user asked not to be asked anything until it is implemented, so design defaults below were chosen by the orchestrator.

## Design decisions (orchestrator defaults)
- Tab bar at the top of the central area. The Top/Bottom/Both side ComboBox in the toolbar is removed (tabs replace it). The status label shows the active tab.
- Both layout selector (Auto / Vertical / Horizontal) shown in the Both tab's header and persisted in Preferences (`#[serde(default)]`, default Auto). Active tab persisted too (default Top). PDF tab falls back to Top when no PDF is loaded at startup.
- Two `CanvasState`s, one per side (top, bottom), each with its own viewport (pan/zoom) and mesh cache. Rotation and mirror are shared: R / M apply to both. `rect_size` leaves `ViewCacheKey` if needed so the same side reuses meshes at a different canvas size (remap/coverage decide).
- Keyboard pan/zoom, overlay wheel and the pin context menu act only on the active canvas (the one last hovered/clicked). Mouse wheel/drag already check hover.
- Overlay photo editing works on the canvas of the overlay's side.
- Space switches Top <-> Bottom tab (from Both/PDF: Top). F fits the visible canvas(es).
- Focus on a part (search, PDF click): in Top/Bottom, switch to the tab of the part's side if not visible; in Both, center the canvas(es) where the part is visible. A PDF click selects/centers on the board but stays on the PDF tab.
- PDF tab without a document shows an "Open PDF" button. PDF text indexing keeps running while its tab is hidden (per-frame pump), so board -> PDF cross-probe works from any tab and the PDF tab opens at the hit.

## Tasks
- [x] T1 — Two per-side canvases: `show_canvas` in an explicit rect, input gated to the active canvas, per-side mesh cache reused across tab sizes, shared rotation/mirror, cache resets for both (route: delegated writer; canvas.rs + lib.rs)
- [x] T2 — Tabs Top/Bottom/Both/PDF with Both layout Auto/Vertical/Horizontal, preferences, i18n, PDF moved into its tab with a hidden-tab indexing pump, focus/cross-probe adapted (route: same delegated writer; lib.rs + pdf.rs + preferences.rs + i18n)
- [ ] T3 — README update for the tabs (route: inline)

## Acceptance criteria
- Each tab shows only its content; Both shows two separated views whose arrangement follows the layout choice (Auto by board aspect after rotation).
- Switching Top -> Both -> Top does not re-tessellate the top side meshes when the view is unchanged (cache test).
- A board part click cross-probes the PDF while another tab is active; the PDF tab shows the hit when opened.
- `cargo test -p bvc-ui-egui --release` and `cargo test -p bvc-core --release` pass; no new clippy warnings (147 base); `cargo build --release` ok.

## Delivery strategy
exception-ok (user asked for no questions until done). Forecast ~800 authored lines.

## Progress
- Branch `feat/board-tabs-layout` from `834b54a` (stacked on feat/board-side-both-system-language). Exploration done.
- T1 done (delegated writer). `BoardCanvases` in canvas.rs holds a top and a bottom `CanvasState` (own pan/zoom/mesh cache, shared rotation/mirror via `rotate_*`/`toggle_mirror_*`/`reset_orientation`, `invalidate_caches`/`clear_board` replace every single-canvas reset) plus the `active` side (last hovered/clicked). `show_canvas(ui, rect, is_active, ...)` draws in an explicit rect with a stable per-side id; keyboard pan/zoom, overlay wheel and the pin context menu only act on the active canvas. `rect_size` left `ViewCacheKey`; a scale-preserving re-map (pan or another canvas position/size) is now treated as exact (`BoardMeshBase::is_exact_for`), so the same side shown at another size remaps once and is kept, never re-tessellated. Interim UI: the side ComboBox picks Top/Bottom/Both (Both = two canvases side by side) until T2 brings the tabs. Focus helpers `focus_point`/`focus_part` center the canvases that can show the item, switching side when needed. F/View fit use the real canvas rects of the last frame.
  - RED: `same_side_at_a_different_canvas_size_reuses_its_meshes` failed (`Top -> Both`: Rebuilt instead of Remapped) before the key/settle change; `board_canvases_share_rotation_and_mirroring` and `a_hidden_side_keeps_its_mesh_cache` failed to compile (no `BoardCanvases`). GREEN after the change.
  - Checks: `cargo test -p bvc-ui-egui --release` 120 passed, 2 ignored; clippy warnings 142 (base 147), none new in touched code.
  - Commit `8d468ee` refactor(ui): split the board view into per-side canvases.
- T2 done (same delegated writer). New `board_tabs.rs`: `BoardTab {Top, Bottom, Both, Pdf}` (`sides`, `flipped` for Space, `at_startup` PDF->Top fallback) and `BothLayout {Auto, Vertical, Horizontal}` with the pure `both_rects`/`canvas_rects` (Auto: rotated bbox width >= height -> vertical stack; `BOTH_GAP` 6 px with a separator line). Tab bar (selectable labels) at the top of the central area, Both layout ComboBox in it while Both is shown; `Preferences.board_tab`/`both_layout` (`#[serde(default)]`), saved at end of frame via `save_prefs_pending`. Side ComboBox and its tests removed. PDF split panel moved into `pdf_tab.rs::show_pdf_tab` (returns open/close requests and the clicked token); no document -> "Open PDF" button; Actions item is "Open Schematic PDF..." or "Show Schematic PDF" (switches to the tab); a PDF opened from the dialog or by drag and drop switches to the PDF tab, the remembered/prompted reopen on board load does not. `PdfSchematicViewer::pump_background` (text requests only, no rasters) runs every frame while a document is loaded and its tab is hidden; `is_open` removed, board -> PDF cross-probe gated on `is_loaded()`. Focus: Top/Bottom tab switches to the part's side; Both centers only the canvases showing it; PDF tab (and a schematic click from any tab) centers without switching. Overlay adjust/photo load makes the overlay side's canvas visible and active. Status label shows the translated tab. i18n: removed `toolbar.view_*` (4) and `pdf.hide`; added `tabs.*` (5), `layout.*` (5), `pdf.show`, `pdf.no_document`; 158 keys in all six catalogs.
  - RED: `board_tabs` layout tests failed with `todo!()` stubs (4 failed: auto/rotation/fixed/each-tab); `test_background_pump_indexes_text_without_painting` failed ("pump made no progress" after 120 s) with a no-op pump. GREEN after implementing. App-level tab/focus tests (`tab_tests`) written with the implementation.
  - Checks: `cargo test -p bvc-ui-egui --release` 131 passed, 2 ignored; `cargo test -p bvc-core --release` all passed; clippy warnings 139 (base 147); `cargo build --release` ok. GUI not run.
  - Commit `73e03b0` feat(ui): board tabs with separated top/bottom layout and PDF tab.
  - Known limitation: a side first shown at a different canvas size keeps its zoom (e.g. Top -> Both shows the board larger than its half); F fits the visible canvases. Actions -> Reset view now resets rotation/mirror only and keeps the tab.
