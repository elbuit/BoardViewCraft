# Design: Dynamic Component Label Sizing & Unnamed Pad/Via Filtering

## Context & Motivation
Large components on dense boards (such as BGA chips, processors, memory modules) suffer from poor legibility when their designators are locked to a fixed 12.0 pt font size. At the same time, thousands of unlabelled vias and test probe pads generate visual noise. Reference implementations like OpenBoardView and Wrench-board offer via filters and footprint-adaptive text rendering.

## Architecture & Data Flow

### 1. Label Sizing in `bvc-app::canvas`
In [canvas.rs](file:///media/no_raid/devel/boarview/BoardViewCraft/crates/bvc-app/src/canvas.rs), during the component label rendering pass:
- Compute projected bounding box minimum dimension: `min_dim = c_rect.width().min(c_rect.height()) as f32 * vp.scale;`
- Proportional font size computation:
  ```rust
  let font_size = (11.0 + (min_dim - 24.0) * 0.12).clamp(10.0, 28.0);
  ```
- Pass `FontId::proportional(font_size)` to `painter.text()`.

### 2. Preference Schema Extensions in `bvc-app::preferences`
In [preferences.rs](file:///media/no_raid/devel/boarview/BoardViewCraft/crates/bvc-app/src/preferences.rs):
```rust
#[serde(default = "default_true")]
pub show_vias: bool,

#[serde(default = "default_false")]
pub show_unnamed_vias: bool,
```
With backward-compatible serde defaults and serialization.

### 3. Canvas Filtering in `bvc-app::canvas::generate_board_meshes`
In the via rendering pass:
- If `!prefs.show_vias && !is_net_selected && !is_hovered`, skip tessellation.
- If `!prefs.show_unnamed_vias && (via.net_name.is_empty() || via.net_name == "UNCONNECTED") && !is_net_selected && !is_hovered`, skip tessellation.

### 4. Localization in `bvc-app::i18n`
Add localized strings for:
- `prefs.show_vias`: "Show Vias" / "Mostrar Vías" / "Mostrar Vies"
- `prefs.show_unnamed_vias`: "Show Unnamed Vias & Pads" / "Mostrar Vías y Pads sin nombre" / "Mostrar Vies i Pads sense nom"
