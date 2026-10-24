# Proposal: BoardViewCraft Performance, Preferences & Actions

- **Change Name:** `boardview-perf-preferences-actions`
- **Status:** Draft / Proposed
- **Author:** sdd-propose
- **Date:** 2026-10-21
- **Target Workspaces:** `BoardViewCraft` (`bvc-core`, `bvc-app`)
- **Reference Codebases:** `OpenBoardView`, `cadcraft`

---

## 1. Intent & Problem Statement

BoardViewCraft provides a modern pure-Rust boardview viewer for hardware reverse engineering and board repair. While normal navigation is snappy, testing with large real-world schematics (such as `MS-V375 REV 7.0.cad` containing 2,481 pins and 2,274 vias on `GND`) revealed critical performance bottlenecks and missing standard CAD navigation features:

1. **High-Density Net Selection Frame Drops:**
   - When dense nets (like `GND` or power rails) are selected, every pan or zoom frame that invalidates the view cache requires evaluating 4,755+ elements across Rayon worker threads.
   - For every element, dynamic equality checks (`selected_net == via.net_name`) and circle tessellations are performed.
   - At wide zoom levels, thousands of tiny ground vias (each smaller than 1–2 screen pixels) are tessellated into geometry, stressing egui's vertex submission pipeline and causing perceptible stuttering.
2. **Missing Preferences & Customization:**
   - Technicians and board repair engineers rely on customized palettes (e.g. OpenBoardView classic dark colors, high-contrast probe modes, or light themes for printing/documentation).
   - Net webs (ratlines connecting nodes) can be visual clutter on complex boards; users need the ability to toggle net web lines on or off.
3. **Missing Orientation & Inspection Actions:**
   - Boards placed on repair benches are frequently rotated at 90°, 180°, or 270° angles relative to original CAD orientation.
   - Currently, only simple Top/Bottom toggle exists without 90° CW/CCW / 180° rotation or horizontal/vertical mirror controls.

---

## 2. Scope

### In Scope
- **Net Spatial Index & Fast-Path Highlighting:**
  - Introduce spatial grid/bucket indexing or pre-indexed net node lists in `bvc-core::Board` for O(visible) net filtering instead of scanning the full pin/via vector.
  - Implement fast-path net membership: pre-filtered index vectors (`net.pin_indices`, `net.via_indices`) avoiding string comparisons in inner loops.
  - Add LOD clustering/culling: when zoomed out such that pin/via screen radius is `< 1.0px`, cull dense ground vias or batch into ultra-compact 1-quad point primitives.
  - Cache net highlight meshes per viewport transformation.
- **Preferences System:**
  - Create persistent/runtime preferences state in `bvc-app`:
    - Theme / Palette selector: `Default Dark` (cadcraft), `OpenBoardView Dark`, `High Contrast`, `Light Mode`.
    - Net web / ratlines toggle: Show or hide connecting lines on selected net.
    - Preferences modal dialog (`egui::Window`) accessible via top menu or shortcut `Ctrl+,`.
- **Actions Menu & Transformation:**
  - Introduce top bar "Actions" menu (and corresponding keyboard shortcuts).
  - Rotation: Rotate 90° Clockwise, Rotate 90° Counter-Clockwise, Rotate 180°.
  - Mirroring & Layer Flipping: Flip to Top, Flip to Bottom, Mirror Horizontally (`Mirror X`), Mirror Vertically (`Mirror Y`).
  - Viewport math updates in `canvas.rs` supporting rotation angle ($\theta \in \{0^\circ, 90^\circ, 180^\circ, 270^\circ\}$) and horizontal/vertical flips.

### Out of Scope
- Modifying underlying CAD/GenCAD binary parsers or format decoders.
- Automatic schematic cross-probing (PDF schematic sync).
- Implementing full custom user palette color pickers with RGB hex editing in this iteration (fixed preset themes will be delivered first; custom color picker deferred to future cycle).

---

## 3. Capabilities & User Experience

### 3.1 High-Density Net Highlighting
- When selecting `GND` or `+12V_EXT` on boards with 5,000+ nodes, panning and continuous pinch/wheel zoom maintains a fluid 60+ FPS.
- Culling eliminates off-screen net elements before mesh generation.
- At low zoom levels, dense via fields render as high-efficiency point clouds without micro-stutters.

