//! PCB Image Pin Alignment module.
//!
//! Provides analytical 2-point similarity transform and 3-to-5-point Umeyama
//! least-squares similarity transform solver, error reporting, residuals, RMSE,
//! and interactive calibration session state management.

use bvc_core::Point;
use egui::Vec2;
use thiserror::Error;

/// Numerical epsilon for distance and denominator degeneracy checks (in mils).
pub const EPSILON: f64 = 1e-4;

/// Maximum number of pin pairs supported in calibration session.
pub const MAX_PAIRS: usize = 5;

/// Error types occurring during calibration solving.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum SolverError {
    #[error("At least 2 correspondence pairs are required to compute alignment (provided: {0})")]
    InsufficientPoints(usize),

    #[error("Maximum 5 correspondence pairs supported (provided: {0})")]
    ExcessPoints(usize),

    #[error("Reference points must be distinct; coincident points detected")]
    CoincidentPoints,

    #[error("Selected reference points are collinear; pick a point not along the line")]
    CollinearPoints,

    #[error("Numerical singularity encountered during transform calculation")]
    NumericalSingularity,
}

/// A correspondence between a boardview CAD pin and a photo coordinate.
#[derive(Debug, Clone, PartialEq)]
pub struct AlignmentPointPair {
    /// Index into Board.pins.
    pub pin_id: usize,
    /// Pin label (e.g. "1", "A1", "GND").
    pub pin_name: String,
    /// Part designator (e.g. "U1", "R10").
    pub part_name: String,
    /// CAD position in board world mils.
    pub cad_pos: Point,
    /// Photo position in board world mils (untransformed slot frame).
    pub photo_pos: Option<Point>,
}

/// Output of a successful similarity transformation solve.
#[derive(Debug, Clone, PartialEq)]
pub struct AlignmentResult {
    /// Optimal overlay slot center in board world mils.
    pub center: Point,
    /// Optimal isotropic zoom multiplier for OverlaySlot.
    pub zoom: f32,
    /// Optimal fine rotation angle in degrees [-180.0, 180.0].
    pub rotation_deg: f32,
    /// Root Mean Square Error in board world mils.
    pub rmse_mils: f32,
    /// Per-point Euclidean residuals in board world mils.
    pub residuals_mils: Vec<f32>,
}

/// Interactive state of the alignment session.
#[derive(Debug, Clone, PartialEq)]
pub enum AlignmentState {
    /// Idle, no alignment session active.
    Idle,
    /// Waiting for user to click a pin on the CAD layout.
    SelectingCadPin { pair_index: usize },
    /// Waiting for user to click corresponding pad on the photo overlay.
    SelectingPhotoPoint {
        pair_index: usize,
        pin_id: usize,
        part_name: String,
        pin_name: String,
        cad_pos: Point,
    },
    /// Alignment calculated and ready to preview or commit.
    ReadyToApply,
}

/// Snapshot of pre-calibration overlay parameters for safe rollback.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OverlaySnapshot {
    pub center: Point,
    pub zoom: f32,
    pub size: Vec2,
    pub fine_rotation_deg: f32,
    pub locked: bool,
}

/// Active alignment calibration session.
#[derive(Debug, Clone)]
pub struct AlignmentSession {
    pub state: AlignmentState,
    pub pairs: Vec<AlignmentPointPair>,
    pub live_result: Option<Result<AlignmentResult, SolverError>>,
    pub snapshot: Option<OverlaySnapshot>,
    pub auto_lock_on_apply: bool,
    pub hovered_candidate_pin: Option<usize>,
}

impl Default for AlignmentSession {
    fn default() -> Self {
        Self {
            state: AlignmentState::Idle,
            pairs: Vec::new(),
            live_result: None,
            snapshot: None,
            auto_lock_on_apply: true,
            hovered_candidate_pin: None,
        }
    }
}

/// Internal similarity transformation parameters mapping photo points P -> CAD points C.
/// C = s * R(theta) * P + t
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SimilarityTransform {
    pub scale: f64,
    pub rotation_rad: f64,
    pub translation: Point,
}

