# Spec: Toolbar Via Visibility Toggle

## Capability: via-visibility-toggle

### Requirements

1. **Toolbar Button Display**:
   - The top Cadcraft toolbar shall render a toggle button displaying the localized label and current state: `{toolbar.vias}: ON` or `{toolbar.vias}: OFF`.
   - The button shall appear directly adjacent to the `Pins` visibility toggle button.
   - Hovering over the button shall display a tooltip describing its action: `"Toggle PCB via visibility"`.

2. **State Synchronization**:
   - Clicking the button shall toggle `preferences.show_vias` between `true` and `false`.
   - The button state shall reflect external changes to `preferences.show_vias` (such as changes made in the Preferences dialog).

3. **Immediate Canvas Invalidation**:
   - When toggled via the toolbar button, `canvas_state.cached_view_key` shall be set to `None`.
   - The egui UI context shall be notified to repaint immediately (`ui.ctx().request_repaint()`).

4. **Localization**:
   - Translation key `"toolbar.vias"` shall be provided for:
     - English: `"Vias"`
     - Spanish: `"Vías"`
     - Catalan: `"Vies"`

5. **Interaction Safeguard**:
   - When `show_vias` is disabled (`false`), any via that is actively hovered, actively selected, or belongs to the currently selected net must remain visible on the canvas to ensure user interaction and net tracing are not interrupted.
