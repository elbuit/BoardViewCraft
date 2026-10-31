# Feature: trace-style-board-render

## Objective
Render boards the way trace-boardviewer does (filled part bodies, real rotated pads, outlines hugging pads), starting with GenCAD (`tmp/MS-V375 REV 7.0.cad`).

## Problem / Why
User likes trace-boardviewer's view (`../trace-boardviewer`, MIT, Canvas2D). BoardViewCraft draws every part as an unfilled bbox outline and every pin as a fixed-radius circle; the model has no pad size/shape/rotation or part outline, and the GenCAD parser uses a fixed pin radius 5.0 and ignores `$PADS` and shape artwork.

## Scope (user-approved steps 1, 2, 4, 6 of the plan)
- Model: `PadShape`, pad width/height/rotation on `Pin`; outline/rotation on `Part` (defaults keep other parsers compiling).
- Pad-aware part bounding box (port of `padExtent`, trace `src/lib/formats/common.ts`).
- GenCAD: parse `$PADS`, resolve padstack -> pad, apply pin and component rotation/mirror/flip, stitch shape artwork into an outer contour (port of trace `src/lib/gencad.ts`).
- Renderer: filled part bodies, rotated rect pads above bodies, palette from trace `src/components/board-palette.ts`.
- Out of scope now: part-kind classifier (single body colour until then), KiCad/TVW parsers, label backings, selection glow.

## Constraints
- Ported code keeps the MIT notice of "TRACE Boardviewer contributors".
- Units: BoardViewCraft uses mils; trace uses mm.
- No new dependencies unless necessary.

## Delivery strategy
ask-on-risk. Forecast ~900-1300 authored lines (> 400 budget: chain strategy to be asked at PR time).

## Tasks
- [x] T1 — Model fields + pad-aware part bbox (route: delegated writer; multi-file)
- [x] T2 — GenCAD pads and part outlines (route: same writer)
- [x] T3 — Renderer: filled bodies, rotated pads, palette (route: same writer)

- [x] T4 — Fix slow board interaction after T3 (user report 2026-10-31: board opens with trace look but is very slow); measure in the real GUI path, then fix (route: delegated writer; runtime diagnosis + likely multi-file)

- [ ] T5 — Zoom is still slow (user report 2026-10-31 after T4: opening fast, zooming not); measure zoom in the real GUI at several levels and fix, e.g. world-space cached meshes transformed per frame instead of re-tessellating every zoom step (route: delegated writer)

## Acceptance criteria
- On `tmp/MS-V375 REV 7.0.cad`: pins carry real pad width/height/rotation; parts carry outlines; PCIe edge connector fingers render as long thin rectangles at true size inside a filled body.
- Other formats render as before (round pins), tests pass.
- `cargo test --workspace --release` green (except known flaky bench), no new clippy warnings, `cargo build --release` ok.

## Progress
- Branch `feat/trace-style-board-render` created from `0d78513` (stacked on feat/board-search-focus-about).

### T1 evidence
- Commit: `f89edac` feat(core): carry pad geometry and part outlines in the board model.
- `PadShape`, `Pin::{shape,width,height,rotation_deg}`, `Part::{outline,rotation_deg}`; `Pin`/`Part`/`Point`/`Side`/`PartType` derive `Default`; all struct literals got `..Default::default()` (incl. test-only sites in `bvc-core/src/lib.rs` and `spatial.rs`, user-approved).
- `board::compute_part_bounds`: outline bounds, else union of rotated pad extents (port of trace `padExtent`), sizeless pads keep the 6-mil margin (other formats unchanged).
- Test-first exception: new function, so RED not observed separately; `rebuild_indices_uses_pad_aware_bounds` would have produced +/-6 on the base.
- `cargo test --release -p bvc-core --lib board::tests`: 7 passed. `cargo test --workspace --release`: all ok (71 passed, 1 ignored in ui-egui). Clippy per-file warning counts identical to base `0d78513`.

### T2 evidence
- Commit: `9a89425` feat(gencad): read pad shapes and component outlines.
- New `parser/gencad_geometry.rs` (port of trace `transform`, `geometry`, `outerContour`, MIT notice); `gencad.rs` parses `$PADS`, stack pads (layer/rot/mirror), shape artwork, and resolves pin -> padstack -> pad with trace's layer selection and transform chain (stack pad -> shape pin -> component).
- Exact RECTANGULAR/SQUARE pads keep local size plus the placed angle; ROUND+CIRCLE pads get diameter; others (FINGER, multi-path) use the placed axis-aligned bounds (as trace). Sizeless pads keep radius 5 and no size.
- Part outline = outer artwork contour, else trace fallback (body bounds + 0.2 mm rectangle); part bbox = body points (artwork + pad rects) as in trace. Fixture has no shape artwork, so all outlines are the fallback rectangle.
- RED observed first: `tests/gencad_pads.rs` 3 of 4 failed before the parser change (positions test passed). GREEN after: 4/4.
- Fixture stats: 7974 pins, 96 sizeless; rect rotations 0/90/180/270 (+ a few 135/225); CN1 fingers 27.559 x 153.937 mil.
- Deviations: pin position stays the transformed shape PIN position (trace uses the pad geometry centre; equal for centred pads, < 0.001 mil off on the fixture); `$ARTWORKS` references are not expanded (trace does not either); `$BOARD` outline unchanged (out of scope); lenient parsing (bad rows skipped, trace fails).
- `cargo test --workspace --release`: all ok. Clippy per-file counts identical to base.

