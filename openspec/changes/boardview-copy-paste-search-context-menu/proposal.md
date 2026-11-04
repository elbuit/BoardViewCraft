# Proposal: Copy-Paste in Search and Context Menu Copy Actions

## Intent
Improve productivity and workflows for board repair technicians and engineers when searching and tracing components and nets. This proposal introduces:
1. Native clipboard pasting into the component and net search filter boxes (supporting Ctrl+V / Command+V and context menu paste if available).
2. Contextual right-click copy actions across search result lists (parts and nets).
3. Contextual right-click copy actions directly on component bodies/bounding boxes on the PCB canvas (complementing existing pin right-click context menu).

## Scope

### In Scope
- **Search Inputs Clipboard Paste**:
  - Support pasting clipboard text into the `filter_part` and `filter_net` search fields in the side panel.
  - Support standard clipboard paste shortcut (`Ctrl+V` / `Cmd+V`) when search fields have focus, and provide convenient context menu actions (or clear/paste button if helpful).
- **Search Results Context Menu**:
  - Right-click context menu on Part search results: "Copy Part Name", "Copy Part Details".
  - Right-click context menu on Net search results: "Copy Net Name", "Copy Net Details".
- **Canvas Component Context Menu**:
  - Right-click (or Ctrl+click) on a component body / bounding box on the PCB canvas:
    - Opens context menu showing Part Name, Side, Pin count.
    - Action: "Copy Part Name" (e.g. `U1`).
    - Action: "Copy Part Details (TSV)".
- **Localization (i18n)**:
  - Add i18n keys for context menus across English, Spanish, Catalan, German, French, Italian.

### Out of Scope
- Modifying board geometry or component definitions (BoardView files remain immutable).
- Global schematic cross-search refactoring.

## Capabilities

### Modified Capabilities
- `boardview-search`: Adds paste support into search fields and right-click context menus on search result rows.
- `boardview-canvas-interaction`: Extends context menu hit-testing to include component bodies when right-clicking on a part outside individual pins.

## Approach
1. **Search Inputs**:
   - `egui::TextEdit` singleline already supports internal keyboard paste via standard egui shortcut handling when focused. Ensure text fields properly receive keyboard paste events and add explicit context menus on text edits or row items if required.
2. **Search Lists Right-Click Context Menu**:
   - In `crates/ui-egui/src/side_panel.rs`: attach `.context_menu(|ui| ...)` to the selectable labels in the part and net search results.
3. **Canvas Component Context Menu**:
   - In `crates/ui-egui/src/canvas.rs`: when right-click occurs and no pin is hit, hit-test component bodies via `b.find_part_near(world_pos, vp.side)`.
   - Store `context_menu_part: Option<(usize, Pos2)>` in `CanvasState` (or unify with a contextual menu enum).
   - Render component context menu popup with copy actions.
4. **Localization**:
   - Update `crates/ui-egui/src/i18n/*.tsv` files with new keys.

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `crates/ui-egui/src/side_panel.rs` | Modified | Add context menus on search results and check paste behaviour in search fields |
| `crates/ui-egui/src/canvas.rs` | Modified | Add component right-click context menu |
| `crates/ui-egui/src/i18n/*.tsv` | Modified | New localization strings for component & search context menus |
