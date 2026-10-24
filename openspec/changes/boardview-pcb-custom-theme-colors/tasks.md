# Tasks: PCB Custom Theme Profile and Color Persistence

## Phase 1: Theme Model & Tokens Update
- [x] 1.1 Add `PcbTheme::Custom` variant to `crates/bvc-app/src/theme.rs`.
- [x] 1.2 Update `PcbTheme::ALL` and `display_name` to include "Custom...".
- [x] 1.3 Add `Tokens::new_with_custom(app_theme, pcb_theme, custom_pcb_tokens)` supporting custom tokens.

## Phase 2: Preferences Model & Color Pickers UI
- [x] 2.1 Add `pub custom_pcb_tokens: Option<PcbTokens>` to `Preferences` struct with `#[serde(default)]` in `crates/bvc-app/src/preferences.rs`.
- [x] 2.2 In `show_preferences_window`, seed `custom_pcb_tokens` from current active PCB theme when switching to `PcbTheme::Custom`.
- [x] 2.3 Expose color pickers grid for canvas, outline, fill, parts, pins, vias, and net web lines.
- [x] 2.4 Add preset reset/seed options in preferences dialog.

## Phase 3: Application Integration & Cache Sync
- [x] 3.1 Pass `preferences.custom_pcb_tokens` to `Tokens` constructor in `crates/bvc-app/src/main.rs`.
- [x] 3.2 Update live preference change handler to detect modifications to `custom_pcb_tokens` and invalidate canvas mesh cache.

## Phase 4: Localization & Tests
- [x] 4.1 Add translation strings in `crates/bvc-app/src/i18n.rs` for EN, ES, CA.
- [x] 4.2 Update unit tests for `Preferences` serde roundtrip with `custom_pcb_tokens`.
- [x] 4.3 Run `cargo test --ignore-rust-version` and verify all tests pass.
