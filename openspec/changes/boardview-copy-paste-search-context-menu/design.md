# Design: Copy & Paste for Search and Context Menu Copy Actions

## Context & Architecture
Users need quick copy-paste workflows to transition between diagnostic tools, schematics, and BoardViewCraft.
Currently:
- Single-line search `TextEdit` widgets allow keyboard paste via egui's native event processing, but having an explicit context menu and dedicated paste action (as well as ensuring clean keyboard shortcuts) guarantees usability across platforms and mouse-only workflows.
- Search result lists for Parts and Nets display selectable rows without context menus.
- Canvas supports right-click on individual pins (`context_menu_pin`), but right-clicking on a component's body/bounding box currently only handles left-click selection without displaying a context menu for the component.

## Detailed Design & Changes

### 1. Context Menu on Component Canvas (`crates/ui-egui/src/canvas.rs`)
In `CanvasState`:
Replace or complement `pub context_menu_pin: Option<(usize, Pos2)>` with component context menu support:
```rust
pub enum ContextMenuTarget {
    Pin(usize),
    Part(usize),
}
pub context_menu: Option<(ContextMenuTarget, Pos2)>,
```
(or add `pub context_menu_part: Option<(usize, Pos2)>` to preserve backwards compatibility).

In canvas event handling (`is_context_trigger`):
1. First hit-test pins (`current_hover_pin`). If hit, set `context_menu_pin = Some((pin_idx, click_pos))`.
2. If no pin is hit, hit-test component bodies:
   ```rust
   let world_pos = vp.screen_to_world(hover_pos, rect);
   if let Some(part_idx) = b.find_part_near(world_pos, vp.side) {
       *selected_part_idx = Some(part_idx);
       *selected_pin_idx = None;
       *selected_via_idx = None;
       *selected_net = None;
       canvas_state.context_menu_part = Some((part_idx, click_pos));
   }
   ```
3. In context menu rendering:
   If `context_menu_part` is active, render popup window/area:
   - Header: Part name (`U1`, `R12`, etc.)
   - Subtitle/Info: Side (`Top` / `Bottom`), Pin count.
   - Action: "Copy Part Name" -> `ui.ctx().copy_text(part.name.clone())`.
   - Action: "Copy Part Details (TSV)" -> `ui.ctx().copy_text(export_part_details_tsv(b, part))`.

### 2. Search Lists Context Menus (`crates/ui-egui/src/side_panel.rs`)
In `Tab::Search`:
- **Part matches list**:
  Attach `.context_menu(|ui| { ... })` to `ui.selectable_label(...)`:
  - `Copy Part Name`: copies `part.name`.
  - `Copy Part Details (TSV)`: copies `export_part_details_tsv(b, part)`.
- **Net matches list**:
  Attach `.context_menu(|ui| { ... })` to `ui.selectable_label(...)`:
  - `Copy Net Name`: copies `name.clone()`.
  - `Copy Net Details (TSV)`: copies `export_net_details_tsv(b, name)`.

### 3. Search Inputs Paste Support (`crates/ui-egui/src/side_panel.rs`)
- Add right-click context menu to search input TextEdits (or an action button / context menu) allowing "Paste" from clipboard:
  ```rust
  let part_edit = ui.add(TextEdit::singleline(&mut state.filter_part).hint_text(...));
  part_edit.context_menu(|ui| {
      if ui.button(tr(lang, "context_menu.paste")).clicked() {
          if let Some(text) = ui.ctx().output(|o| o.copied_text.clone()) /* or ui.input(|i| ...) / clipboard fetch */ {
              state.filter_part = text.trim().to_string();
          }
          ui.close();
      }
      if !state.filter_part.is_empty() && ui.button(tr(lang, "context_menu.clear")).clicked() {
          state.filter_part.clear();
          ui.close();
      }
  });
  ```
- Also support clipboard reading in egui via `ui.ctx().input(|i| ...)` or checking clipboard helper if present.

### 4. Internationalization (`crates/ui-egui/src/i18n/*.tsv`)
Add keys across all 6 languages:
- `context_menu.copy_part_name`: "Copy Part Name" / "Copiar nombre de componente" / etc.
- `context_menu.copy_part_details`: "Copy Part Details (TSV)" / "Copiar detalles del componente (TSV)" / etc.
- `context_menu.paste`: "Paste" / "Pegar" / "Enganxar" / "Einfügen" / "Coller" / "Incolla"
- `context_menu.clear`: "Clear" / "Limpiar" / "Netejar" / "Löschen" / "Effacer" / "Cancella"
- `context_menu.pins_count`: "Pins" / "Pines" / "Pins" / "Pins" / "Broches" / "Pin"
