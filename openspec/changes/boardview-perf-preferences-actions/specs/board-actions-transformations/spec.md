# Specification: Board Actions & Viewport Transformations

- **Capability:** `board-actions-transformations`
- **Change:** `boardview-perf-preferences-actions`
- **Status:** Draft / Ready for Review
- **Target Components:** `bvc-app::canvas`, `bvc-app::main`, `bvc-core::model`

---

## 1. Overview & Problem

When troubleshooting physical PCBs on a repair workbench, the board under inspection is frequently positioned at various angles (e.g. rotated 90° clockwise, 180° upside down, or inspected from the reverse side under a microscope).

In the current implementation:
- Only a basic `Top / Bottom` toggle exists, which hardcodes an X-inversion.
- Users cannot rotate the view by 90°, 180°, or 270°.
- Users cannot perform arbitrary horizontal (`Mirror X`) or vertical (`Mirror Y`) reflections.
- Screen-to-world coordinate transformations and hit-testing routines assume zero rotation, making any manual rotation attempt break mouse selection and bounding box frustum culling.

This specification establishes an Actions menu, comprehensive viewport orientation primitives, and a unified 2D affine transformation pipeline that preserves hit-testing accuracy and rendering correctness across all rotations and reflections.

---

## 2. Requirements

### 2.1 Actions Menu & User Interactions

- **REQ-ACT-010 [Actions Top Menu]:**
  The top menu bar in `bvc-app` MUST provide an `Actions` dropdown menu containing the following operations:
  - `Rotate 90° CW`
  - `Rotate 90° CCW`
  - `Rotate 180°`
  - `Flip Side (Top / Bottom)`
  - `Mirror Horizontally (Mirror X)`
  - `Mirror Vertically (Mirror Y)`
  - `Reset Orientation`

- **REQ-ACT-011 [Keyboard Shortcuts]:**
  The application MUST bind the following keyboard shortcuts to actions when the canvas or main window has focus:
  - `R`: Rotate 90° Clockwise
  - `Shift+R`: Rotate 90° Counter-Clockwise
  - `Space`: Flip Side (toggle between `Side::Top` and `Side::Bottom`)
  - `M`: Toggle Horizontal Mirror (`mirror_x`)
  - `Shift+M`: Toggle Vertical Mirror (`mirror_y`)
  - `F`: Fit board to window (preserving current orientation)

---

### 2.2 Viewport Orientation Primitives

- **REQ-ACT-020 [Rotation Discrete Enum]:**
  Orientation rotation MUST be defined by a discrete 4-state enum:
  ```rust
  #[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
  pub enum Rotation {
      #[default]
      Deg0,
      Deg90,
      Deg180,
      Deg270,
  }
  ```

- **REQ-ACT-021 [Rotation Progression Arithmetic]:**
  - Clockwise rotation (`rotate_cw`) SHALL advance:
    $$\text{Deg0} \rightarrow \text{Deg90} \rightarrow \text{Deg180} \rightarrow \text{Deg270} \rightarrow \text{Deg0}$$
  - Counter-clockwise rotation (`rotate_ccw`) SHALL advance:
    $$\text{Deg0} \rightarrow \text{Deg270} \rightarrow \text{Deg180} \rightarrow \text{Deg90} \rightarrow \text{Deg0}$$
  - 180° rotation (`rotate_180`) SHALL advance by 2 steps:
    $$\text{Deg0} \leftrightarrow \text{Deg180},\quad \text{Deg90} \leftrightarrow \text{Deg270}$$

- **REQ-ACT-022 [Updated Viewport State]:**
  The `Viewport` struct in `bvc-app::canvas` MUST be extended to include:
  ```rust
  pub struct Viewport {
      pub center: Point,
      pub scale: f32,
      pub side: Side,
      pub rotation: Rotation,
      pub mirror_x: bool,
      pub mirror_y: bool,
  }
  ```

- **REQ-ACT-023 [Side Flip and Mirror Semantics]:**
  - By convention in PCB CAD, viewing the bottom layer inherently mirrors the X axis so that features align with viewing through the substrate.
  - The effective horizontal reflection factor $S_x$ MUST be calculated as:
    $$S_x = ( \text{if } (\text{side} == \text{Side::Bottom}) \oplus \text{mirror\_x} \text{ then } -1.0 \text{ else } 1.0 )$$
  - The effective vertical reflection factor $S_y$ MUST be calculated as:
    $$S_y = ( \text{if } \text{mirror\_y} \text{ then } -1.0 \text{ else } 1.0 )$$

---

### 2.3 Viewport Transformation Mathematics

- **REQ-ACT-030 [World to Screen Transformation]:**
  Given world coordinate point $P_w = (x_w, y_w)$, board center $C = (c_x, c_y)$, scale $s$, canvas center $S_c = (\text{center}_x, \text{center}_y)$, effective mirrors $(S_x, S_y)$, and rotation angle $\theta \in \{0, \frac{\pi}{2}, \pi, \frac{3\pi}{2}\}$:
  1. Offset relative to board center:
     $$\Delta x = (x_w - c_x) \times S_x$$
     $$\Delta y = (y_w - c_y) \times S_y$$
  2. Apply CAD-to-Screen Y inversion (CAD $+Y$ is up, screen $+Y$ is down):
     $$\Delta y_{\text{cad}} = -\Delta y$$
  3. Apply 2D screen-plane rotation by $\theta$:
     $$x_{\text{rot}} = \Delta x \cos \theta - \Delta y_{\text{cad}} \sin \theta$$
     $$y_{\text{rot}} = \Delta x \sin \theta + \Delta y_{\text{cad}} \cos \theta$$
  4. Apply uniform scale and translate to canvas screen center:
     $$\text{screen}_x = S_c.x + x_{\text{rot}} \times s$$
     $$\text{screen}_y = S_c.y + y_{\text{rot}} \times s$$

