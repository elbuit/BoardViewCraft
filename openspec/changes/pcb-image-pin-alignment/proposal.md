# Proposal: Automated PCB Photo Alignment Using Pins

## Intent

Hardware repair technicians and reverse engineers using BoardViewCraft frequently overlay high-resolution physical PCB photographs on top of CAD boardviews. Manually panning, scaling, and fine-tuning rotation sliders to align the photo with CAD coordinates is tedious, imprecise, and time-consuming.

This proposal introduces an interactive semi-automated calibration and alignment workflow: the user selects between 2 and 5 known reference pins on the boardview CAD and marks their corresponding physical positions on the imported PCB photo. The application then automatically solves the optimal rigid/similarity transformation (scale, rotation, and translation) using closed-form analytical geometry (for 2 points) or least-squares estimation (Umeyama similarity transform for 3 to 5 points), directly updating the overlay slot's center, zoom, and fine rotation angle, and providing visual feedback including residual error metrics.

## Scope

### In Scope
- **Interactive Calibration Workflow**:
  - Modal or dedicated step-by-step overlay alignment mode in the UI.
  - Pairwise correspondence matching: select reference pin from CAD layout, then pinpoint its exact center on the photo overlay canvas (or vice-versa).
  - Support a minimum of 2 pins (exact similarity transform: translation, isotropic scale, 2D rotation) and up to 5 pins (over-determined system solved via Umeyama / Procrustes least-squares).
  - Ability to add, modify, re-pick, or delete correspondence pairs.
- **Transform Solver**:
  - Closed-form 2-point similarity calculation (exact fit).
  - 3-to-5-point least-squares similarity transformation (Umeyama algorithm) minimizing mean squared distance error.
  - Calculation and display of per-point residual error and overall Root Mean Square Error (RMSE) in mils and pixels, helping users identify misclicked points.
  - Handling of edge cases: collinear points, zero-distance pairs, or invalid degeneracies gracefully reported to the user without panics.
- **Overlay Slot Integration**:
  - Automatically calculate and apply updated `OverlaySlot` parameters:
    - `center: Point`
    - `zoom: f32` and `size: Vec2`
    - `fine_rotation_deg: f32`
  - Option to preview alignment before committing, or cancel back to prior settings.
  - Post-calibration auto-lock option to preserve alignment against accidental dragging.
- **UI & Controls**:
  - "Align with Pins..." button in the right side panel's Overlay tab and top overlay context controls.
  - Visual overlay on the canvas during alignment mode showing picked CAD pins (colored crosshairs / rings) and picked photo spots with connecting error vectors.
  - Status banner or floating guide instructing the user on current step ("Click pin on board, then click corresponding pad on photo").
- **Localization**:
  - Complete UI strings and instructions in English, Spanish, and Catalan.

### Out of Scope
- Fully automated black-box computer vision / feature descriptor matching (e.g. SIFT/ORB) without user pin selection (photo lighting, distortion, silkscreen wear make fully unsupervised matching brittle).
- Non-linear projective homography or lens barrel distortion warping (PCB photos are assumed reasonably planar and orthogonal; standard similarity/rigid transform is sufficient and avoids non-affine warping artifacts).
- More than 5 reference points (diminishing returns and added user friction; 2–5 is optimal for repair workflows).

## Capabilities

### New Capabilities
- `pcb-image-pin-alignment`: Interactive point-correspondence calibration tool computing optimal 2D similarity transformations (2 to 5 points) between CAD coordinates and image overlay coordinates, with residual error reporting.

### Modified Capabilities
- `pcb-image-overlay`: Integration with calibration solver results to update `OverlaySlot` translation, zoom/dimensions, and rotation; canvas overlay rendering for calibration markers and error vectors.

## Approach