impl SimilarityTransform {
    /// Transforms a point from photo space to CAD space.
    #[inline]
    pub fn transform_point(&self, p: Point) -> Point {
        let (sin_t, cos_t) = self.rotation_rad.sin_cos();
        let rx = self.scale * (cos_t * p.x - sin_t * p.y);
        let ry = self.scale * (sin_t * p.x + cos_t * p.y);
        Point::new(rx + self.translation.x, ry + self.translation.y)
    }
}

/// Validates input pairs: checks count (2..=5) and coincident points.
fn validate_pairs(pairs: &[AlignmentPointPair]) -> Result<Vec<(Point, Point)>, SolverError> {
    let complete_pairs: Vec<(Point, Point)> = pairs
        .iter()
        .filter_map(|p| p.photo_pos.map(|photo| (p.cad_pos, photo)))
        .collect();

    let n = complete_pairs.len();
    if n < 2 {
        return Err(SolverError::InsufficientPoints(n));
    }
    if n > MAX_PAIRS {
        return Err(SolverError::ExcessPoints(n));
    }

    // Check for coincident CAD points or coincident Photo points
    for i in 0..n {
        for j in (i + 1)..n {
            let (c1, p1) = complete_pairs[i];
            let (c2, p2) = complete_pairs[j];

            let cad_dist = ((c1.x - c2.x).powi(2) + (c1.y - c2.y).powi(2)).sqrt();
            let photo_dist = ((p1.x - p2.x).powi(2) + (p1.y - p2.y).powi(2)).sqrt();

            if cad_dist < EPSILON || photo_dist < EPSILON {
                return Err(SolverError::CoincidentPoints);
            }
        }
    }

    Ok(complete_pairs)
}

/// Computes closed-form 2-point similarity transform.
pub fn solve_2point(cad: [(f64, f64); 2], photo: [(f64, f64); 2]) -> Result<SimilarityTransform, SolverError> {
    let (c1x, c1y) = cad[0];
    let (c2x, c2y) = cad[1];
    let (p1x, p1y) = photo[0];
    let (p2x, p2y) = photo[1];

    let v_cx = c2x - c1x;
    let v_cy = c2y - c1y;
    let v_px = p2x - p1x;
    let v_py = p2y - p1y;

    let d_c = (v_cx * v_cx + v_cy * v_cy).sqrt();
    let d_p = (v_px * v_px + v_py * v_py).sqrt();

    if d_c < EPSILON || d_p < EPSILON {
        return Err(SolverError::CoincidentPoints);
    }

    let scale = d_c / d_p;
    if !scale.is_finite() || scale <= 0.0 {
        return Err(SolverError::NumericalSingularity);
    }

    let theta_c = v_cy.atan2(v_cx);
    let theta_p = v_py.atan2(v_px);
    let mut theta = theta_c - theta_p;

    // Normalize to [-PI, PI]
    while theta > std::f64::consts::PI {
        theta -= 2.0 * std::f64::consts::PI;
    }
    while theta < -std::f64::consts::PI {
        theta += 2.0 * std::f64::consts::PI;
    }

    let (sin_t, cos_t) = theta.sin_cos();
    let tx = c1x - scale * (cos_t * p1x - sin_t * p1y);
    let ty = c1y - scale * (sin_t * p1x + cos_t * p1y);

    Ok(SimilarityTransform {
        scale,
        rotation_rad: theta,
        translation: Point::new(tx, ty),
    })
}

