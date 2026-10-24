# Proposal: Dynamic Component Label Sizing and Unnamed Pad/Via Filtering

## Intent

In BoardViewCraft, two visual presentation issues degrade readability on dense PCB boards:
1. **Uniform component text size:** Component designator labels (e.g. `U1`, `CPU`, `R1234`) are rendered at a constant fixed font size (12.0 pt), regardless of physical part footprint dimensions. Large ICs and connectors appear with tiny, hard-to-read labels relative to their area, while tiny 0402 passives compete for the same label prominence.
2. **Visual clutter from unnamed/isolated pads:** Dense formats like `.tvw` and `.cad` define thousands of unlabeled stitching vias, test points, and probe pads with empty or unnamed net references. Unlike reference viewers (such as TeboView and Wrench-board), which either keep vias/stitching points hidden by default, filter unnamed test pads, or offer visibility toggles, BoardViewCraft renders every unnamed via dot unconditionally, cluttering the canvas during inspection.

## Scope

### In Scope
- **Dynamic / Proportional Part Label Sizing:** Scale component designator text size proportionally based on the component bounding box footprint area/dimensions (e.g., clamp between 10.0 pt and 28.0 pt with proportional footprint scaling) so larger components receive distinctly legible, prominent labels.
- **Filter Unnamed Vias / Test Points Toggle & Visibility:**
  - Add preference toggle `show_unnamed_vias` (or `show_unnamed_pads`, default: `false` or toggleable) to prevent unrouted/unnamed via dots from cluttering the viewport when zoomed out.
  - Add preference toggle `show_vias` (default: `true`) to allow toggling all vias on/off, matching standard boardview workflow (Wrench-board / OpenBoardView).
  - Ensure unnamed vias/pads associated with an active selection or search remain visible when highlighted.
- **Preferences & UI Integration:** Update preferences modal and canvas controls with i18n support in English, Spanish, and Catalan.

### Out of Scope
- Modifying underlying parser geometry or stripping vias from board models in memory.
- Dynamic GPU vector font generation (relying on egui proportional FontId scaling).

## Capabilities

### New Capabilities
- `dynamic-part-label-sizing`: Component label font sizing scaled dynamically to physical bounding box dimensions while respecting LOD decimation.
- `via-and-unnamed-pad-filtering`: Viewport rendering filters and user preference toggles for all vias and unnamed vias/test pads.

### Modified Capabilities
- None

## Approach

1. **Canvas Label Sizing ([crates/bvc-app/src/canvas.rs](file:///media/no_raid/devel/boarview/BoardViewCraft/crates/bvc-app/src/canvas.rs)):**
   - Compute `footprint_dim = (c_rect.width().min(c_rect.height()) as f32 * vp.scale)`.
   - Scale label font size: calculate base size `(10.0 + (footprint_dim - 24.0) * 0.15).clamp(10.0, 28.0)`.
   - High-priority large ICs and multi-pin parts naturally receive larger, more readable text while maintaining viewport LOD thresholds.
2. **Via & Unnamed Pad Filtering ([crates/bvc-app/src/canvas.rs](file:///media/no_raid/devel/boarview/BoardViewCraft/crates/bvc-app/src/canvas.rs) & [crates/bvc-app/src/preferences.rs](file:///media/no_raid/devel/boarview/BoardViewCraft/crates/bvc-app/src/preferences.rs)):**
   - In `generate_board_meshes`, evaluate `via.net_name.is_empty() || via.net_name == "UNCONNECTED"`.
   - When `!prefs.show_unnamed_vias`, skip rendering unnamed vias unless `is_net_selected` or `is_hovered`.
   - Add toggle in Preferences dialog and shortcuts/toolbar for instant decluttering.

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `crates/bvc-app/src/canvas.rs` | Modified | Proportional label font sizing and unnamed via render filtering |
| `crates/bvc-app/src/preferences.rs` | Modified | Add `show_vias` and `show_unnamed_vias` preference fields and UI toggles |
| `crates/bvc-app/src/i18n.rs` | Modified | Localized string keys in English, Spanish, and Catalan |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Label text clipping bounding box on large labels | Low | Clamp max font size to 28.0 pt and enforce `Align2::CENTER_CENTER` |
| Hiding critical unnamed test points during diagnosis | Medium | Keep unnamed elements visible on hover, selection, and provide an explicit toggle in UI |

## Rollback Plan

Revert git changes to `crates/bvc-app/src/canvas.rs`, `preferences.rs`, and `i18n.rs`. Existing config schema backwards compatibility is preserved via serde `#[serde(default)]`.

## Dependencies

- None

## Success Criteria

- [ ] Large ICs and connectors display proportionally larger designator text than small passives.
- [ ] Unnamed vias/test points can be hidden, eliminating visual speckling on dense boards like `test1.tvw`.
- [ ] Selected and hovered elements always remain clearly visible regardless of filter settings.
- [ ] All unit and integration tests compile and pass.