### T3 evidence
- `canvas.rs`: part bodies filled (outline polygon, lyon via per-thread tessellator, convex-quad fast path) + stroked (0.95 px, selected 2 px in `selection`), drawn before vias/pads; the 18 px skip is replaced by trace's tiny-part marker (< 3 px -> >= 2.6 px rect). Pads with a size draw as rotated rects / true-size circles / equal-area points below 1.5 px (trace `appendPadPath`, markers 1 px ordinary, 2.2 px net/hover); sizeless pins keep the old LOD circle path (other formats unchanged). Hover, net highlight (filled) and opposite-side net outline work for rect pads too.
- `render.rs`: `append_rotated_rect_to_mesh`, `append_rotated_rect_outline_to_mesh`, `append_closed_polyline_to_mesh`, `append_polygon_fill_to_mesh`; removed the now-unused `append_rect_outline_to_mesh`.
- `theme.rs`: new `part_body_line` token (serde default for saved custom themes); Cadcraft/Dark use trace dark chip/chipLine/pad, Light uses trace light; Warm/SweetCold keep their pin colour, get opaque bodies.
- Test-first exception: renderer behaviour has no prior runnable RED (visual); tests added with the change: ScreenPad rotation/view, small sizes, CN1 fingers tall on screen, filled bodies present, render helper tests.
- Mesh build timing (`bench_fixture_mesh_build`, release, 1920x1080, median of 11; 3 runs): before (T2 renderer) fit 2.0-2.9 ms / zoom8 0.48-0.50 ms; after fit 2.8-3.8 ms / zoom8 0.62-0.74 ms (~+30-50%, still cached by `ViewCacheKey`).
- `cargo test --workspace --release`: all ok (ui-egui 77 passed, 1 ignored). `cargo clippy --workspace --release`: warnings identical by file+message to the previous commit, per-file counts identical to base. `cargo build --release`: ok.
- Not verified: visual appearance in the GUI (no screen access); custom-theme editor has no picker for `part_body_line` (needs a new i18n key; i18n.rs out of surface).
- T3 commit: `3180acf` feat(ui): draw filled part bodies and real pad shapes.
- Parent spot check: `cargo test --workspace --release` all green (bvc-core 22+4+18, bvc-ui-egui 77 passed, 1 ignored).
- Review assessment (`--base-ref 0d78513 --committed-only`): risk high (`process_boundary` signal in crates/bvc-core/src/lib.rs; that file only got test-only `..Default::default()`), review due. Consent relayed to user: declined for this candidate (`declined_this_candidate`). `.atl/` added to local `.git/info/exclude` to unblock the preflight.

### T4 evidence
- Measured in the real GUI (release `boardviewcraft` on the Wayland desktop, window maximized 2560x1346, `tmp/MS-V375 REV 7.0.cad`, user preferences) with a temporary env-gated hook in `BoardViewApp::ui` (`raw_input_hook` injecting pointer/drag events; per-frame `frame.info().cpu_usage`, ui time split into side panel and canvas, mesh builds, vertices, wall interval). Hook removed before commit. Phases at 1x/3x/8x/20x of the fit scale, centred on the densest part: idle, hover sweep, drag pan.
- Root cause: not the new renderer. The canvas cost was 0.3-4 ms/frame (mesh build 1-3 ms, 36-42k vertices, 4-5 meshes, egui tessellation negligible). The right-hand Search panel laid out every part and every net as a `selectable_label` each frame (non-virtualised `ScrollArea`): 10-17 ms/frame, so frames took 12-20 ms CPU and the wall interval was 17-34 ms (missing vsync, ~30 fps). The same cost exists on base `0d78513` (side panel 11-17 ms/frame there too), it just became noticeable. Second finding: hover was part of `ViewCacheKey`, so every pin/via under the moving mouse rebuilt the whole board mesh (32-82 rebuilds per 80-120 hover frames, 2-3 ms each), and cache hits deep-copied every mesh into the painter.
- Fix: `side_panel.rs` search lists are virtualised with `show_rows` over a `SearchCache` (matches recomputed only when the filter text or board changes; invalidated on load). `canvas.rs`: hover removed from `ViewCacheKey` (built by `ViewCacheKey::new`) and drawn by a small per-frame `hover_overlay_mesh`; cached meshes are `Arc<Mesh>` so emitting them is a refcount bump.
- Numbers (maximized, before -> after): side panel 10.1-17.1 -> 0.54-0.69 ms/frame; frame CPU (`cpu_usage`) idle 18.4 -> 2.9 ms, hover 13.7-18.0 -> 2.0-3.0 ms, pan 19.0-20.0 -> 3.7-6.6 ms; wall interval 17-34 -> 16.7 ms (vsync-bound 60 fps); hover rebuilds 22-42 -> 0 per 80 frames. Pan still rebuilds (1.2-3.1 ms) and needs no world-space cache at this cost.
- Tests (no wall-clock thresholds): `view_cache_key_is_stable_across_idle_and_hover_frames`, `hover_overlay_is_small_and_separate_from_board_meshes`, `fixture_fit_view_stays_within_vertex_budget` (<= 12 vertices per pin, <= 8 meshes), `search_filters_parts_and_nets_case_insensitively`, `search_cache_recomputes_only_on_change`. Test-first exception: the slowness is a frame-time property found by GUI measurement; the regression tests encode the fix's invariants and were written with it.
- `cargo test --workspace --release`: all ok (ui-egui 82 passed, 1 ignored). Clippy per-file counts for ui-egui identical to `d8db4f0` (canvas 22, lib 13, side_panel 7).
- T4 commit: `a504ad8` perf(ui): virtualise search lists and keep hover out of the mesh cache.

## Next step
Manual GUI check on `tmp/MS-V375 REV 7.0.cad` (CN1 PCIe fingers, filled bodies, larger hover rings from real pad radii). Remaining plan steps (part-kind colours, KiCad/TVW pads, label backings, selection glow) await user decision.