/// Checks whether a set of 2D points are collinear (all lying on the same line).
pub fn are_points_collinear(points: &[(f64, f64)]) -> bool {
    if points.len() <= 2 {
        return false;
    }
    let (p0x, p0y) = points[0];
    let (p1x, p1y) = points[1];
    let dx1 = p1x - p0x;
    let dy1 = p1y - p0y;

    for i in 2..points.len() {
        let (px, py) = points[i];
        let dx2 = px - p0x;
        let dy2 = py - p0y;
        // 2D cross product: dx1 * dy2 - dy1 * dx2
        let cross = dx1 * dy2 - dy1 * dx2;
        let norm_sq = (dx1 * dx1 + dy1 * dy1).max(dx2 * dx2 + dy2 * dy2);
        if norm_sq > EPSILON && (cross * cross / norm_sq) > EPSILON {
            return false;
        }
    }
    true
}

/// Computes 3-to-5 point Umeyama least-squares similarity transform.
pub fn solve_umeyama(cad: &[(f64, f64)], photo: &[(f64, f64)]) -> Result<SimilarityTransform, SolverError> {
    let n = cad.len();
    if n < 3 {
        return Err(SolverError::InsufficientPoints(n));
    }
    if n > MAX_PAIRS {
        return Err(SolverError::ExcessPoints(n));
    }
    if n != photo.len() {
        return Err(SolverError::NumericalSingularity);
    }

    if are_points_collinear(cad) || are_points_collinear(photo) {
        return Err(SolverError::CollinearPoints);
    }

    let n_f = n as f64;

    // 1. Centroids
    let (mut mu_cx, mut mu_cy) = (0.0, 0.0);
    let (mut mu_px, mut mu_py) = (0.0, 0.0);
    for i in 0..n {
        mu_cx += cad[i].0;
        mu_cy += cad[i].1;
        mu_px += photo[i].0;
        mu_py += photo[i].1;
    }
    mu_cx /= n_f;
    mu_cy /= n_f;
    mu_px /= n_f;
    mu_py /= n_f;

    // 2. Centered coordinates and variance of source points (P)
    let mut sigma_p_sq = 0.0;
    let mut sigma_c_sq = 0.0;
    let mut h00 = 0.0;
    let mut h01 = 0.0;
    let mut h10 = 0.0;
    let mut h11 = 0.0;

    for i in 0..n {
        let x_i = cad[i].0 - mu_cx;
        let y_i = cad[i].1 - mu_cy;
        let u_i = photo[i].0 - mu_px;
        let v_i = photo[i].1 - mu_py;

        sigma_p_sq += u_i * u_i + v_i * v_i;
        sigma_c_sq += x_i * x_i + y_i * y_i;

        // Cross-covariance H = (1/N) * sum(P_centered * C_centered^T)
        // H = [ [u*x, u*y], [v*x, v*y] ]
        h00 += u_i * x_i;
        h01 += u_i * y_i;
        h10 += v_i * x_i;
        h11 += v_i * y_i;
    }

    sigma_p_sq /= n_f;
    sigma_c_sq /= n_f;

    if sigma_p_sq < EPSILON || sigma_c_sq < EPSILON {
        return Err(SolverError::CoincidentPoints);
    }

    h00 /= n_f;
    h01 /= n_f;
    h10 /= n_f;
    h11 /= n_f;

    // 3. Optimal 2D rotation: theta = atan2(H01 - H10, H00 + H11)
    let num = h01 - h10;
    let den = h00 + h11;

    if num.abs() < EPSILON && den.abs() < EPSILON {
        return Err(SolverError::CollinearPoints);
    }

    let theta = num.atan2(den);
    let (sin_t, cos_t) = theta.sin_cos();

    // 4. Optimal isotropic scale: s = tr(R^T H) / sigma_p_sq
    // tr(R^T H) = cos(theta)*(H00 + H11) + sin(theta)*(H01 - H10)
    let tr_rt_h = cos_t * den + sin_t * num;
    let scale = tr_rt_h / sigma_p_sq;

    if !scale.is_finite() || scale <= 0.0 {
        return Err(SolverError::NumericalSingularity);
    }

    // 5. Optimal translation: t = mu_C - s * R(theta) * mu_P
    let tx = mu_cx - scale * (cos_t * mu_px - sin_t * mu_py);
    let ty = mu_cy - scale * (sin_t * mu_px + cos_t * mu_py);

    Ok(SimilarityTransform {
        scale,
        rotation_rad: theta,
        translation: Point::new(tx, ty),
    })
}

