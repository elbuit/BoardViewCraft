# Proposal: BoardViewCraft UX Enhancements: ESC Deselection, Hollow Opposite Pins & Side Panel Tab Copy

- **Change Name:** `boardview-ux-esc-hollow-pins-tab-copy`
- **Status:** Draft / Proposed
- **Author:** sdd-propose
- **Date:** 2026-10-23
- **Target Workspaces:** `BoardViewCraft` (`bvc-app`, `bvc-core`)
- **Reference Codebases:** `OpenBoardView`, `cadcraft`

---

## 1. Intent & Problem Statement

Technicians and hardware diagnostic engineers using BoardViewCraft frequently inspect dense motherboards and multi-layer graphics cards. Recent additions (such as strict layer filtering, selectable panels, and customizable palettes) established robust foundation controls, but several high-frequency interaction bottlenecks remain:

1. **Inconvenient Selection Dismissal (Missing ESC Key Shortcut):**
   - Once a pin, part, or net is selected on the canvas or side panel, there is no keyboard shortcut to quickly deselect or reset pin focus. Users must manually hunt down the exact same pin and click it again to dismiss selection.
   - Standard CAD and boardview workflows expect the `Escape` (ESC) key to instantly dismiss active selections, reset pin focus, and restore an unselected overview canvas state.

2. **Occlusion vs. Visibility of Opposite-Layer Net Connections:**
   - Under strict layer filtering (`show_opposite_elements: false`), opposite-side elements are completely culled to eliminate background clutter.
   - However, when a technician selects a pin to trace a net (e.g. `VCC_CORE`, `PPBUS_G3H`), knowing where that net surfaces on the opposite side of the board is critical for finding probe test points or back-side decoupling capacitors.
   - If opposite pins are rendered solid/filled, they obscure current-side pads and traces, creating confusion about which side the pad physically resides on.
   - The user needs opposite-side pins of the active net to be clearly visible **only as an outline/border (hollow/unfilled stroke)**. This provides immediate spatial awareness of reverse-side pads without obstructing active-layer geometry.

3. **Incomplete Side Panel Tab Clipboard Copying:**
   - The existing side panel copy button (`📋`) in the Net and Part details tabs only copies the isolated net name (e.g., `PP3V3_S0`) or part name (e.g., `U12`).
   - When preparing board repair notes, documentation, or debugging tickets, technicians need to copy the **entire active tab's contents**—including net/part metadata, connected pin lists with component designators, pad numbers, layer sides, and coordinates—in a clean, structured tabular format (TSV/Text) ready to paste into spreadsheets or documentation.

4. **Compiler Warning in `i18n.rs`:**
   - In `crates/bvc-app/src/i18n.rs`, the compiler produces a dead code warning: `method code is never used in Language`.
   - The codebase must compile cleanly with zero warnings.

---

## 2. Scope

### In Scope

- **Keyboard Interaction (`bvc-app`):**
  - Implement `Escape` (ESC) key handler in `BoardViewApp::ui`.
  - When pressed: clear `selected_pin_idx = None`, `selected_net = None`, and `selected_part_idx = None`.
  - Reset status message or indicate selection cleared if appropriate.
  - Automatically invalidate the render cache so the canvas repaints immediately without highlights.

- **Opposite-Layer Net Pin Outline Tessellation (`bvc-app`):**
  - Add stroked circle/ring tessellation to `Renderer` (`append_circle_outline_to_mesh` / `append_lod_circle_outline_to_mesh`).
  - In `canvas::generate_board_meshes`:
    - When a pin is selected (and its net is active), retain pins belonging to that net even if they belong to the opposite layer and `show_opposite_elements == false`.
    - For opposite-layer net pins, tessellate them strictly as **hollow/unfilled outlines** (stroke width 1.5–2.0 px) using `tokens.pin_highlight_opp_side`.
    - Ensure the inner region is completely hollow (zero fill), allowing active-layer surface pads and traces to remain fully visible underneath.

