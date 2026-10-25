# Spec: PCB Image Pin Alignment

## Capability: `pcb-image-pin-alignment`

Interactive point-correspondence calibration tool computing optimal 2D similarity transformations (scale, 2D rotation, translation) between CAD boardview coordinates and photo overlay coordinates, supporting 2 to 5 reference pin pairs with residual error metrics.

---

### Requirements

#### 1. Pin Pair Correspondence Constraints
- The alignment system SHALL accept a minimum of 2 pin correspondence pairs and a maximum of 5 pin correspondence pairs.
- Each correspondence pair SHALL consist of:
  - CAD boardview coordinate (`cad_pos: Point` in board world mils, associated with a recognized pin identifier, pin name, and part name).
  - Photo overlay coordinate (`photo_pos: Point` representing physical position on the reference photo).
- The system SHALL reject solving when fewer than 2 valid correspondence pairs are provided.
- The system SHALL prevent adding more than 5 correspondence pairs.
- The user SHALL be able to select, re-pick, or delete individual correspondence pairs at any stage before applying.

#### 2. Closed-Form 2-Point Similarity Solver
- When exactly 2 correspondence pairs $(C_1, P_1)$ and $(C_2, P_2)$ are provided, the solver SHALL compute an exact similarity transformation:
  - CAD vector: $\vec{v}_C = C_2 - C_1$
  - Photo vector: $\vec{v}_P = P_2 - P_1$
  - Isotropic scale factor: $s = \|\vec{v}_C\| / \|\vec{v}_P\|$
  - Relative rotation angle: $\Delta\theta = \text{atan2}(v_{C,y}, v_{C,x}) - \text{atan2}(v_{P,y}, v_{P,x})$
  - Center translation: $t = C_1 - s \cdot R(\Delta\theta) \cdot P_1$
- The computed residual error for both points in a valid 2-point alignment SHALL be 0.0 mils (within standard floating-point precision tolerance $\epsilon < 1e-4$).

#### 3. Over-Determined 3-to-5 Point Umeyama Least-Squares Solver
- When 3, 4, or 5 correspondence pairs are provided, the solver SHALL compute the optimal 2D similarity transform using the Umeyama (least-squares Procrustes) algorithm minimizing mean squared Euclidean error.
- The algorithm SHALL:
  1. Compute centroids $\mu_C = \frac{1}{N}\sum C_i$ and $\mu_P = \frac{1}{N}\sum P_i$.
  2. Compute centered coordinates $x_i = C_i - \mu_C$ and $y_i = P_i - \mu_P$.
  3. Compute variance $\sigma_P^2 = \frac{1}{N}\sum \|y_i\|^2$.
  4. Compute 2D cross-covariance matrix $H = \frac{1}{N}\sum y_i x_i^T$.
  5. Compute optimal rotation angle $\theta = \text{atan2}(H_{01} - H_{10}, H_{00} + H_{11})$ (or via SVD with reflection disambiguation $\det(R) = +1$).
  6. Compute optimal isotropic scale $s = \frac{\text{tr}(R^T H)}{\sigma_P^2}$.
  7. Compute optimal translation vector $t = \mu_C - s \cdot R(\theta) \cdot \mu_P$.
- The solver SHALL compute individual Euclidean residuals $r_i = \|C_i - (s R P_i + t)\|$ for each pair $i \in \{1 \dots N\}$.
- The solver SHALL compute the overall Root Mean Square Error (RMSE):
  $$\text{RMSE} = \sqrt{\frac{1}{N} \sum_{i=1}^N r_i^2}$$
- Residual errors and RMSE SHALL be reported in board world mils and screen/image pixels.

#### 4. Degenerate Geometry Handling
- The solver SHALL validate inputs before computing transformations and SHALL NOT panic or crash on invalid inputs.
- The solver SHALL detect coincident points:
  - If $\|\vec{v}_C\| < \epsilon$ (where $\epsilon = 1e-4$ mils) or $\|\vec{v}_P\| < \epsilon$, the solver SHALL return a descriptive error (`CoincidentPoints`).
- The solver SHALL detect collinear points in 3-to-5 point sets when variance along a perpendicular principal axis is below numerical singularity thresholds, returning a descriptive error (`CollinearPoints`).
- When a degenerate error is returned, the UI SHALL display a non-blocking diagnostic message (e.g. "Selected pins are too close or collinear to determine orientation") and SHALL disable the "Apply Alignment" action until valid correspondences are provided.

