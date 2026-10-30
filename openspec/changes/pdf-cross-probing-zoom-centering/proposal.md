# Proposal: Center and Scale Matching Components on PDF Cross-Probing

## Intent

When clicking on a component in the boardview layout linked to a schematic PDF, the viewer synchronizes and navigates to the matching component designator. However, if the PDF view is currently at default or full-page scale (1.0x / 100%), component text and pin connections can appear too small on dense multi-page schematics to be immediately legible. Conversely, if the user has already manually zoomed in to inspect nearby circuitry (e.g. at 2.5x or 3.0x), resetting the zoom level disrupts their ongoing visual context.

This proposal ensures that when a component is selected and cross-probed to the PDF viewer:
1. The viewport is centered directly on the matching component marker.
2. If the active PDF zoom is at default or lower (<= 1.0), the zoom is automatically elevated to 200% (`zoom = 2.0`) to make the component and adjacent circuit nets clearly visible.
3. If the user already has an active zoom greater than 100% (`zoom > 1.0`), their existing zoom level is preserved, avoiding unwanted scale jumps while still centering on the component.

## Scope

### In Scope
- Update `PdfSchematicViewer::cross_probe` and viewport centering in `crates/ui-egui/src/lib.rs` / `crates/ui-egui/src/pdf.rs`.
- Define the zoom threshold rule: when cross-probing a component, if `self.zoom <= 1.0`, set `self.zoom = 2.0`; if `self.zoom > 1.0`, preserve `self.zoom`.
- Ensure subsequent centering calculations (`pan_offset`) calculate the exact center offset matching the target component token with the effective zoom applied.
- Automated tests verifying the zoom behavior (<= 1.0 becomes 2.0; > 1.0 is preserved; pan offset lands on token center).

### Out of Scope
- Affecting manual zoom actions (mouse wheel, pan gestures, keyboard zoom shortcuts).
- Reverse cross-probing zoom changes (clicking PDF tokens to center boardview items already has dedicated boardview fit presets).

## Capabilities

### New Capabilities
- None

### Modified Capabilities
- `pdf-schematic-auto-prompt`: Cross-probing navigation to components applies adaptive 200% zoom escalation when unzoomed and preserves existing zoom when enlarged.

## Approach

1. **Zoom Escalation in `cross_probe`**:
   In `crates/ui-egui/src/pdf.rs`, when a search match is found during component cross-probing:
   ```rust
   if self.zoom <= 1.0 {
       self.zoom = 2.0;
   }
   ```
2. **Accurate Centering under Dynamic Zoom**:
   In `crates/ui-egui/src/lib.rs`, the centering routine recalculates `effective_scale` using `self.pdf_viewer.zoom` immediately before computing `pan_offset`:
   ```rust
   let effective_scale = auto_scale * self.pdf_viewer.zoom;
   let render_size = orig_dim * effective_scale;
   // pan_offset calculation centered at hit.bbox.center()
   ```
   This ensures that the token appears dead-center in the PDF viewport irrespective of whether zoom was elevated or preserved.

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `crates/ui-egui/src/pdf.rs` | Modified | Update `cross_probe` to apply 200% default zoom floor |
| `crates/ui-egui/src/lib.rs` | Modified | Ensure centering math respects the newly adjusted zoom before rendering |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| User prefers bird's-eye view when selecting many components in sequence | Low | Preserves zoom if user zooms out to a specific custom factor or navigates manually. 200% provides an optimal balance for readable component designators. |

## Rollback Plan

Revert the zoom check in `PdfSchematicViewer::cross_probe`. The rest of cross-probing remains unaffected.

## Dependencies

- None.

## Success Criteria

- [ ] When cross-probing a component with PDF zoom at 1.0 (default), the PDF switches to page, sets zoom to 2.0 (200%), and centers the component.
- [ ] When cross-probing a component with PDF zoom already at 2.5 (250%), the zoom remains at 2.5 and the component is centered.
- [ ] When cross-probing a component with PDF zoom at 1.5 (150%), the zoom remains at 1.5.
- [ ] All automated tests pass.