1. **Alignment Solver (`crates/bvc-app/src/overlay_align.rs`)**:
   - Define data structures for calibration pairs:
     ```rust
     pub struct AlignmentPointPair {
         pub pin_id: usize,
         pub pin_name: String,
         pub part_name: String,
         pub cad_pos: Point,    // World mils
         pub photo_pos: Point,  // Image pixel coordinates (u, v) or unscaled overlay world mils
     }

     pub struct AlignmentResult {
         pub center: Point,
         pub zoom: f32,
         pub rotation_deg: f32,
         pub rmse: f32,
         pub residuals: Vec<f32>,
     }
     ```
   - **2-Point Exact Solve**:
     - Given CAD points $C_1, C_2$ and photo points $P_1, P_2$:
     - Vector CAD: $\vec{v}_C = C_2 - C_1$, Vector Photo: $\vec{v}_P = P_2 - P_1$.
     - Scale factor $s = \|\vec{v}_C\| / \|\vec{v}_P\|$.
     - Rotation $\Delta\theta = \text{atan2}(v_{C,y}, v_{C,x}) - \text{atan2}(v_{P,y}, v_{P,x})$.
     - Center translation calculated so transformed photo center matches the boardview world frame.
   - **3 to 5-Point Umeyama Solve**:
     - Compute centroids $\mu_C$ and $\mu_P$.
     - Center point sets: $x_i = C_i - \mu_C$, $y_i = P_i - \mu_P$.
     - Compute covariance matrix $H = \sum y_i x_i^T$.
     - Extract optimal 2D rotation $\theta$ and scale $s = \frac{\sum x_i \cdot R y_i}{\sum \|y_i\|^2}$.
     - Compute translation $t = \mu_C - s R \mu_P$.
     - Calculate residual error $r_i = \|C_i - (s R P_i + t)\|$ and RMSE $\sqrt{\frac{1}{N}\sum r_i^2}$.

2. **Interactive Calibration State Machine**:
   - Maintain `AlignmentSession` in `bvc-app`:
     - States: `Idle`, `SelectingCadPin { pair_index }`, `SelectingPhotoPoint { pair_index }`, `ReadyToApply`.
     - Highlight active CAD pin and render prompt.
     - When clicked on canvas in image mode, convert screen coordinate to photo UV / pixel coordinate.
     - Show list of pairs in side panel with individual delete and re-pick buttons.
     - When $N \ge 2$, calculate live preview and display calculated RMSE.

3. **Overlay Slot Application**:
   - Update `slot.center`, `slot.set_zoom(new_zoom)`, and `slot.fine_rotation_deg`.
   - Option to automatically lock the overlay once calibration is accepted.

4. **Side Panel & Canvas UI**:
   - Add "Pin Calibration" collapsible header or launcher in `side_panel.rs` under `Tab::Overlay`.
   - On canvas, render numbered calibration markers (e.g. 1 to 5) with distinct colors for CAD pins vs Photo targets, with dotted connection lines representing residuals when solving with $\ge 3$ points.

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `crates/bvc-app/src/overlay_align.rs` | New | Mathematical solver (2-point closed form, 3-5 point Umeyama similarity transform, residual calculation) |
| `crates/bvc-app/src/overlay.rs` | Modified | Add helper methods on `OverlaySlot` for coordinate conversions and applying alignment results |
| `crates/bvc-app/src/side_panel.rs` | Modified | Add interactive alignment UI, pin pair table, RMSE display, and Apply/Cancel actions |
| `crates/bvc-app/src/canvas.rs` | Modified | Handle click events in calibration mode; render calibration markers, crosshairs, and error vectors |
| `crates/bvc-app/src/main.rs` | Modified | Manage active calibration state and interaction modes |
| `crates/bvc-app/src/i18n.rs` | Modified | Localization strings in English, Spanish, and Catalan |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Degenerate geometry (e.g. user selects two identical pins or pins extremely close together) | Medium | Check distance threshold between reference pins; display clear error warning ("Selected pins are too close to determine orientation") and disable Apply until valid |
| Photo perspective distortion / non-orthographic camera angles causing high residuals | Medium | Display per-pin residuals in mils/pixels and overall RMSE. Highlight pins with unusually high error in red so the user can re-pick or pick a better pin |
| Accidental click handling conflicts between normal pin selection and alignment picking | Low | Explicit calibration modal/mode with visually distinct canvas cursor, crosshairs, and prompt bar |

## Rollback Plan

The alignment module `overlay_align.rs` is purely additive. If issues arise, disabling the "Align with Pins..." button reverts the system to existing manual sliders and drag-to-align functionality without breaking overlay persistence or rendering.

## Dependencies

- No external crate dependencies required (standard 2D vector/matrix math using existing `glam` or `egui::Vec2` / `bvc_core::Point` primitives).

## Success Criteria

- [ ] User can initiate "Align with Pins" calibration for a loaded overlay photo.
- [ ] User can select between 2 and 5 pin pairs by clicking a pin on the boardview and its corresponding location on the photo.
- [ ] 2-point alignment produces an exact rigid/similarity fit matching the two selected points.
- [ ] 3-to-5-point alignment solves optimal similarity via least squares and displays residual errors and RMSE.
- [ ] Applying alignment updates the overlay's center, scale/zoom, and rotation angle accurately.
- [ ] Edge cases (coincident points, degenerate collinear points) are caught with user-friendly warnings without panics.
- [ ] All unit tests for 2-point and N-point solvers pass with near-zero numerical tolerance.