/// Solves alignment for correspondence pairs and computes per-point residuals and RMSE.
pub fn solve_alignment(
    pairs: &[AlignmentPointPair],
    _initial_center: Point,
    _initial_zoom: f32,
    _initial_rotation_deg: f32,
) -> Result<AlignmentResult, SolverError> {
    let valid_pairs = validate_pairs(pairs)?;
    let n = valid_pairs.len();

    let transform = if n == 2 {
        let cad = [(valid_pairs[0].0.x, valid_pairs[0].0.y), (valid_pairs[1].0.x, valid_pairs[1].0.y)];
        let photo = [(valid_pairs[0].1.x, valid_pairs[0].1.y), (valid_pairs[1].1.x, valid_pairs[1].1.y)];
        solve_2point(cad, photo)?
    } else {
        let cad: Vec<(f64, f64)> = valid_pairs.iter().map(|(c, _)| (c.x, c.y)).collect();
        let photo: Vec<(f64, f64)> = valid_pairs.iter().map(|(_, p)| (p.x, p.y)).collect();
        solve_umeyama(&cad, &photo)?
    };

    // Calculate residuals and RMSE
    let mut sum_sq_residuals = 0.0;
    let mut residuals_mils = Vec::with_capacity(n);

    for (c, p) in &valid_pairs {
        let c_hat = transform.transform_point(*p);
        let dx = c.x - c_hat.x;
        let dy = c.y - c_hat.y;
        let res = (dx * dx + dy * dy).sqrt();
        residuals_mils.push(res as f32);
        sum_sq_residuals += res * res;
    }

    let rmse_mils = (sum_sq_residuals / n as f64).sqrt() as f32;

    // Direct mapping to OverlaySlot parameters:
    // When photo points are stored in invariant photo local coordinates (unscaled mils relative to photo center),
    // the similarity transform maps photo_local -> cad_pos directly:
    // cad_pos = transform.scale * R(transform.rotation_rad) * photo_local + transform.translation
    // Therefore:
    // new_center = transform.translation
    // new_zoom = transform.scale as f32
    // new_rot = transform.rotation_rad.to_degrees() as f32
    let new_center = transform.translation;
    let new_zoom = transform.scale as f32;

    let mut new_rot = transform.rotation_rad.to_degrees() as f32;
    while new_rot > 180.0 {
        new_rot -= 360.0;
    }
    while new_rot < -180.0 {
        new_rot += 360.0;
    }

    Ok(AlignmentResult {
        center: new_center,
        zoom: new_zoom,
        rotation_deg: new_rot,
        rmse_mils,
        residuals_mils,
    })
}

impl AlignmentSession {
    /// Starts a new calibration session, capturing current overlay snapshot.
    pub fn start(&mut self, snapshot: OverlaySnapshot) {
        self.snapshot = Some(snapshot);
        self.pairs.clear();
        self.live_result = None;
        self.state = AlignmentState::SelectingCadPin { pair_index: 0 };
        self.hovered_candidate_pin = None;
    }

    /// Cancels session and returns the snapshot to be restored.
    pub fn cancel(&mut self) -> Option<OverlaySnapshot> {
        let snapshot = self.snapshot.take();
        self.state = AlignmentState::Idle;
        self.pairs.clear();
        self.live_result = None;
        self.hovered_candidate_pin = None;
        snapshot
    }

