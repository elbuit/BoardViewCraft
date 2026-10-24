# Proposal: Zoom Window-Fit Defaults and Via Routing Display Preferences

## Intent

Users need a more efficient workflow when opening and inspecting boardviews, as well as greater control over net line clutter and via visibility:
1. **Window-Fit Visibility & Access:** When inspecting a board, the first intuitive zoom action in the Zoom menu should be "Fit to Window" (with continued support for the `F` shortcut key). Additionally, opening any boardview file currently centers the board without fitting zoom, often requiring manual zooming to see the complete PCB outline. Automatically fitting the board to the window upon loading removes manual repositioning friction.
2. **Via Display Routing in Preferences:** Currently, vias and net flight lines are always rendered either completely on or off via `show_net_web` without fine-grained choice for via connectivity geometry. Users need a preference setting to choose how vias are connected and displayed on selected nets:
   - **Radial:** Connect vias/pins outward from a central focal point / daisy-chain star topology.
   - **Fastest Path (Shortest / MST):** The current Euclidean Minimum Spanning Tree flight line algorithm linking pins and vias along optimal shortest segments.
   - **Hidden (Off):** Do not render vias on the net web / hide vias on the net lines.

## Scope

### In Scope
- **Zoom Menu Reorganization:** Make "Fit to Window (F)" / "Ajustar tamaño ventana (F)" the first option in the Zoom dropdown menu, followed by a separator and the standard zoom percentages (25%, 50%, 100%, 150%, 200%).
- **Retain & Consolidate 'F' Key:** Ensure pressing `F` continues to trigger "Fit to Window" seamlessly.
- **Auto-Fit on File Load:** In `BoardViewApp::load_file` (or initial frame post-load when canvas dimensions are known), automatically invoke `fit_to_rect` using the board's bounding box and canvas rect so the PCB instantly fills the viewport at optimal scale.
- **Via Display Mode Preference (`ViaDisplayMode`):** Add an enum in preferences with options:
  - `Radial` ("Radial"): Connects via nodes in star/radial paths.
  - `FastestPath` ("Camino más rápido / Shortest Path (MST)"): Current flight line optimization linking nearest nodes.
  - `Hidden` ("No mostrar / Off"): Suppresses vias in the net flight line / hides via net connections.
- **Preferences UI & i18n:** Add radio buttons or ComboBox in Preferences under "Visualization" / "Apariencia" allowing selection between Radial, Fastest Path, and Hidden, with full English, Spanish, and Catalan translations.
- **Net Web Tessellation Updates:** Update `generate_board_meshes` in `crates/bvc-app/src/canvas.rs` to respect the selected `ViaDisplayMode`.
- **Unit & Integration Tests:** Add tests covering auto-fit behavior, preference serialization, and net flight line via filtering.

### Out of Scope
- Interactive routing/trace editing (BoardViewCraft remains a high-performance read-only boardview viewer).
- Persistent camera viewport overrides per board hash.

## Capabilities

### New Capabilities
None.

### Modified Capabilities
- `boardview-zoom-and-viewport`: Adds automatic window-fit upon board load and places "Fit to Window (F)" as the primary Zoom menu entry.
- `boardview-preferences-and-visualization`: Adds `ViaDisplayMode` (`Radial`, `FastestPath`, `Hidden`) to Preferences and updates net flight line rendering accordingly.

## Approach

1. **Auto-fit on Load & Deferred Canvas Measurement:**
   - When `load_file` parses a board with a valid `bounding_box`, set a pending flag `pending_auto_fit = true` on `BoardViewApp`.
   - On the next frame when `CentralPanel` evaluates `show_canvas` and allocates the painter rect, if `pending_auto_fit` is true and `rect.width() > 10.0`, call `viewport.fit_to_rect(bbox, rect)` and reset the flag.
   - Update `Zoom` menu to list "Fit to Window (F)" as item 0.
2. **Via Display Mode Model:**
   - Define enum in `preferences.rs`:
     ```rust
     #[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
     pub enum ViaDisplayMode {
         #[default]
         FastestPath,
         Radial,
         Hidden,
     }
     ```
   - Persist in `Preferences` struct with backwards compatibility (`#[serde(default)]`).
3. **Net Web Rendering Logic in `canvas.rs`:**
   - When `show_net_web` is enabled:
     - If `ViaDisplayMode::Hidden`, do not append `via.pos` to the node list for flight line segments (only connect pins).
     - If `ViaDisplayMode::FastestPath`, retain existing Euclidean MST algorithm over active nodes.
     - If `ViaDisplayMode::Radial`, connect each via to the nearest pin or centroid of the net, producing clean radial spoke ratlines.
4. **Localization:**
   - Add strings for `toolbar.zoom_fit_window`, `prefs.via_display_mode`, `prefs.via_mode_radial`, `prefs.via_mode_fastest`, `prefs.via_mode_hidden` in English, Spanish, and Catalan.

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `crates/bvc-app/src/preferences.rs` | Modified | Add `ViaDisplayMode` enum, default field, and UI selector in Preferences modal |
| `crates/bvc-app/src/main.rs` | Modified | Auto-fit flag on load, first entry in Zoom dropdown, keyboard shortcut handler |
| `crates/bvc-app/src/canvas.rs` | Modified | Update `ViewCacheKey` and `generate_board_meshes` to support radial, fastest path, and hidden via modes |
| `crates/bvc-app/src/i18n.rs` | Modified | Add translations for zoom fit and via display options in EN, ES, CA |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Initial canvas rect has size 0 before first draw | Medium | Use `pending_auto_fit` flag and apply `fit_to_rect` once non-zero `rect` is allocated in `show_canvas` |
| Large nets with radial via mode degrade frame rate | Low | Limit max node count threshold as with MST, batch lines into single vertex mesh |
| Preferences migration from previous config files | Low | Use `#[serde(default)]` on `via_display_mode` |

## Rollback Plan

Revert changes to `main.rs`, `preferences.rs`, `canvas.rs`, and `i18n.rs` using standard git commit rollback. Preferences defaults ensure no legacy configuration files break.

## Dependencies

None. Builds upon existing egui and bvc-core structures.

## Success Criteria

- [ ] "Ajustar tamaño ventana (F)" is the first item in the Zoom dropdown menu.
- [ ] Pressing `F` continues to fit the board within the viewport.
- [ ] Loading any board file (.cad, .tvw, .brd, etc.) automatically frames and fits the board to the window.
- [ ] Preferences window includes via display options: Radial, Fastest Path (camino más rápido), and Hidden (no mostrar).
- [ ] Flight line web reflects the selected via display option instantly.
- [ ] All unit and regression tests pass with `cargo test --ignore-rust-version`.