#### 5. Interactive Calibration State Machine
- The calibration session SHALL transition through explicit states:
  - `Idle`: No active alignment session.
  - `SelectingCadPin { pair_index }`: Waiting for user to select a pin on the boardview canvas.
  - `SelectingPhotoPoint { pair_index, cad_pos, pin_name }`: Waiting for user to click corresponding position on the photo overlay canvas.
  - `ReadyToApply { pair_count, rmse }`: At least 2 pairs defined; live solution available with preview.
- Canvas interaction in calibration mode:
  - Left-click when `SelectingCadPin`: picks nearest pin within snap threshold.
  - Left-click when `SelectingPhotoPoint`: sets photo mark for current pair and advances to next pair or ready state.
  - Escape key or "Cancel": cancels the session without modifying the original overlay slot transformation.
  - "Reset": clears all picked pin pairs and restarts calibration at pair 1.
  - "Apply": commits the calculated transformation to the overlay slot and exits calibration mode.

---

### Scenarios

#### Scenario 1: Exact 2-Point Similarity Calculation
- **GIVEN** a boardview with an unlocked photo overlay slot loaded on Top side
- **AND** an active calibration session in state `SelectingCadPin { pair_index: 0 }`
- **WHEN** the user selects CAD pin `U1.1` at $(100.0, 200.0)$ mils and marks its photo position at $(120.0, 180.0)$ mils
- **AND** the user selects CAD pin `U1.10` at $(400.0, 600.0)$ mils and marks its photo position at $(420.0, 580.0)$ mils
- **THEN** the solver computes an exact 2D similarity transform matching both points
- **AND** the computed residual error for both pairs is $0.0 \pm 1e-4$ mils
- **AND** the session transitions to `ReadyToApply` displaying $\text{RMSE} = 0.0$ mils.

#### Scenario 2: Over-Determined 3-to-5 Point Least-Squares Solve with RMSE
- **GIVEN** an active calibration session with 2 valid pin correspondence pairs already registered
- **WHEN** the user adds a 3rd pin pair (e.g., `R5.1`), a 4th pin pair (`C12.2`), and a 5th pin pair (`Q2.3`) spread across the PCB
- **THEN** the solver calculates the optimal similarity transform using the Umeyama algorithm
- **AND** the UI displays the overall RMSE in mils and pixels
- **AND** each correspondence row in the side panel displays its individual residual error $r_i$
- **AND** pins with residuals exceeding 1.5× the average error are visually highlighted to indicate potential misclicks.

#### Scenario 3: Attempting to Add a 6th Pin Pair
- **GIVEN** an active calibration session with 5 registered pin correspondence pairs
- **WHEN** the user attempts to add an additional correspondence pair
- **THEN** the application prevents adding a 6th pair
- **AND** displays an informative notice that maximum calibration points (5) have been reached.

#### Scenario 4: Degenerate Coincident Pin Selection
- **GIVEN** an active calibration session with 1 pin pair already picked
- **WHEN** the user picks a second CAD pin that has identical coordinates to the first pin ($C_2 == C_1$)
- **OR** marks the photo position at the exact same location ($P_2 == P_1$)
- **THEN** the solver identifies the coincident point condition
- **AND** returns a `DegeneratePoints("Reference points must be distinct")` error without panicking
- **AND** the UI displays an error warning and disables the "Apply" button.

#### Scenario 5: Degenerate Collinear 3-Point Selection
- **GIVEN** an active calibration session with 3 correspondence pairs
- **WHEN** all 3 CAD pins or photo points lie on a straight line such that the 2D cross-covariance matrix is rank-deficient
- **THEN** the solver safely detects the degeneracy
- **AND** reports a collinearity warning advising the user to pick a non-collinear pin
- **AND** falls back to 2-point alignment using the extreme endpoints or prevents applying until a non-collinear pin is selected.

#### Scenario 6: Interactive State Progression and Pin Snap
- **GIVEN** an active calibration session in `SelectingCadPin { pair_index: 1 }`
- **WHEN** the user hovers over the boardview canvas
- **THEN** the nearest pin within snap radius is highlighted with a calibration target ring
- **WHEN** the user clicks on the highlighted pin
- **THEN** the CAD coordinate is stored and the session transitions to `SelectingPhotoPoint { pair_index: 1 }`
- **AND** the canvas displays a prompt instructing the user to mark the corresponding physical pad on the photo.

#### Scenario 7: Cancellation and Session Reset
- **GIVEN** an active calibration session with 3 registered correspondence pairs and modified preview transform
- **WHEN** the user clicks "Cancel" or presses `Escape`
- **THEN** the calibration session terminates
- **AND** the overlay slot transformation reverts completely to its pre-calibration state
- **AND** all temporary calibration markers and vectors are cleared from the canvas.
