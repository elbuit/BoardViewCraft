# Proposal: Toolbar Option to Toggle Pin Visibility

## Intent

Currently, BoardViewCraft unconditionally renders all component pads/pins on the active board side. In certain boardview workflows (e.g. tracing copper planes, inspecting board outlines, reviewing silkscreen/part arrangements, or analyzing dense nets without pad clutter), technicians and engineers need to temporarily hide component pins/pads while preserving or focusing on component boundaries, outlines, or net connections.

The user has requested adding an option in the top toolbar to easily toggle pin visibility on or off.

## Scope

### In Scope
- **Preferences & State:** Add `show_pins` preference field (default: `true`), persisted across sessions with serde defaults.
- **Top Toolbar Toggle:** Add a clean, accessible button / toggle in the top toolbar (e.g. `Pins: ON / OFF` or icon/button with keyboard shortcut or tooltip) alongside the existing view/side and rotate controls.
- **Preferences Dialog Integration:** Expose the `show_pins` checkbox in the preferences modal under Visualization.
- **Canvas Rendering Control:** Respect `show_pins` in `generate_board_meshes` (skipping normal pin meshes when `show_pins == false`, while keeping selected/hovered pins visible to ensure interactive net inspection remains functional).
- **Internationalization:** Provide translation keys for English, Spanish, and Catalan.

### Out of Scope
- Hiding component bounding boxes or refdes text when pins are toggled off (these remain controlled by their respective settings).
- Modifying underlying board models or parser logic.

## Capabilities

### New Capabilities
- `pin-visibility-toggle`: Configurable rendering toggle and toolbar control for component pin/pad visibility.

### Modified Capabilities
- None

## Approach

1. **Preference & State ([crates/bvc-app/src/preferences.rs](file:///media/no_raid/devel/boarview/BoardViewCraft/crates/bvc-app/src/preferences.rs)):**
   - Add `#[serde(default = "default_true")] pub show_pins: bool` to `Preferences`.
   - Add checkbox toggle in `Preferences::show()` under Visualization options.
2. **Top Toolbar Control ([crates/bvc-app/src/main.rs](file:///media/no_raid/devel/boarview/BoardViewCraft/crates/bvc-app/src/main.rs)):**
   - Add a toolbar toggle button displaying localized label: `Pins [ON/OFF]` in the main horizontal bar next to side flip and rotation.
   - When clicked, toggle `self.preferences.show_pins`, invalidate the canvas cache key, and request repaint.
3. **Canvas Tessellation ([crates/bvc-app/src/canvas.rs](file:///media/no_raid/devel/boarview/BoardViewCraft/crates/bvc-app/src/canvas.rs)):**
   - Pass `show_pins` into `generate_board_meshes` and include in `ViewCacheKey`.
   - In the pin filter step:
     ```rust
     let is_active = is_net_selected || is_hovered || selected_pin_idx == Some(*_idx);
     if !show_pins && !is_active {
         return false;
     }
     ```
4. **Localization ([crates/bvc-app/src/i18n.rs](file:///media/no_raid/devel/boarview/BoardViewCraft/crates/bvc-app/src/i18n.rs)):**
   - `toolbar.toggle_pins`: "Pins" / "Pines" / "Pins"
   - `prefs.show_pins`: "Show Component Pins & Pads" / "Mostrar Pines y Pads de Componentes" / "Mostrar Pins i Pads de Components"

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `crates/bvc-app/src/main.rs` | Modified | Add toolbar toggle button for pin visibility |
| `crates/bvc-app/src/preferences.rs` | Modified | Add `show_pins` preference field, serde defaults, and UI checkbox |
| `crates/bvc-app/src/canvas.rs` | Modified | Filter pin meshes when `show_pins` is false (preserving active selection/hover) |
| `crates/bvc-app/src/i18n.rs` | Modified | Localized string keys for toolbar and preferences |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| User loses selection when pins are hidden | Low | Any pin belonging to an active net or currently selected/hovered remains visible with full highlight |
| Top toolbar crowding on small window widths | Low | Use compact `Pins [ON/OFF]` label alongside existing compact controls |

## Rollback Plan

Revert changes in `main.rs`, `preferences.rs`, `canvas.rs`, and `i18n.rs`. `#[serde(default = "default_true")]` ensures existing configurations continue loading safely.

## Dependencies

- None

## Success Criteria

- [ ] User can click a button in the top toolbar to toggle component pins on and off.
- [ ] When pins are turned off, unselected pins disappear from the canvas.
- [ ] Selected pins, hovered pins, and active net pins remain visible.
- [ ] Setting is accessible and toggleable in the Preferences dialog.
- [ ] All unit and integration tests compile and pass.