### 3.2 Preferences Dialog
- **Theme Selection:**
  - **Cadcraft Dark:** Polished dark slate/navy theme with bright emerald net accents.
  - **OpenBoardView Classic:** Signature dark charcoal canvas, yellow selected net, blue opposite pins.
  - **High Contrast:** Pure black canvas with neon green and bright magenta elements for noisy repair-bench monitors.
  - **Light Mode:** Clean light gray/white background with dark outlines and saturated pins for printing or documentation.
- **Net Web Visibility:**
  - Checkbox toggle: `[x] Show net web lines on selection`. When disabled, selecting a pin or net highlights nodes without rendering crisscrossing line segments.

### 3.3 Actions Menu & Viewport Transformations
- Top bar menu items:
  - `Actions -> Rotate 90° CW` (`R`)
  - `Actions -> Rotate 90° CCW` (`Shift+R`)
  - `Actions -> Rotate 180°`
  - `Actions -> Flip Side (Top / Bottom)` (`Space`)
  - `Actions -> Mirror Horizontally` (`M`)
  - `Actions -> Mirror Vertically` (`Shift+M`)
- All mouse hit-testing (`find_pin_near`, `find_via_near`) and screen-to-world unproject transforms seamlessly honor the active rotation and mirror matrix.

---

## 4. Technical Approach & Architecture

### 4.1 Viewport Transformation Matrix (`bvc-app/src/canvas.rs`)
Currently, `Viewport` applies a hardcoded linear transformation:
```rust
pub struct Viewport {
    pub center: Point,
    pub scale: f32,
    pub side: Side,
}
```
We upgrade `Viewport` to incorporate orientation:
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rotation {
    Deg0,
    Deg90,
    Deg180,
    Deg270,
}

pub struct Viewport {
    pub center: Point,
    pub scale: f32,
    pub side: Side,
    pub rotation: Rotation,
    pub mirror_x: bool,
    pub mirror_y: bool,
}
```
`world_to_screen` and `screen_to_world` will use a 2D affine matrix transformation:
1. Translate by `-center`
2. Apply `mirror_x` (incorporating `side == Side::Bottom` default flip) and `mirror_y`
3. Rotate by $\theta \in \{0, 90, 180, 270\}$ degrees
4. Scale by `scale`
5. Translate to canvas center `(canvas_rect.center())`

Hit testing, component bounding boxes, and label rotations will consistently derive from these matrix operations.

### 4.2 Spatial Indexing & Net Highlighting Architecture

#### The Problem in Existing Code:
In `show_canvas`, every frame iterates through `b.vias.par_iter()` and `b.pins.par_iter()`. If `GND` is selected:
1. `selected_net_ref.map(|sn| sn == via.net_name)` performs thousands of string comparisons.
2. All 2,274 vias and 2,481 pins on GND generate meshes, even those clustered sub-pixel or outside the view frustum.

#### The Optimized Pipeline:
```
[Board Loaded]
      │
      ▼
Spatial Grid / Buckets per Side + Precomputed Net Index Sets
      │
[Render Frame]
      │
      ├── 1. Frustum Query (Rect visible_world)
      │      └── Retrieve visible entity IDs via Spatial Buckets (O(visible) instead of O(N))
      │
      ├── 2. Net Selection Fast-Path
      │      ├── If selected_net is active:
      │      │     Query net.pin_indices and net.via_indices intersecting visible_world
      │      │     Tessellate normal visible pins (non-selected) AND net-highlight pins directly
      │      │     Bypasses string comparison completely!
      │
      └── 3. LOD Decimation
             └── If screen_r < 0.75px and dense net (> 500 nodes):
                   Batch into unified 1x1 point rects or skip subpixel vias
