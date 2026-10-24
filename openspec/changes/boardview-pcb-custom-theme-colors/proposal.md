# Proposal: PCB Custom Theme Profile with Seed-from-Active and Persistence

## Intent

Users want the flexibility to customize boardview PCB rendering colors beyond the standard presets (Light, Dark, Warm, Sweet Cold, Cadcraft) to match specific display environments, personal contrast requirements, or repair laboratory lighting. 
While the application chrome (menus, toolbars, side panels) remains fixed with clean Light/Dark modes, the PCB canvas needs a dedicated `Custom` theme option. When selecting `Custom`, it should initialize its color tokens from whichever theme was currently active, allow interactive color picking for key PCB elements, and persist these customizations reliably in the user preferences.

## Scope

### In Scope
- **PCB Theme Enum Extension:** Add `PcbTheme::Custom` to `PcbTheme::ALL` in `crates/bvc-app/src/theme.rs`.
- **Custom PCB Tokens Storage:** Add `custom_pcb_tokens: Option<PcbTokens>` to `Preferences` in `crates/bvc-app/src/preferences.rs`.
- **Seed from Active Theme:** When switching `pcb_theme` to `Custom`, if `custom_pcb_tokens` is not yet set or when switching into Custom, seed the custom values from the active/previous PCB theme.
- **Color Customization UI in Preferences:** When `pcb_theme == PcbTheme::Custom`, render color pickers for PCB elements:
  - Canvas background (`canvas`)
  - Board outline (`board_outline`) and fill (`board_fill`)
  - Component body (`part_body`) and designator text (`part_text`)
  - Pin default (`pin_default`) and unconnected (`pin_unconnected`)
  - Pin highlights (active side `pin_highlight_cur_side`, opposite side `pin_highlight_opp_side`)
  - Vias (active side `via_cur_side`, opposite side `via_opp_side`, highlight `via_highlight`)
  - Net flight lines (`net_web`)
- **Reset / Seed Button:** Provide a button to reset/re-seed custom colors from any of the standard presets (Cadcraft, Dark, Light, Warm, Sweet Cold).
- **Atomic Persistence:** Save `custom_pcb_tokens` into `preferences.json` alongside other settings.
- **Immediate Live Preview:** When colors are changed, immediately invalidate canvas mesh cache (`cached_view_key = None`) and update runtime `Tokens`.
- **Localization:** Add translation strings for `Custom`, color picker labels, and reset actions in English, Spanish, and Catalan.

### Out of Scope
- Customizing application chrome tokens (toolbar, menus, side panel UI styles remain strictly fixed to Light/Dark `AppTheme`).
- Importing/exporting external theme files (custom theme is stored directly in user preferences).

## Capabilities

### New Capabilities
None.

### Modified Capabilities
- `boardview-preferences-and-visualization`: Adds `Custom` PCB theme preset, interactive color editors, seed-from-active behavior, and persistence for `custom_pcb_tokens`.

## Approach

1. **Extend `PcbTheme` & `Tokens`:**
   - In `crates/bvc-app/src/theme.rs`:
     - Add `Custom` variant to `PcbTheme`.
     - Update `Tokens::new_with_custom(app_theme: AppTheme, pcb_theme: PcbTheme, custom: Option<PcbTokens>) -> Tokens`.
     - If `pcb_theme == PcbTheme::Custom`, use `custom.unwrap_or(PcbTokens::CADCRAFT)`.
2. **Update `Preferences` Struct:**
   - In `crates/bvc-app/src/preferences.rs`:
     - Add `pub custom_pcb_tokens: Option<PcbTokens>`.
     - In `show_preferences_window`, when the user selects `PcbTheme::Custom`, initialize `prefs.custom_pcb_tokens = Some(prefs.custom_pcb_tokens.unwrap_or_else(|| prev_pcb_theme.tokens()))`.
     - Render an `egui::Grid` with `ui.color_edit_button_srgba_unmultiplied` for each configurable color field.
3. **Application State & Cache Sync:**
   - In `crates/bvc-app/src/main.rs`:
     - Instantiate `self.tokens` with `custom_pcb_tokens` considered.
     - On preference change, detect changes in `pcb_theme` or `custom_pcb_tokens`, recompute tokens, invalidate canvas cache, and save atomically.
4. **Unit Tests:**
   - Add tests for `Preferences` serialization/deserialization with `custom_pcb_tokens`.
   - Verify `PcbTheme::Custom` fallback and token generation.

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `crates/bvc-app/src/theme.rs` | Modified | Add `PcbTheme::Custom`, update token constructor with custom pcb tokens |
| `crates/bvc-app/src/preferences.rs` | Modified | Add `custom_pcb_tokens` field, UI color pickers grid, seed logic |
| `crates/bvc-app/src/main.rs` | Modified | Pass custom tokens when constructing `Tokens` and handle live updates |
| `crates/bvc-app/src/i18n.rs` | Modified | Translations for "Custom...", color labels, and seed button in EN, ES, CA |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Deserializing legacy preferences without `custom_pcb_tokens` | Low | Field is `Option<PcbTokens>` with `#[serde(default)]` defaulting to `None` |
| Low contrast custom colors | Low | User controls their own custom palette; standard presets remain available |
| Frame drops while adjusting color sliders | Low | Egui color edits mutate in-place; mesh cache invalidation only recomputes when changed |

## Rollback Plan

Revert changes in `theme.rs`, `preferences.rs`, `main.rs`, and `i18n.rs`. Stored configuration gracefully ignores unknown `custom_pcb_tokens` or defaults to standard presets.

## Dependencies

None. Uses built-in `egui::Color32` and `serde`.

## Success Criteria

- [ ] `PcbTheme::Custom` appears in the PCB Theme dropdown in Preferences.
- [ ] Selecting `Custom` automatically starts from the currently active PCB theme colors.
- [ ] Users can edit individual PCB colors (canvas background, pins, vias, outline, part body/text, flight lines).
- [ ] Edits immediately reflect on the PCB canvas.
- [ ] Preferences persist across application restarts.
- [ ] All unit and regression tests pass with `cargo test --ignore-rust-version`.
