# Design: Toolbar Option to Toggle Pin Visibility

## Context & Architecture
Currently, pins are tessellated and rendered unconditionally for all visible entities in the frustum. Adding a pin visibility toggle allows clearing visual clutter from dense land patterns.

## Components & Changes

### 1. `Preferences` in `bvc-app::preferences`
Add `#[serde(default = "default_true")] pub show_pins: bool` to `Preferences`.
Expose a checkbox in the Preferences modal dialog under the Visualization section.

### 2. Top Toolbar in `bvc-app::main`
In [main.rs](file:///media/no_raid/devel/boarview/BoardViewCraft/crates/bvc-app/src/main.rs), in `Panel::top("top_bar")`:
Add a button:
```rust
let pins_label = format!("{}: {}", i18n::tr(lang, "toolbar.pins"), if self.preferences.show_pins { "ON" } else { "OFF" });
if ui.button(pins_label).on_hover_text("Toggle component pin/pad visibility").clicked() {
    self.preferences.show_pins = !self.preferences.show_pins;
    self.canvas_state.cached_view_key = None;
    ui.ctx().request_repaint();
}
```

### 3. Canvas Rendering in `bvc-app::canvas`
Update `generate_board_meshes`:
- Add `show_pins: bool` parameter.
- Filter pins in step 4:
  ```rust
  let is_active = is_net_selected || is_hovered || selected_pin_idx == Some(*_idx);
  if !show_pins && !is_active {
      return false;
  }
  ```
- Include `show_pins` in `ViewCacheKey`.

### 4. Internationalization in `bvc-app::i18n`
Add translation keys:
- `toolbar.pins`: "Pins" / "Pines" / "Pins"
- `prefs.show_pins`: "Show Component Pins" / "Mostrar Pines de Componentes" / "Mostrar Pins de Components"
