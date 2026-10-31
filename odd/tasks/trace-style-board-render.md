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
- [ ] T2 — GenCAD pads and part outlines (route: same writer)
- [ ] T3 — Renderer: filled bodies, rotated pads, palette (route: same writer)

## Acceptance criteria
- On `tmp/MS-V375 REV 7.0.cad`: pins carry real pad width/height/rotation; parts carry outlines; PCIe edge connector fingers render as long thin rectangles at true size inside a filled body.
- Other formats render as before (round pins), tests pass.
- `cargo test --workspace --release` green (except known flaky bench), no new clippy warnings, `cargo build --release` ok.

## Progress
- Branch `feat/trace-style-board-render` created from `0d78513` (stacked on feat/board-search-focus-about).

### T1 evidence
- `PadShape`, `Pin::{shape,width,height,rotation_deg}`, `Part::{outline,rotation_deg}`; `Pin`/`Part`/`Point`/`Side`/`PartType` derive `Default`; all struct literals got `..Default::default()` (incl. test-only sites in `bvc-core/src/lib.rs` and `spatial.rs`, user-approved).
- `board::compute_part_bounds`: outline bounds, else union of rotated pad extents (port of trace `padExtent`), sizeless pads keep the 6-mil margin (other formats unchanged).
- Test-first exception: new function, so RED not observed separately; `rebuild_indices_uses_pad_aware_bounds` would have produced +/-6 on the base.
- `cargo test --release -p bvc-core --lib board::tests`: 7 passed. `cargo test --workspace --release`: all ok (71 passed, 1 ignored in ui-egui). Clippy per-file warning counts identical to base `0d78513`.
