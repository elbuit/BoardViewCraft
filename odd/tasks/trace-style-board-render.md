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
