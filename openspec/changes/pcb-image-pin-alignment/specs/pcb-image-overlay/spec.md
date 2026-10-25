# Spec: PCB Background Image Overlay (Delta)

## Capability: `pcb-image-overlay`

Extension of PCB background image overlay to integrate automated pin calibration results, visual rendering of calibration targets, and post-alignment state management.

---

### MODIFIED Requirements

#### 2. Overlay Transformation Properties
- Each overlay slot (`OverlaySlot`) SHALL store:
  - `file_path`: Path of the loaded photo.
  - `center`: World coordinate point `(x, y)` in mils.
  - `base_size`: Original fitted base size in board world mils.
  - `zoom`: Zoom multiplier (`1.0 = 100%`).
  - `size`: World width and height in mils (`base_size * zoom`).
  - `aspect_locked`: Boolean indicating if width/height scaling is locked to aspect ratio.
  - `opacity`: Alpha multiplier in range `0.0` to `1.0`.
  - `fine_rotation_deg`: Fine rotation angle in degrees `[-180.0, 180.0]` relative to PCB orientation.
  - `locked`: Boolean lock toggle preventing accidental dragging or resizing.
  - `visible`: Boolean visibility toggle.
- When an alignment result is committed from `pcb-image-pin-alignment`:
  - `slot.center` SHALL be set to the calculated optimal world center coordinate.
  - `slot.zoom` SHALL be set to the calculated scale factor relative to the base image dimensions.
  - `slot.size` SHALL be updated to `slot.base_size * slot.zoom`.
  - `slot.fine_rotation_deg` SHALL be set to the calculated optimal rotation angle normalized to `[-180.0, 180.0]`.

#### 3. Canvas Visual Rendering Pipeline
- When an active alignment session is in progress:
  - The canvas SHALL render distinct visual calibration targets:
    - Numbered CAD pin indicators (e.g., circled `1` through `5` with high-contrast colored crosshairs) positioned at the selected boardview pin coordinates.
    - Numbered photo mark indicators (e.g., diamond or square markers with matching numeric labels) positioned at the marked photo locations.
  - When $\ge 2$ pairs are defined, the canvas SHALL render visual residual error vectors:
    - A line connecting each CAD pin coordinate to the transformed photo mark position.
    - Color-coded residual severity (e.g., green for low residual $< 2.0$ mils, yellow for moderate residual, red for outlier residual $> 5.0$ mils).
  - An interactive status banner or floating HUD prompt SHALL be rendered across the top or bottom of the canvas indicating the current active calibration step (e.g., *"Step 2/3: Click pin on CAD layout"* or *"Step 2/3: Click corresponding pad on photo"*).

#### 4. Post-Alignment Auto-Lock Behavior
- The overlay calibration UI SHALL provide an "Auto-lock overlay when applied" option (default: `true`).
- When "Apply Alignment" is executed and auto-lock is enabled:
  - `slot.locked` SHALL be set to `true`.
  - `slot.locked_board_rotation_deg`, `slot.locked_board_sx`, and `slot.locked_board_sy` SHALL capture the board's current transformation state to lock the overlay firmly to the PCB coordinate space.
  - Accidental canvas drag gestures (`Alt + Left Drag` or `Alt + Scroll`) SHALL be ignored until explicitly unlocked by the user.

#### 5. Right Side Panel Tab ("Overlay")
- Add a dedicated "Pin Alignment" section within the `Overlay` tab in `side_panel.rs`:
  - "Align with Pins..." button to initiate an interactive calibration session.
  - While calibration is active:
    - A correspondence table listing pin pairs (1 to 5) with CAD Pin name/part, CAD coordinate, Photo coordinate, and individual residual error in mils.
    - Action buttons per row: "Re-pick" and "Delete".
    - Calculated overall RMSE metric in bold (e.g., *"RMSE: 1.2 mils / 0.8 px"*).
    - "Add Pair" button (disabled when pair count is 5).
    - "Apply Alignment" button (enabled when pairs $\ge 2$ and geometry is non-degenerate).
    - "Cancel" button to abandon changes and revert overlay transformation.
    - Auto-lock checkbox option.

---

### Scenarios

#### Scenario 1: Applying Alignment Updates OverlaySlot Properties
- **GIVEN** an active calibration session with 3 validated pin pairs
- **AND** calculated optimal parameters: center $(1500.0, 2200.0)$ mils, scale $1.25$, fine rotation $-12.5^\circ$
- **WHEN** the user clicks "Apply Alignment"
- **THEN** `OverlaySlot.center` is updated to $(1500.0, 2200.0)$ mils
- **AND** `OverlaySlot.zoom` is set to $1.25$ and `OverlaySlot.size` is recomputed accordingly
- **AND** `OverlaySlot.fine_rotation_deg` is updated to $-12.5^\circ$
- **AND** the photo on the canvas updates immediately to reflect the new transform.

#### Scenario 2: Auto-Locking Overlay Upon Alignment Application
- **GIVEN** the "Auto-lock overlay when applied" checkbox is checked
- **WHEN** the user commits an alignment via "Apply Alignment"
- **THEN** `OverlaySlot.locked` is set to `true`
- **AND** a padlock icon indicates the locked state in the side panel
- **AND** dragging with `Alt + Left Click` does not alter the overlay position until unlocked.

#### Scenario 3: Real-Time Canvas Residual Error Vectors Rendering
- **GIVEN** an active calibration session with 3 pin pairs selected
- **WHEN** the preview similarity transform is calculated
- **THEN** the canvas renders error vectors between each CAD pin and its projected photo target
- **AND** residual error metrics are displayed adjacent to each pin target on the canvas in mils.

#### Scenario 4: Reverting on Session Cancellation
- **GIVEN** an overlay slot with initial center $(1000.0, 1000.0)$ and rotation $0.0^\circ$
- **AND** a calibration session showing a preview with rotation $45.0^\circ$
- **WHEN** the user clicks "Cancel" or closes the calibration panel
- **THEN** the overlay preview is discarded
- **AND** the overlay slot properties remain exactly at center $(1000.0, 1000.0)$ and rotation $0.0^\circ$.