- **Full Side Panel Tab Content Copy (`bvc-app`):**
  - Update the copy button `📋` in `Tab::Net` to serialize and copy the entire Net Details tab content:
    - Net name, pin count, via count.
    - TSV formatted table of all connected pins: `Part\tPin\tSide\tX (mil)\tY (mil)`.
    - TSV formatted table of all connected vias: `Via ID\tSide\tX (mil)\tY (mil)`.
  - Update the copy button `📋` in `Tab::Part` to serialize and copy the entire Part Details tab content:
    - Part name, layer side, pin count, bounding box.
    - TSV formatted table of pins: `Pin\tNet\tSide\tX (mil)\tY (mil)`.
  - Provide inline user feedback (e.g. tooltip or brief status bar confirmation `"Copied entire Net details to clipboard"`).
  - Retain the right-click context menu options on individual items for single-value copying.

- **Warning Cleanup (`bvc-app`):**
  - Suppress dead code warning on `Language::code` via `#[allow(dead_code)]` or integrate it into display/serialization routines.

### Out of Scope

- Modifying file parsers or underlying CAD data structures in `bvc-core`.
- Interactive multi-point polygon editing or arbitrary schematic rendering.
- Modifying via rendering behavior (vias remain through-hole / multi-layer cylinders as currently modeled).

---

## 3. Technical Architecture & Implementation Design

### 3.1 ESC Key Deselection (`crates/bvc-app/src/main.rs`)

In `BoardViewApp::ui`, inside the global hotkey handling block:

```rust
let esc_pressed = ui.input(|i| i.key_pressed(egui::Key::Escape));
if esc_pressed && !self.preferences_open {
    if self.selected_pin_idx.is_some() || self.selected_net.is_some() || self.selected_part_idx.is_some() {
        self.selected_pin_idx = None;
        self.selected_part_idx = None;
        self.selected_net = None;
        self.status_msg = "Selection cleared.".to_string();
    }
}
```

Since `ViewCacheKey` includes `selected_pin`, `selected_net`, and `selected_part`, clearing these fields triggers a cache miss on the next frame, repainting the canvas cleanly in standard view mode.

---

### 3.2 Hollow Opposite-Layer Net Pins (`crates/bvc-app/src/render.rs` & `canvas.rs`)

#### 3.2.1 Hollow Circle Tessellation in `Renderer`
In `crates/bvc-app/src/render.rs`, add a method to tessellate a stroked circular ring directly into `egui::Mesh`:

```rust
/// Appends a stroked circle (annulus / ring) directly to `mesh` with hollow interior.
pub fn append_circle_outline_to_mesh(
    mesh: &mut Mesh,
    center: Pos2,
    radius: f32,
    stroke_width: f32,
    color: Color32,
) {
    if radius <= 0.0 || stroke_width <= 0.0 {
        return;
    }

    let r_outer = radius + stroke_width * 0.5;
    let r_inner = (radius - stroke_width * 0.5).max(0.0);
    let segments = (radius.round() as usize).clamp(8, 32);

    let start_idx = mesh.vertices.len() as u32;
    mesh.vertices.reserve(segments * 2);
    mesh.indices.reserve(segments * 6);

    let step = std::f32::consts::TAU / segments as f32;
    for i in 0..segments {
        let angle = i as f32 * step;
        let (sin_a, cos_a) = angle.sin_cos();

        // Inner vertex
        mesh.vertices.push(Vertex {
            pos: Pos2::new(center.x + r_inner * cos_a, center.y + r_inner * sin_a),
            uv: Pos2::ZERO,
            color,
        });
        // Outer vertex
        mesh.vertices.push(Vertex {
            pos: Pos2::new(center.x + r_outer * cos_a, center.y + r_outer * sin_a),
            uv: Pos2::ZERO,
            color,
        });
    }

    for i in 0..segments {
        let next_i = (i + 1) % segments;
        let inner_curr = start_idx + (i as u32 * 2);
        let outer_curr = inner_curr + 1;
        let inner_next = start_idx + (next_i as u32 * 2);
        let outer_next = inner_next + 1;

        // Quad composed of two triangles
        mesh.indices.push(inner_curr);
        mesh.indices.push(outer_curr);
        mesh.indices.push(outer_next);

        mesh.indices.push(inner_curr);
        mesh.indices.push(outer_next);
        mesh.indices.push(inner_next);
    }
}
```

