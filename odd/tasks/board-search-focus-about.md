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

- [x] T4 — Reorganize the top toolbar into menus: File (Open Board, Open Recent, Preferences), Actions (unchanged), View (Zoom submenu, Pins/Vias checkboxes), Help (About); side and rotate quick buttons kept (route: inline; user request 2026-10-31)

- [x] T5 — Remove the quick rotate button from the toolbar (keep the TOP/BOTTOM side button) (route: same delegated writer as T6)
- [x] T6 — Side panel without tabs: show only the active tool, chosen from context: photo overlay alignment -> overlay panel (with a way to finish/leave it); pin/via/net selected -> net/pin details; part only -> part details; nothing selected -> search; a "back to search" control clears the selection (route: delegated writer; side_panel.rs + lib.rs + i18n, design default chosen by orchestrator, user to confirm in GUI)

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
- T4: UI-only restructure inside the toolbar closure, no meaningful RED; strings `menu.file/view/help` added to 6 catalogs, key-count test 145 -> 148.
  - `cargo test -p bvc-ui-egui --release`: 66 passed, 0 failed, 1 ignored. clippy lib.rs 13 -> 13. `cargo build --release`: success. GUI check pending.
- T5: UI-only removal inside the toolbar closure, no meaningful RED. `toolbar.rotate` became unused and was removed from the 6 catalogs (key-count test 148 -> 147); `Rotation::display_str` is still used by the status bar.
  - `cargo test -p bvc-ui-egui --release`: 66 passed, 0 failed, 1 ignored. clippy per-file ui-egui counts equal to base (lib.rs 13). `cargo build --release`: Finished. GUI check pending.
- T6: RED observed (3 of 4 new `auto_view` tests failing against a stub returning `Search`), then GREEN. Tab bar removed; `SidePanelState.active_tab` replaced by `overlay_editing: bool`. The panel shows the overlay view while `overlay_editing` or an alignment session is active (with a "Done" button, disabled with a hint while calibrating); otherwise `auto_view(part, pin, via, net)` picks Net (net or via), Part (part, or an unconnected pin) or Search, recomputed every frame. Detail views have a "← Search" button that clears part/pin/via/net. New Actions entry "Adjust Overlay Photo" (enabled when either side has a photo; switches to that side if the current one has none). `Tab::Overlay` variant and the PDF-click `active_tab` assignments removed; canvas `overlay_mode_active` = `overlay_editing`. Keys: removed `side_panel.net_details/part_details/search/overlay`, added `side_panel.back_to_search`, `overlay.done`, `overlay.done_calibrating_hint`, `actions.adjust_overlay` in 6 catalogs (count stays 147).
  - Behaviour change: clicking a pin inside the part details now switches to net details (the rule "pin selected -> net details").
  - `cargo test -p bvc-ui-egui --release`: 70 passed, 0 failed, 1 ignored. clippy per-file ui-egui counts equal to base (side_panel.rs 7, lib.rs 13). `cargo build --release`: Finished. GUI check pending (user to confirm the design).