- **REQ-ACT-031 [Screen to World Inverse Transformation]:**
  The inverse mapping `screen_to_world` MUST be the exact mathematical inverse of `world_to_screen`:
  1. Un-center and un-scale from screen coordinates:
     $$x_{\text{rot}} = (\text{screen}_x - S_c.x) / s$$
     $$y_{\text{rot}} = (\text{screen}_y - S_c.y) / s$$
  2. Reverse rotation by $-\theta$:
     $$\Delta x = x_{\text{rot}} \cos(-\theta) - y_{\text{rot}} \sin(-\theta) = x_{\text{rot}} \cos \theta + y_{\text{rot}} \sin \theta$$
     $$\Delta y_{\text{cad}} = x_{\text{rot}} \sin(-\theta) + y_{\text{rot}} \cos(-\theta) = -x_{\text{rot}} \sin \theta + y_{\text{rot}} \cos \theta$$
  3. Reverse CAD Y inversion:
     $$\Delta y = -\Delta y_{\text{cad}}$$
  4. Reverse reflections and restore world center:
     $$x_w = c_x + \frac{\Delta x}{S_x}$$
     $$y_w = c_y + \frac{\Delta y}{S_y}$$

- **REQ-ACT-032 [Bijective Invariance Contract]:**
  For any arbitrary world point $P$, any scale $s > 0$, any rotation $\theta$, any mirror flags, and any canvas rectangle, the round-trip condition:
  $$\| \text{screen\_to\_world}(\text{world\_to\_screen}(P)) - P \| < 10^{-4}\,\text{mils}$$
  MUST hold unconditionally.

- **REQ-ACT-033 [Visible Frustum Calculation]:**
  `visible_world_rect(&self, canvas_rect: EguiRect) -> Rect` MUST compute the screen-to-world unprojected points of all four screen corners:
  $$P_1 = \text{unproject}(\text{top-left}), \quad P_2 = \text{unproject}(\text{top-right})$$
  $$P_3 = \text{unproject}(\text{bottom-left}), \quad P_4 = \text{unproject}(\text{bottom-right})$$
  The visible world bounding box SHALL be defined as the axis-aligned bounding box (AABB) enclosing $\{P_1, P_2, P_3, P_4\}$.

---

### 2.4 Hit-Testing and Rendering Integration

- **REQ-ACT-040 [Hit Testing Consistency]:**
  Mouse interactions (`find_pin_near`, `find_via_near`, component selection) MUST pass mouse screen positions through `screen_to_world` before evaluating proximity. Hover detection and selection MUST function identically regardless of active rotation and mirror flags.

- **REQ-ACT-041 [Part Label Orientation]:**
  When rendering text labels for components and pins, text orientation SHOULD remain readable (upright or aligned with the rotated board axis) without upside-down inversions when flipped.

- **REQ-ACT-042 [View Cache Key Invalidation]:**
  `ViewCacheKey` MUST include `rotation: Rotation`, `mirror_x: bool`, and `mirror_y: bool` so that any orientation change invalidates cached meshes immediately.

---

## 3. Scenarios & Acceptance Criteria

### Scenario 1: Clockwise Rotation via Shortcut
- **Given** a board viewed in default orientation (`Deg0`),
- **When** the user presses `R`,
- **Then** `viewport.rotation` transitions to `Deg90`,
- **And** the board canvas redraws rotated 90 degrees clockwise,
- **And** clicking a visible pad accurately selects that exact pin.

### Scenario 2: Bottom Layer Inspection with Horizontal Mirror
- **Given** the user is viewing the board top layer,
- **When** the user presses `Space` to switch to `Side::Bottom`,
- **Then** the board flips horizontally matching through-substrate perspective,
- **When** the user additionally presses `M` (`Mirror X`),
- **Then** the horizontal reflection inverts again, showing true bottom CAD coordinates.

### Scenario 3: Bidirectional Coordinate Invariance
- **Given** any combination of `rotation` $\in \{0, 90, 180, 270\}$, `side` $\in \{\text{Top}, \text{Bottom}\}$, `mirror_x` $\in \{\text{true}, \text{false}\}$, `mirror_y` $\in \{\text{true}, \text{false}\}$,
- **When** any arbitrary point $P = (1234.56, -789.12)$ is projected to screen coordinates via `world_to_screen`,
- **And** the resulting screen coordinate is unprojected via `screen_to_world`,
- **Then** the reconstructed point matches $P$ within $0.0001\,\text{mils}$.

---

## 4. Verification Contracts

| Verification Target | Test Method | Pass Criteria |
| :--- | :--- | :--- |
| Coordinate round-trip invariance | Property / unit test (`test_viewport_transform_roundtrip`) | Round-trip error $< 10^{-4}\,\text{mils}$ for 1,000 random points across all 16 rotation/mirror combinations. |
| Rotation cycle | Unit test (`test_rotation_progression`) | 4 CW rotations return to start; 4 CCW rotations return to start; 2 180° rotations return to start. |
| Frustum corner coverage | Unit test (`test_visible_world_rect_coverage`) | Unprojected corners of canvas rectangle are completely contained within the computed `visible_world_rect`. |
| Hit-testing invariance | Unit test (`test_find_pin_near_transformed`) | A pin located at $(x, y)$ is detected by `find_pin_near` when hovering at its computed `world_to_screen` position under any orientation. |
| View cache invalidation | Unit test (`test_cache_key_invalidation_on_transform`) | Changing `rotation`, `mirror_x`, or `mirror_y` results in unequal `ViewCacheKey` values. |
