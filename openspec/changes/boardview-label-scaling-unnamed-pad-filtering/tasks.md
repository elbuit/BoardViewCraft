# Tasks: Dynamic Component Label Sizing & Unnamed Pad/Via Filtering

## Phase 1: Preferences & Localization
- [ ] 1.1 Add `show_vias` (default true) and `show_unnamed_vias` (default false) to `Preferences` in `crates/bvc-app/src/preferences.rs`.
- [ ] 1.2 Add i18n translation keys in `crates/bvc-app/src/i18n.rs` (English, Spanish, Catalan).
- [ ] 1.3 Add checkbox UI controls in the Preferences window.

## Phase 2: Canvas Rendering Implementation
- [ ] 2.1 Update component label sizing in `crates/bvc-app/src/canvas.rs` to scale font size proportionally to footprint dimensions.
- [ ] 2.2 Update via rendering filter in `generate_board_meshes` in `crates/bvc-app/src/canvas.rs` to respect `show_vias` and `show_unnamed_vias`.
- [ ] 2.3 Forward new preference flags through `BoardCanvas` to `generate_board_meshes`.

## Phase 3: Verification
- [ ] 3.1 Run `cargo test -p bvc-core` and verify all tests pass.
- [ ] 3.2 Run `cargo check -p bvc-app --ignore-rust-version`.
