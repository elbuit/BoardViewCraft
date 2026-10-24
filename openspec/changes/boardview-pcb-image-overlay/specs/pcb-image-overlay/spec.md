# Spec: PCB Background Image Overlay

## Capability: pcb-image-overlay

### Requirements

1. **Image Loading & Per-Side Slot Management**:
   - The application shall support loading separate reference photos for the `Top` side and `Bottom` side of the PCB.
   - Formats supported: PNG, JPEG, BMP, and WebP.
   - Users can load or replace a photo via a native file picker dialog from either the top toolbar or the dedicated side panel tab.
   - When the user flips the PCB view (Top vs Bottom), the canvas shall automatically display the photo corresponding to that side.

2. **Overlay Transformation Properties**:
   - Each overlay (Top/Bottom) shall independently store:
     - `file_path`: Path of the loaded photo.
     - `center`: World coordinate point `(x, y)` in mils. Default is center of PCB bounding box.
     - `size`: World width and height in mils. Default fits the PCB bounding box while preserving the image's original aspect ratio.
     - `aspect_locked`: Boolean indicating if width/height scaling is locked to aspect ratio (default: `true`).
     - `opacity`: Alpha multiplier in range `0.0` (fully transparent) to `1.0` (fully opaque). Default is `0.5` (50%).
     - `fine_rotation_deg`: Fine rotation angle in degrees `[-180.0, 180.0]` relative to PCB orientation. Default is `0.0`.
     - `locked`: Boolean lock toggle preventing accidental dragging or resizing (default: `false`).
     - `visible`: Boolean visibility toggle (default: `true`).

3. **Canvas Rendering Pipeline**:
   - When an active overlay is present for the current side and `visible == true`:
     - The photo texture shall be rendered behind all PCB geometry (traces, fills, parts, pins, vias) and in front of the empty canvas background.
     - The image quad shall be projected into screen coordinates using the active `Viewport` transform (applying center, size, PCB rotation + fine rotation, mirror X/Y, scale, and pan).
     - Alpha transparency shall modulate the texture color.

4. **Interactive Canvas Gizmo / Direct Manipulation**:
   - When an overlay is unlocked (`locked == false`):
     - `Alt + Left Drag`: Pans the image center in world coordinates.
     - `Alt + Scroll Wheel`: Zooms/scales the image size relative to cursor position.
   - Normal clicks (without `Alt`) continue to perform pin/via/part selection without interference.

5. **Right Side Panel Tab ("Overlay")**:
   - Add a tab `Overlay` / `Foto` in the side panel.
   - Controls provided:
     - File info & "Load / Replace Photo" button.
     - "Remove Photo" button.
     - "Lock Position" toggle with visual padlock indicator.
     - "Visible" toggle.
     - Opacity slider (`0%` to `100%`).
     - Position controls: `X` and `Y` coordinate numeric drag-values.
     - Size controls: `Width` and `Height` numeric drag-values, plus an "Aspect Ratio Lock" toggle.
     - Fine Rotation slider (`-180.0°` to `+180.0°`) and a "Reset Alignment" button.

6. **Localization**:
   - All labels and tooltips shall be localized in English, Spanish, and Catalan.