#### 3.2.2 Filtering and Rendering in `crates/bvc-app/src/canvas.rs`
In `canvas::generate_board_meshes`:

1. **Filtering**:
   ```rust
   .filter(|(_idx, pin)| {
       let is_on_cur_side = pin.side.visible_on(vp.side);
       let is_net_selected = active_membership.contains_pin(*_idx);

       // Allow opposite-side pins if either:
       // 1. show_opposite_elements preference is enabled, OR
       // 2. a pin is actively selected and this pin belongs to its connected net.
       let allow_opposite = show_opposite_elements || (selected_pin_idx.is_some() && is_net_selected);
       if !allow_opposite && !is_on_cur_side {
           return false;
       }

       pin.pos.x >= visible_world.min_x
           && pin.pos.x <= visible_world.max_x
           && pin.pos.y >= visible_world.min_y
           && pin.pos.y <= visible_world.max_y
   })
   ```

2. **Mesh Generation**:
   ```rust
   if is_selected_pin {
       let screen_r = raw_screen_r.clamp(2.0, 40.0);
       Renderer::append_circle_to_mesh(&mut hl_mesh, screen_pos, screen_r, tokens.selection);
   } else if is_hovered {
       let screen_r = raw_screen_r.clamp(2.0, 40.0);
       Renderer::append_circle_to_mesh(&mut hl_mesh, screen_pos, screen_r, tokens.hover);
   } else if is_net_selected {
       if is_on_cur_side {
           // Current side: filled highlight circle
           Renderer::append_lod_circle_to_mesh(
               &mut hl_mesh,
               screen_pos,
               raw_screen_r,
               is_high_density,
               tokens.pin_highlight_cur_side,
           );
       } else {
           // Opposite side: hollow outline / border ONLY!
           let stroke_w = if is_high_density { 1.2 } else { 1.8 };
           Renderer::append_circle_outline_to_mesh(
               &mut hl_mesh,
               screen_pos,
               raw_screen_r.clamp(1.5, 40.0),
               stroke_w,
               tokens.pin_highlight_opp_side,
           );
       }
   } else {
       ...
   }
   ```

---

### 3.3 Side Panel Tab Full Content Clipboard Serialization (`crates/bvc-app/src/side_panel.rs`)

In `crates/bvc-app/src/side_panel.rs`:

#### 3.3.1 Net Details Tab Export
When clicking `📋` in `Tab::Net`:
```rust
fn export_net_details_tsv(b: &Board, net_name: &str) -> String {
    let mut out = String::new();
    out.push_str(&format!("Net:\t{}\n", net_name));
    if let Some(net) = b.get_net(net_name) {
        out.push_str(&format!("Pins:\t{}\tVias:\t{}\n\n", net.pin_indices.len(), net.via_indices.len()));
        out.push_str("Pins on Net:\n");
        out.push_str("Part\tPin\tSide\tX (mil)\tY (mil)\n");
        for &pin_idx in &net.pin_indices {
            if let Some(pin) = b.pins.get(pin_idx) {
                let part_name = b.parts.get(pin.part_id).map(|p| p.name.as_str()).unwrap_or("?");
                let side_str = match pin.side {
                    Side::Top => "Top",
                    Side::Bottom => "Bottom",
                    Side::Both => "Both",
                };
                out.push_str(&format!("{}\t{}\t{}\t{:.2}\t{:.2}\n", part_name, pin.name, side_str, pin.pos.x, pin.pos.y));
            }
        }

        if !net.via_indices.is_empty() {
            out.push_str("\nVias on Net:\n");
            out.push_str("Via ID\tSide\tX (mil)\tY (mil)\n");
            for &via_idx in &net.via_indices {
                if let Some(via) = b.vias.get(via_idx) {
                    let side_str = match via.side {
                        Side::Top => "Top",
                        Side::Bottom => "Bottom",
                        Side::Both => "Both",
                    };
                    out.push_str(&format!("{}\t{}\t{:.2}\t{:.2}\n", via.id, side_str, via.pos.x, via.pos.y));
                }
            }
        }
    }
    out
}
```

