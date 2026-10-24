# Proposal: Toolbar Via Visibility Toggle

## Intent

Users need to quickly toggle PCB vias visibility on and off directly from the Cadcraft top toolbar, mirroring the pin visibility toggle functionality. In dense boards, vias can clutter the visual inspection of tracks and components. While `show_vias` exists in `Preferences`, having a dedicated quick toggle button in the primary toolbar allows immediate toggling without opening the preferences modal.

## Scope

### In Scope
- Add a dedicated quick toggle button `Vias: ON/OFF` in the Cadcraft top toolbar next to the `Pins: ON/OFF` button.
- Synchronize button state with `self.preferences.show_vias`.
- Invalidate the canvas view cache (`cached_view_key = None`) and request immediate UI repaint on toggle.
- Add i18n support for the toolbar label across English (`Vias`), Spanish (`Vías`), and Catalan (`Vies`).
- Verify backward compatibility and unit tests.

### Out of Scope
- Changing via tessellation algorithms or via LOD thresholds.
- Renaming or altering the existing `prefs.show_vias` configuration key.
- Hotkey shortcuts for via toggling (deferred).

## Capabilities

### New Capabilities
None

### Modified Capabilities
- `via-visibility-toggle`: Expose quick toggle control for via visibility in the main application toolbar.

## Approach

1. **Toolbar UI Integration**:
   - In `crates/bvc-app/src/main.rs`, add a button next to `Pins: ON/OFF` in the top bar using localized text:
     ```rust
     let vias_status = if self.preferences.show_vias { "ON" } else { "OFF" };
     let vias_label = format!("{}: {}", i18n::tr(lang, "toolbar.vias"), vias_status);
     if ui.button(vias_label).on_hover_text("Toggle PCB via visibility").clicked() {
         self.preferences.show_vias = !self.preferences.show_vias;
         self.canvas_state.cached_view_key = None;
         ui.ctx().request_repaint();
     }
     ```
2. **Localization**:
   - Add `"toolbar.vias"` in `crates/bvc-app/src/i18n.rs` for English (`"Vias"`), Spanish (`"Vías"`), and Catalan (`"Vies"`).
3. **Canvas Cache & Tessellation Verification**:
   - `ViewCacheKey` and `generate_board_meshes` already respect `prefs.show_vias` (retaining selected/hovered vias and active net vias). Invalidation ensures an instant response without viewport adjustments.

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `crates/bvc-app/src/main.rs` | Modified | Add `Vias: ON/OFF` toggle button to top toolbar |
| `crates/bvc-app/src/i18n.rs` | Modified | Add `"toolbar.vias"` translations (EN, ES, CA) |
| `crates/bvc-app/src/canvas.rs` | Verified | Already supports `show_vias` in rendering pipeline & cache |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Toolbar clutter on smaller displays | Low | Button label is compact (`Vias: ON` / `Vias: OFF`) |
| Breaking selection of vias when hidden | Low | Interactive hover/selection and active net vias remain visible even when `show_vias == false` |

## Rollback Plan

Revert toolbar button addition in `main.rs` and the new translation key in `i18n.rs`.

## Dependencies

- Existing `Preferences::show_vias` field and `ViewCacheKey::show_vias`.

## Success Criteria

- [ ] A button `Vias: ON` / `Vias: OFF` is visible in the top toolbar next to `Pins`.
- [ ] Clicking the button toggles `preferences.show_vias` and repaints the canvas immediately.
- [ ] Tooltip and localized labels display properly in English, Spanish, and Catalan.
- [ ] Unit tests in `bvc-app` pass.
