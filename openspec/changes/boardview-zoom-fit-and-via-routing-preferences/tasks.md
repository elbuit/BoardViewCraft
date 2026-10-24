# Tasks: Zoom Fit Defaults and Via Routing Display Preferences

## Phase 1: Preferences Model & i18n
- [x] 1.1 Define `ViaDisplayMode` enum (`FastestPath`, `Radial`, `Hidden`) with serde and default in `crates/bvc-app/src/preferences.rs`.
- [x] 1.2 Add `via_display_mode: ViaDisplayMode` to `Preferences` struct with backwards compatibility.
- [x] 1.3 Add i18n localization keys in `crates/bvc-app/src/i18n.rs` (EN, ES, CA).
- [x] 1.4 Expose selector for Via Display Mode in Preferences dialog under Visualization section.

## Phase 2: Zoom Menu & Auto-Fit Implementation
- [x] 2.1 Add "Fit to Window (F)" as item 0 in Zoom dropdown menu in `crates/bvc-app/src/main.rs`.
- [x] 2.2 Add `pending_auto_fit: bool` state in `BoardViewApp`.
- [x] 2.3 Set `pending_auto_fit = true` on successful `load_file`.
- [x] 2.4 Trigger `fit_to_rect` in `show_canvas` once valid non-empty canvas rect is measured, and clear `pending_auto_fit`.
- [x] 2.5 Ensure global shortcut `F` continues to trigger `fit_to_rect`.

## Phase 3: Canvas Net Web Tessellation
- [x] 3.1 Include `via_display_mode` in `ViewCacheKey` in `crates/bvc-app/src/canvas.rs`.
- [x] 3.2 Update `generate_board_meshes` to support `FastestPath`, `Radial`, and `Hidden` modes when collecting nodes and computing segments.

## Phase 4: Verification & Tests
- [x] 4.1 Update and add unit tests for `Preferences` serde and `ViaDisplayMode`.
- [x] 4.2 Verify compilation and unit tests with `cargo test --ignore-rust-version`.