    /// User selected a CAD pin.
    pub fn select_cad_pin(&mut self, pin_id: usize, pin_name: String, part_name: String, cad_pos: Point) {
        let pair_index = match self.state {
            AlignmentState::SelectingCadPin { pair_index } => pair_index,
            _ => self.pairs.len(),
        };

        if pair_index < self.pairs.len() {
            // Re-picking existing pair
            self.pairs[pair_index].pin_id = pin_id;
            self.pairs[pair_index].pin_name = pin_name.clone();
            self.pairs[pair_index].part_name = part_name.clone();
            self.pairs[pair_index].cad_pos = cad_pos;
        } else {
            // Appending new pair with unassigned photo pos
            self.pairs.push(AlignmentPointPair {
                pin_id,
                pin_name: pin_name.clone(),
                part_name: part_name.clone(),
                cad_pos,
                photo_pos: None,
            });
        }

        self.state = AlignmentState::SelectingPhotoPoint {
            pair_index,
            pin_id,
            part_name,
            pin_name,
            cad_pos,
        };
    }

    /// User placed photo point for the currently selected pair.
    pub fn select_photo_point(&mut self, photo_pos: Point) {
        if let AlignmentState::SelectingPhotoPoint { pair_index, .. } = self.state {
            if let Some(pair) = self.pairs.get_mut(pair_index) {
                pair.photo_pos = Some(photo_pos);
            }

            self.recalculate();

            let complete_count = self.pairs.iter().filter(|p| p.photo_pos.is_some()).count();
            if complete_count < 2 {
                // If fewer than 2 pairs, automatically prompt for the next pair
                let next_idx = self.pairs.len();
                if next_idx < MAX_PAIRS {
                    self.state = AlignmentState::SelectingCadPin { pair_index: next_idx };
                } else {
                    self.state = AlignmentState::ReadyToApply;
                }
            } else {
                // Once at least 2 pairs are complete, the solver has run and user is ready to apply or add more
                self.state = AlignmentState::ReadyToApply;
            }
        }
    }

    /// Adds another pair slot if pairs count < MAX_PAIRS.
    pub fn add_pair(&mut self) {
        if self.pairs.len() < MAX_PAIRS {
            let next_idx = self.pairs.len();
            self.state = AlignmentState::SelectingCadPin { pair_index: next_idx };
        }
    }

    /// Re-picks an existing pair.
    pub fn repick_pair(&mut self, pair_index: usize) {
        if pair_index < self.pairs.len() {
            self.state = AlignmentState::SelectingCadPin { pair_index };
        }
    }

    /// Deletes a pair at index.
    pub fn delete_pair(&mut self, pair_index: usize) {
        if pair_index < self.pairs.len() {
            self.pairs.remove(pair_index);
            self.recalculate();
            if self.pairs.iter().all(|p| p.photo_pos.is_some()) && self.pairs.len() >= 2 {
                self.state = AlignmentState::ReadyToApply;
            } else if let Some(incomplete) = self.pairs.iter().position(|p| p.photo_pos.is_none()) {
                self.state = AlignmentState::SelectingCadPin { pair_index: incomplete };
            } else {
                self.state = AlignmentState::SelectingCadPin { pair_index: self.pairs.len() };
            }
        }
    }

    /// Re-runs the solver if a valid snapshot is present.
    pub fn recalculate(&mut self) {
        if let Some(snapshot) = self.snapshot {
            let complete_count = self.pairs.iter().filter(|p| p.photo_pos.is_some()).count();
            if complete_count >= 2 {
                let res = solve_alignment(&self.pairs, snapshot.center, snapshot.zoom, snapshot.fine_rotation_deg);
                self.live_result = Some(res);
            } else {
                self.live_result = None;
            }
        }
    }