#### 3.3.2 Part Details Tab Export
When clicking `📋` in `Tab::Part`:
```rust
fn export_part_details_tsv(b: &Board, part: &bvc_core::Part) -> String {
    let mut out = String::new();
    let side_str = match part.side {
        Side::Top => "Top",
        Side::Bottom => "Bottom",
        Side::Both => "Both",
    };
    out.push_str(&format!("Part:\t{}\nSide:\t{}\nPins:\t{}\n\n", part.name, side_str, part.pin_indices.len()));
    out.push_str("Pins:\n");
    out.push_str("Pin\tNet\tSide\tX (mil)\tY (mil)\n");
    for &pin_idx in &part.pin_indices {
        if let Some(pin) = b.pins.get(pin_idx) {
            let net = if pin.net_name.is_empty() { "UNCONNECTED" } else { &pin.net_name };
            let pin_side = match pin.side {
                Side::Top => "Top",
                Side::Bottom => "Bottom",
                Side::Both => "Both",
            };
            out.push_str(&format!("{}\t{}\t{}\t{:.2}\t{:.2}\n", pin.name, net, pin_side, pin.pos.x, pin.pos.y));
        }
    }
    out
}
```

---

### 3.4 Compiler Warning Cleanup (`crates/bvc-app/src/i18n.rs`)

In `crates/bvc-app/src/i18n.rs`:
Add `#[allow(dead_code)]` to `Language::code`:

```rust
impl Language {
    pub const ALL: [Language; 3] = [Language::English, Language::Spanish, Language::Catalan];

    pub fn display_name(self) -> &'static str {
        match self {
            Language::English => "English",
            Language::Spanish => "Español",
            Language::Catalan => "Català",
        }
    }

    #[allow(dead_code)]
    pub fn code(self) -> &'static str {
        match self {
            Language::English => "en",
            Language::Spanish => "es",
            Language::Catalan => "ca",
        }
    }
}
```

---

## 4. File Changes Summary

| File | Change Nature | Description |
|------|---------------|-------------|
| `crates/bvc-app/src/render.rs` | Modification | Add `append_circle_outline_to_mesh` for direct GPU mesh annulus/stroke tessellation without interior fill. |
| `crates/bvc-app/src/canvas.rs` | Modification | Update pin filtering and rendering so opposite-layer net pins are shown as hollow outlines when a pin is selected. |
| `crates/bvc-app/src/main.rs` | Modification | Add `Key::Escape` handler to clear pin, part, and net selections and update status message. |
| `crates/bvc-app/src/side_panel.rs` | Modification | Update tab header copy buttons to format and copy entire tab data (TSV format) to clipboard. |
| `crates/bvc-app/src/i18n.rs` | Modification | Add `#[allow(dead_code)]` to `Language::code` to eliminate compiler warning. |

---

## 5. Verification & Acceptance Criteria

1. **ESC Shortcut Verification:**
   - Click any pin on the canvas. Pin is highlighted, net is highlighted, and side panel opens Net Details.
   - Press `Escape`. Active pin selection, part selection, and net selection are immediately cleared. Canvas returns to unselected board state.
   - Select a part from the Search tab; press `Escape`. Part selection is cleared.

2. **Hollow Opposite Pin Outline Verification:**
   - With `show_opposite_elements` unchecked (default), click a pin connected to a multi-layer net (e.g. GND or power rail).
   - Verify that opposite-layer pins connected to this net appear on the canvas.
   - Verify that opposite-layer pins appear strictly as hollow borders (stroked outlines) and do NOT obscure or paint over underlying pads or traces on the current side.
   - Flipping sides (`Space`) switches the active side: previously hollow pins become solid highlights, and previous solid pins become hollow outlines.
   - Pressing `Escape` removes both solid and hollow highlights.

3. **Side Panel Full Content Copy Verification:**
   - In Net Details, click `📋` button. Paste into text editor or spreadsheet. Confirm tab contains Net header, pin count, via count, and the complete TSV table of pins and vias.
   - In Part Details, click `📋` button. Paste into text editor or spreadsheet. Confirm tab contains Part name, side, pin count, and full TSV table of component pins.

4. **Compiler Warnings Verification:**
   - Compiling `crates/bvc-app` generates no warnings regarding `Language::code`.
