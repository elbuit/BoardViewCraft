# Tasks: PCB Image Pin Alignment

## Phase 1.0: Math & Solver Implementation (`crates/bvc-app/src/overlay_align.rs`)
- [x] 1.1 Define error enum `SolverError` and core data structures (`AlignmentPointPair`, `AlignmentResult`, `OverlaySnapshot`)
- [x] 1.2 Implement input validation and degeneracy checks (minimum 2 pairs, maximum 5 pairs, coincident point detection, collinearity checks)
- [x] 1.3 Implement analytical 2-point similarity transform solver (isotropic scale $s$, rotation angle $\theta$, translation $t$)
- [x] 1.4 Implement 3-to-5-point Umeyama least-squares similarity transform solver using centered cross-covariance matrix and analytic 2D rotation
- [x] 1.5 Implement residual vector computation ($r_i$) and overall Root Mean Square Error (RMSE) calculation in mils
- [x] 1.6 Unit tests for 2-point exact fits, 3-point orthogonal configurations, 4-point rectangles, 5-point random configurations, and numerical edge cases

## Phase 2.0: Session State Machine & Coordinate Conversions
- [x] 2.1 Define `AlignmentState` enum (`Idle`, `SelectingCadPin`, `SelectingPhotoPoint`, `ReadyToApply`)
- [x] 2.2 Create `AlignmentSession` struct managing pairs, state transitions, live solver result cache, and pre-calibration snapshot rollback
- [x] 2.3 Implement coordinate transformation helpers on `OverlaySlot` converting between screen space, board world mils, and overlay image quad
- [x] 2.4 Implement application of `AlignmentResult` to update `OverlaySlot.center`, `OverlaySlot.set_zoom`, and `OverlaySlot.fine_rotation_deg` with optional auto-locking

## Phase 3.0: UI & Canvas Visual Overlays
- [x] 3.1 Implement canvas click interceptor for pin snapping in `SelectingCadPin` and photo mark placement in `SelectingPhotoPoint`
- [x] 3.2 Render numbered visual calibration markers for CAD pins (cyan crosshairs) and photo positions (magenta diamonds)
- [x] 3.3 Render residual error vectors connecting CAD pins to projected photo locations with green/yellow/red color thresholds
- [x] 3.4 Render top/bottom floating calibration status guide banner instructing user on current step
- [x] 3.5 Implement `Tab::Overlay` calibration controls in `side_panel.rs`: "Align with Pins" launcher, pin correspondence table, per-pair residuals, RMSE badge, re-pick/delete buttons, auto-lock toggle, and Apply/Cancel actions
- [x] 3.6 Handle keyboard shortcuts (`Escape` to cancel and revert, `Enter` to commit when ready)

## Phase 4.0: Localization, Testing & Verification
- [x] 4.1 Add all UI strings, prompts, and error notifications to `crates/bvc-app/src/i18n.rs` for English, Spanish, and Catalan
- [x] 4.2 Integration tests for calibration state machine lifecycle (start session, add pairs, delete pair, preview, commit, cancel)
- [x] 4.3 Verify zero unwrap/panic rules (`cargo clippy --all-targets`) and formatting (`cargo fmt --check`)
- [x] 4.4 End-to-end regression testing with loaded sample boardviews and photo overlays

Path: `openspec/changes/pcb-image-pin-alignment/tasks.md`