```

1. **Pre-indexed Fast Path:** In `bvc-core::Board`, `Net` already contains `pin_indices: Vec<usize>` and `via_indices: Vec<usize>`. We add a `HashSet<usize>` or `BitVec`/`RoaringBitSet` (or pre-allocated lookup buffer) so membership test is $O(1)$ without string comparisons.
2. **Spatial Grid Partitioning:** Add a uniform grid spatial index (`SpatialGrid`) in `bvc-core` indexing pins and vias by mil cell coordinates (e.g. 500 mil cell size). Viewport queries only fetch entities in visible cells.
3. **LOD Thresholding:** When zoomed out such that `via.radius * scale < 0.8px`, vias on heavy nets (> 1,000 nodes) are rendered as batched single-pixel points or culled if sub-pixel density exceeds monitor resolution.

### 4.3 Preferences & Theme Tokens (`bvc-app/src/theme.rs`, `side_panel.rs`, `main.rs`)
1. **Preferences Structure:**
   ```rust
   #[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
   pub struct AppPreferences {
       pub theme_preset: ThemePreset,
       pub show_net_web: bool,
       pub high_density_lod: bool,
   }
   
   #[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
   pub enum ThemePreset {
       Dark,
       OpenBoardView,
       HighContrast,
       Light,
   }
   ```
2. **Tokens Customization:**
   - `Tokens::DARK` (Cadcraft default dark)
   - `Tokens::OPEN_BOARD_VIEW` (OpenBoardView dark palette: dark gray background, orange pins, gold highlights)
   - `Tokens::HIGH_CONTRAST` (OLED black, neon lime green, cyan, magenta)
   - `Tokens::LIGHT` (White/off-white background, deep charcoal outlines, high visibility pins)

---

## 5. Affected Areas & Files

| Component / File | Description of Modification |
| :--- | :--- |
| `bvc-core/src/board.rs` | Add spatial grid acceleration structure for fast frustum queries on pins/vias; provide fast bitset/lookup for net membership. |
| `bvc-core/src/model.rs` | Add orientation / rotation helpers if needed in geometry primitives. |
| `bvc-app/src/canvas.rs` | Update `Viewport` with `Rotation`, `mirror_x`, `mirror_y`. Replace full scans with spatial queries and net-indexed fast paths. Update mouse coordinate transforms. Apply LOD thresholding for dense nets. Respect `show_net_web` preference. |
| `bvc-app/src/theme.rs` | Add preset palettes (`DARK`, `OPEN_BOARD_VIEW`, `HIGH_CONTRAST`, `LIGHT`). |
| `bvc-app/src/main.rs` | Add Top Menu with "Actions" and "Preferences" menus. Add Preferences dialog window. Add rotation/mirror hotkeys. |

---

## 6. Risks & Mitigation

| Risk | Impact | Mitigation Strategy |
| :--- | :--- | :--- |
| Coordinate transform discrepancies with rotation/mirroring | High: Hover detection, hit clicks, or labels misaligned with board features. | Implement unit tests verifying bidirectional consistency: `screen_to_world(world_to_screen(p)) == p` across all 4 rotation angles and mirror combinations. |
| Spatial indexing memory or build time overhead on large boards | Low: Slight delay on initial file load. | Use a simple flat uniform grid (`Vec<Vec<usize>>` indexed by cell `(x_cell, y_cell)`) which builds in under 5ms for 100,000 pins/vias. |
| Visual artifacts with LOD decimation on dense nets | Medium: Ground vias appearing to vanish or pop during zoom. | Use smooth alpha-fade or transition to 1.5px dot mesh before culling completely. |

---

## 7. Rollback Plan

If spatial indexing or rotation introduces regressions:
1. Viewport transformations are isolated in `bvc-app/src/canvas.rs`: rotation angle defaults to `Rotation::Deg0` and mirror defaults to `false`, exactly preserving previous behavior.
2. The linear rendering fallback remains available if spatial grid lookups are conditionally bypassed.
3. Git commits will be cleanly phased:
   - Phase 1: Viewport rotation & mirroring in `canvas.rs` + Actions menu.
   - Phase 2: Theme presets & Preferences dialog.
   - Phase 3: Spatial indexing and high-density net fast path.

---

## 8. Dependencies

- No new external crates required!
- Built-in `rayon` handles parallel tessellation.
- `egui` 0.36 and `eframe` natively support modal dialog windows and top menu bars.

---

## 9. Success Criteria

1. **Performance:**
   - Loading `MS-V375 REV 7.0.cad` and selecting `GND` (4,755 nodes) allows fluid pan/zoom with frame tessellation time `< 8ms` on cache-miss frames.
   - Bypasses string comparisons during rendering passes.
2. **Preferences:**
   - Switching between `Cadcraft Dark`, `OpenBoardView Classic`, `High Contrast`, and `Light` dynamically updates the UI and canvas colors instantly.
   - Toggling "Show Net Web" immediately hides/shows ratline connections without affecting pin highlights.
3. **Actions:**
   - 90°, 180°, 270° CW/CCW rotations accurately reorient the board, pins, and outlines.
   - Hit testing (hovering and clicking pins/vias) functions precisely under all rotation and mirror configurations.
4. **Code Quality:**
   - Clean `cargo test --ignore-rust-version` passes with 0 failures.
   - `unsafe_code = "forbid"` maintained; zero panics or unwraps in library code.
