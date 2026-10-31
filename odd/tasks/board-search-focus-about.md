# Feature: board-search-focus-about

## Objective
1. Clicking a part in the side panel's component search selects ONLY that part (no pin, via or net) and centers the board view on it.
2. Add an "About" menu/window showing the app version and other build/runtime information.

## Problem / Why
User request 2026-10-31. Today the search click (`crates/ui-egui/src/side_panel.rs` ~312-330) also selects the part's first pin and its net, and the board view does not move to the part.

## Scope
- Side panel search click: `selected_part_idx = Some(id)`, pin/via/net cleared; board viewport centered on the part's bounding box; if the part is not visible on the current view side, switch the view side. Keep zoom.
- About window opened from the toolbar: name, version (`CARGO_PKG_VERSION`), license, OS/arch, build profile, egui version if available, short description; translated strings in the 6 catalogs.
- Branch `feat/board-search-focus-about` stacked on `feat/pdf-tiled-raster` (b0b2c3f).

## Tasks
- [x] T1 — Search click selects only the part and centers the board on it (route: delegated writer; 2+ non-trivial files)
- [x] T2 — About window with version and build info (route: same delegated writer)

- [x] T3 — Clicking a part in the schematic PDF selects only the part and centers the board via `part_focus_target`; sets `probed_part` so the PDF does not jump back to the part's first hit (route: inline; one-file edit, user request 2026-10-31)

## Acceptance criteria
- After clicking a search result: only the part is selected (pin/via/net None), viewport center == part bbox center, view side shows the part.
- About menu entry opens a window with at least version, license, OS/arch, build profile.
- `cargo test -p bvc-ui-egui` passes (i18n key-count test updated), no new clippy warnings, `cargo build --release` succeeds.

## Delivery strategy
ask-on-risk. Forecast ~200 authored lines.

## Progress
- Branch created from `b0b2c3f`.
- T1: RED observed (3 new `side_panel` tests failing against stubs), then GREEN. `select_part_only` clears pin/via/net; `part_focus_target` returns bbox center (first-pin fallback) and the part's side when not visible on the current view side; `SidePanelState::focus_part` is consumed in `lib.rs` after `show_side_panel`, keeping zoom and resetting `cached_view_key`. `cargo test -p bvc-ui-egui --release`: 64 passed, 1 ignored. Clippy: per-file counts equal to base. Commit: b0fa30e.
- T2: RED observed (2 `about` tests failing on empty rows), then GREEN. New `crates/ui-egui/src/about.rs` (`about_rows` + `show_about_window`: name, description, version, license, OS/arch, build profile, UI toolkit, PDF engine; Close button and Escape). Toolbar "About..." button after Preferences. 10 new keys in all 6 catalogs (key-count test 135 -> 145). egui version omitted (no const exported without new deps). `cargo test -p bvc-ui-egui --release`: 66 passed, 1 ignored. Clippy: per-file counts equal to base. `cargo build --release`: Finished. Commit: `feat(ui): add About window with version and build info`.
- Not verified: interactive GUI run (search click centering, About window rendering).
- T3: no RED possible (handler lives inside the egui panel closure in lib.rs; centering logic already covered by `part_focus_target` tests).
  - `cargo test -p bvc-ui-egui --release`: 66 passed, 0 failed, 1 ignored. clippy lib.rs warnings 13 -> 13. `cargo build --release`: success.
  - GUI behaviour not verified: manual check pending.
