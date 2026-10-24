# Proposal: PCB Background Image Overlay

## Intent

Hardware repair technicians and reverse engineers frequently need to compare real physical PCB high-resolution photos against the boardview CAD layout. This facilitates identifying burnt or missing components, validating test pad coordinates, or tracing physical tracks. This proposal adds support for loading a PCB background photo overlay, adjusting its position, scale, opacity (alpha transparency), rotation, and side affiliation, and managing all overlay properties from the right side panel and top toolbar.

## Scope

### In Scope
- **Image Loading & Decoding**: Support loading common image formats (PNG, JPG, BMP, WebP) via native file dialog (`rfd`).
- **Canvas Rendering**: Render the image under the board elements (above canvas background, below PCB traces/pins/vias) respecting the canvas viewport transform.
- **Transform Adjustments**:
  - Opacity slider (0% to 100% transparency).
  - Offset translation (X / Y in mils/world units).
  - Scale / Dimensions adjustment (width/height ratio locked or free, scaling factor).
  - Rotation angle fine-tuning (independent or aligned with PCB rotation).
  - Side association (Top side photo vs Bottom side photo vs Both sides).
- **Direct Interactive Manipulation (Canvas Gizmo)**:
  - Dragging the image on the canvas to move it when in "Adjust Image" mode (or via modifier key e.g. Alt+drag).
- **Right Side Panel Tab**:
  - A dedicated "Photo" / "Overlay" tab (or dynamic panel section) showing image thumbnail/status, opacity slider, position X/Y inputs, scale/size inputs, reset button, and remove photo button.
- **Top Toolbar Integration**:
  - "Load Photo..." button or menu action, with quick visibility toggle (Show/Hide overlay).

### Out of Scope
- Automated feature detection / computer vision matching (future enhancement).
- Image editing filters (contrast/brightness/color balance) beyond alpha transparency.
- Multiple simultaneous layered photos on the same side (initially 1 photo per side: Top and Bottom).

## Suggested Additional Enhancements (included in proposal for consideration)
1. **Per-Side Photos (Top & Bottom)**: Allow loading one photo for the TOP side and another for the BOTTOM side, automatically switching or hiding when flipping the board with Space bar.
2. **Lock Overlay (Candado de Bloqueo)**: Once aligned, lock the photo position and scale so normal panning and pin selection don't accidentally displace the background image.
3. **Quick Invert Colors**: A toggle to invert the photo colors (useful for dark silkscreen / white silkscreen contrast against dark themes).

## Capabilities

### New Capabilities
- `pcb-image-overlay`: Loading, positioning, scaling, opacity control, and side-filtering of background reference PCB photos beneath the boardview geometry.

### Modified Capabilities
- `side-panel-inspection`: Add dedicated "Overlay" tab or controls in the right side panel when a photo is active.

## Approach

1. **Image Storage & Texture Management**:
   - Add `image` dependency (with PNG/JPEG features) in `crates/bvc-app/Cargo.toml`.
   - Maintain `PcbImageOverlay` state in `bvc-app`:
     ```rust
     pub struct PcbImageOverlay {
         pub file_path: Option<PathBuf>,
         pub texture: Option<egui::TextureHandle>,
         pub side: Side, // Top, Bottom, or Both
         pub center: Point,
         pub size: Vec2, // in board world units (mils)
         pub opacity: f32, // 0.0 to 1.0
         pub rotation_deg: f32,
         pub locked: bool,
         pub visible: bool,
     }
     ```
   - Lazily allocate or retain `egui::TextureHandle` on image load.
2. **Canvas Pipeline Integration**:
   - In `BoardCanvas::show`, before drawing board outline meshes, if overlay is enabled and matches current viewport side:
     - Compute the transformed screen rectangle and UVs based on `overlay.center`, `overlay.size`, and `overlay.rotation_deg`.
     - Render the texture with `Color32::from_white_alpha((overlay.opacity * 255.0) as u8)`.
3. **Side Panel**:
   - Add a 4th tab `Tab::Overlay` in `crates/bvc-app/src/side_panel.rs`.
   - Expose sliders and controls: Opacity, Position X/Y, Width/Height, Rotation, Lock toggle, and Remove button.

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `crates/bvc-app/Cargo.toml` | Modified | Add `image` crate dependency |
| `crates/bvc-app/src/overlay.rs` | New | Overlay data structure, texture loading, world-to-screen quad calculation |
| `crates/bvc-app/src/canvas.rs` | Modified | Render background image quad behind PCB meshes |
| `crates/bvc-app/src/side_panel.rs` | Modified | Add "Overlay" tab with sliders and alignment controls |
| `crates/bvc-app/src/main.rs` | Modified | Add "Load Photo" action and wire overlay state |
| `crates/bvc-app/src/i18n.rs` | Modified | Add translations for overlay controls (EN, ES, CA) |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Memory overhead from huge PCB photos (e.g. 50MB raw photos) | Medium | Downsample or clamp max texture dimensions (e.g. 4096px or 8192px GPU limit) |
| Inconvenient alignment if only numeric inputs are available | Medium | Support direct Alt+Drag on canvas for moving and Alt+Wheel for scaling when unlocked |

## Rollback Plan

Remove `overlay.rs` and the render call in `canvas.rs`; restore previous `side_panel.rs` tabs.

## Dependencies

- `image` crate (standard pure-Rust image decoding).
- `rfd` (already present in dependencies).

## Success Criteria

- [ ] User can open an image file (.png, .jpg) via file dialog.
- [ ] Image appears behind the PCB boardview aligned to world coordinates.
- [ ] Opacity slider in side panel smoothly adjusts transparency from 0% to 100%.
- [ ] User can move and resize the image both from the side panel and interactively on canvas.
- [ ] Photo respects Top/Bottom side flipping.
