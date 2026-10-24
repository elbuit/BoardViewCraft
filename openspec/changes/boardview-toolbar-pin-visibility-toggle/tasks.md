# Tasks: Toolbar Option to Toggle Pin Visibility

## Phase 1: Preferences & Translations
- [ ] 1.1 Add `show_pins` to `Preferences` with `default_true` in `crates/bvc-app/src/preferences.rs`.
- [ ] 1.2 Add checkbox for `show_pins` in `Preferences::show()`.
- [ ] 1.3 Add i18n keys `toolbar.pins` and `prefs.show_pins` for English, Spanish, and Catalan in `crates/bvc-app/src/i18n.rs`.

## Phase 2: Toolbar & Canvas Integration
- [ ] 2.1 Add `Pins: ON/OFF` toggle button to top toolbar in `crates/bvc-app/src/main.rs`.
- [ ] 2.2 Add `show_pins` to `ViewCacheKey` and `generate_board_meshes` in `crates/bvc-app/src/canvas.rs`.
- [ ] 2.3 Filter unselected/unhighlighted pins in `generate_board_meshes` when `show_pins == false`.

## Phase 3: Verification
- [ ] 3.1 Run `cargo test -p bvc-app --ignore-rust-version`.
- [ ] 3.2 Run `cargo test -p bvc-core`.
