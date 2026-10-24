# Proposal: Pin Context Menu on Right Click and Ctrl+Left Click

## Intent
When inspecting dense boards, technicians and engineers frequently need immediate, in-place access to pin details (pin number, connected net name, parent component, coordinate, and copy actions) without diverting attention away from the canvas to the side panel. Providing a contextual popup menu triggered by **Right Click** (secondary click) or **Ctrl + Left Click** on any pin will streamline debugging and net tracing.

## Scope

### In Scope
- Context menu trigger on pin hit:
  - Secondary mouse click (Right click) on a pin.
  - Primary mouse click (Left click) with `Ctrl` modifier held on a pin.
- In-place context popup (`egui::show_menu` / `egui::Area` / `popup_below_widget` or context menu) displaying:
  - Component name & pin number (e.g. `U1.A4`).
  - Net name (e.g. `GND`, `+12V_GPU`).
  - Pin side (`TOP` / `BOTTOM`) and coordinates.
  - Quick action: "Copy Net Name" to system clipboard.
  - Quick action: "Copy Pin Info" (e.g., `U1.A4 (GND)`) to system clipboard.
- Automatic pin selection when triggering the context menu.

### Out of Scope
- Editing pin or component metadata in-place (BoardView files are read-only inspection documents).
- Context menus for empty space or arbitrary board traces.

## Capabilities

### Modified Capabilities
- `boardview-canvas-interaction`: Add secondary click (right click) and Ctrl+left click handling on pins to open an in-place context menu with pin metadata and copy actions.

## Approach
1. In `crates/bvc-app/src/canvas.rs`, capture `secondary_clicked()` or `clicked()` with `ui.input(|i| i.modifiers.ctrl)` on the canvas response.
2. Query hit test for pin at cursor position using `b.spatial_grid` / `hit_test_pin`.
3. If a pin is hit:
   - Select the pin and its corresponding net.
   - Store open context menu state (e.g., `pin_id`, cursor screen position).
4. Render the contextual menu popup displaying the pin details and clipboard copy buttons with localized labels (`i18n.rs`).

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `crates/bvc-app/src/canvas.rs` | Modified | Add mouse click detection for secondary/Ctrl+click and render context menu popup |
| `crates/bvc-app/src/i18n.rs` | Modified | Add translation keys for context menu items and copy actions |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Context menu clipping near screen edges | Low | Use standard `egui` popup positioning or clamp within viewport rect |
| Conflict with right-click pan drag | Low | Check drag delta threshold: only open menu if secondary click was not a drag gesture |

## Rollback Plan
Revert changes in `canvas.rs` and `i18n.rs` via `git checkout`.

## Success Criteria
- [ ] Right-clicking on any pin opens the context menu showing component, pin number, and net name.
- [ ] Ctrl + Left-clicking on any pin opens the same context menu.
- [ ] Clicking "Copy Net Name" copies the net name to clipboard.
- [ ] Dragging with right click (panning) does not spuriously trigger the context menu.
- [ ] All unit tests pass cleanly.