    /// Returns true if session is currently active.
    pub fn is_active(&self) -> bool {
        self.state != AlignmentState::Idle
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_solve_2point_pure_translation() {
        let cad = [(100.0, 100.0), (200.0, 100.0)];
        let photo = [(0.0, 0.0), (100.0, 0.0)];

        let transform = solve_2point(cad, photo).expect("solve 2point translation failed");
        assert!((transform.scale - 1.0).abs() < 1e-6);
        assert!(transform.rotation_rad.abs() < 1e-6);
        assert!((transform.translation.x - 100.0).abs() < 1e-6);
        assert!((transform.translation.y - 100.0).abs() < 1e-6);

        let p_transformed = transform.transform_point(Point::new(0.0, 0.0));
        assert!((p_transformed.x - 100.0).abs() < 1e-6);
        assert!((p_transformed.y - 100.0).abs() < 1e-6);
    }

    #[test]
    fn test_solve_2point_rotation_and_scaling() {
        // Photo: (0, 0) and (10, 0) (length 10)
        // CAD: (50, 50) and (50, 70) (length 20, rotated 90 deg CCW)
        let cad = [(50.0, 50.0), (50.0, 70.0)];
        let photo = [(0.0, 0.0), (10.0, 0.0)];

        let transform = solve_2point(cad, photo).expect("solve 2point rotation and scale failed");
        assert!((transform.scale - 2.0).abs() < 1e-6);
        assert!((transform.rotation_rad - std::f64::consts::FRAC_PI_2).abs() < 1e-6);

        let p1_eval = transform.transform_point(Point::new(0.0, 0.0));
        assert!((p1_eval.x - 50.0).abs() < 1e-6);
        assert!((p1_eval.y - 50.0).abs() < 1e-6);

        let p2_eval = transform.transform_point(Point::new(10.0, 0.0));
        assert!((p2_eval.x - 50.0).abs() < 1e-6);
        assert!((p2_eval.y - 70.0).abs() < 1e-6);
    }

    #[test]
    fn test_solve_2point_coincident_points() {
        let cad = [(10.0, 10.0), (10.0, 10.0)];
        let photo = [(0.0, 0.0), (10.0, 0.0)];
        assert_eq!(solve_2point(cad, photo), Err(SolverError::CoincidentPoints));
    }

    #[test]
    fn test_solve_umeyama_3point_orthogonal() {
        // Source points: (0,0), (10,0), (0,10)
        // Target points: scaled by 1.5, rotated 180 deg (negated), translated (+100, +200)
        let scale = 1.5;
        let theta = std::f64::consts::PI;
        let tx = 100.0;
        let ty = 200.0;

        let src = [(0.0, 0.0), (10.0, 0.0), (0.0, 10.0)];
        let dst: Vec<(f64, f64)> = src
            .iter()
            .map(|&(x, y)| {
                let rx = scale * (theta.cos() * x - theta.sin() * y) + tx;
                let ry = scale * (theta.sin() * x + theta.cos() * y) + ty;
                (rx, ry)
            })
            .collect();

        let transform = solve_umeyama(&dst, &src).expect("Umeyama 3-point failed");
        assert!((transform.scale - scale).abs() < 1e-5);
        assert!((transform.rotation_rad.abs() - std::f64::consts::PI).abs() < 1e-5);
        assert!((transform.translation.x - tx).abs() < 1e-5);
        assert!((transform.translation.y - ty).abs() < 1e-5);
    }

    #[test]
    fn test_solve_umeyama_4point_rectangle() {
        let scale: f64 = 0.8;
        let theta: f64 = 0.45; // rad
        let tx: f64 = -30.0;
        let ty: f64 = 45.0;

        let src = [(0.0, 0.0), (50.0, 0.0), (50.0, 20.0), (0.0, 20.0)];
        let dst: Vec<(f64, f64)> = src
            .iter()
            .map(|&(x, y)| {
                let rx = scale * (theta.cos() * x - theta.sin() * y) + tx;
                let ry = scale * (theta.sin() * x + theta.cos() * y) + ty;
                (rx, ry)
            })
            .collect();

        let transform = solve_umeyama(&dst, &src).expect("Umeyama 4-point rectangle failed");
        assert!((transform.scale - scale).abs() < 1e-5);
        assert!((transform.rotation_rad - theta).abs() < 1e-5);
        assert!((transform.translation.x - tx).abs() < 1e-5);
        assert!((transform.translation.y - ty).abs() < 1e-5);
    }

    #[test]
    fn test_solve_umeyama_5point_configuration() {
        let src = [
            (10.0, 15.0),
            (80.0, 20.0),
            (90.0, 75.0),
            (15.0, 85.0),
            (45.0, 50.0),
        ];
        let scale: f64 = 2.5;
        let theta: f64 = -0.75;
        let tx: f64 = 250.0;
        let ty: f64 = -120.0;

        let dst: Vec<(f64, f64)> = src
            .iter()
            .map(|&(x, y)| {
                let rx = scale * (theta.cos() * x - theta.sin() * y) + tx;
                let ry = scale * (theta.sin() * x + theta.cos() * y) + ty;
                (rx, ry)
            })
            .collect();

        let transform = solve_umeyama(&dst, &src).expect("Umeyama 5-point failed");
        assert!((transform.scale - scale).abs() < 1e-5);
        assert!((transform.rotation_rad - theta).abs() < 1e-5);
        assert!((transform.translation.x - tx).abs() < 1e-5);
        assert!((transform.translation.y - ty).abs() < 1e-5);
    }

    #[test]
    fn test_solve_umeyama_collinear_rejection() {
        let src = [(0.0, 0.0), (10.0, 0.0), (20.0, 0.0)];
        let dst = [(0.0, 0.0), (10.0, 0.0), (20.0, 0.0)];
        let err = solve_umeyama(&dst, &src).unwrap_err();
        assert_eq!(err, SolverError::CollinearPoints);
    }

    #[test]
    fn test_alignment_residuals_and_rmse() {
        let pairs = vec![
            AlignmentPointPair {
                pin_id: 1,
                pin_name: "1".into(),
                part_name: "U1".into(),
                cad_pos: Point::new(10.0, 10.0),
                photo_pos: Some(Point::new(0.0, 0.0)),
            },
            AlignmentPointPair {
                pin_id: 2,
                pin_name: "2".into(),
                part_name: "U1".into(),
                cad_pos: Point::new(20.0, 10.0),
                photo_pos: Some(Point::new(10.0, 0.0)),
            },
        ];

        let res = solve_alignment(&pairs, Point::new(0.0, 0.0), 1.0, 0.0).expect("Solve alignment failed");
        assert!(res.rmse_mils < 1e-4);
        assert_eq!(res.residuals_mils.len(), 2);
        assert!(res.residuals_mils[0] < 1e-4);
        assert!(res.residuals_mils[1] < 1e-4);
        assert!((res.zoom - 1.0).abs() < 1e-4);
        assert!((res.center.x - 10.0).abs() < 1e-4);
        assert!((res.center.y - 10.0).abs() < 1e-4);
    }

    #[test]
    fn test_session_state_machine_lifecycle() {
        let mut session = AlignmentSession::default();
        let snapshot = OverlaySnapshot {
            center: Point::new(100.0, 100.0),
            zoom: 1.0,
            size: Vec2::new(500.0, 500.0),
            fine_rotation_deg: 0.0,
            locked: false,
        };

        session.start(snapshot);
        assert!(session.is_active());
        assert_eq!(session.state, AlignmentState::SelectingCadPin { pair_index: 0 });

        // Select pin 1
        session.select_cad_pin(1, "1".into(), "U1".into(), Point::new(10.0, 10.0));
        assert!(matches!(session.state, AlignmentState::SelectingPhotoPoint { pair_index: 0, .. }));

        // Pick photo point 1
        session.select_photo_point(Point::new(0.0, 0.0));
        assert_eq!(session.state, AlignmentState::SelectingCadPin { pair_index: 1 });

        // Select pin 2
        session.select_cad_pin(2, "2".into(), "U1".into(), Point::new(20.0, 10.0));
        session.select_photo_point(Point::new(10.0, 0.0));

        // 2 points complete -> ReadyToApply
        assert_eq!(session.state, AlignmentState::ReadyToApply);
        assert!(session.live_result.is_some());

        // Cancel
        let restored = session.cancel();
        assert_eq!(restored, Some(snapshot));
        assert_eq!(session.state, AlignmentState::Idle);
        assert!(!session.is_active());
    }
}
