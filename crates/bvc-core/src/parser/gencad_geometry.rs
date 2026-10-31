//! Geometry helpers for GenCAD pads, shape artwork and placements.
//!
//! Portions adapted from TRACE Boardviewer (MIT), Copyright (c) 2026 TRACE Boardviewer contributors
//! (`src/lib/gencad.ts`: `transform`, `geometry`, `rectangle`, `outerContour`).
//!
//! Coordinates are in board units (mils) after the GenCAD unit conversion, Y upwards, as in the file.

use std::collections::{HashMap, HashSet};
use std::f64::consts::{PI, TAU};

use crate::model::{Point, Rect};

/// GenCAD mirror flag (`0`, `MIRRORX`, `MIRRORY`, `MIRRORXY`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Mirror {
    #[default]
    None,
    /// Reflection about the X axis (Y changes sign).
    X,
    /// Reflection about the Y axis (X changes sign).
    Y,
    /// Both reflections.
    XY,
}

impl Mirror {
    /// Parses a mirror token; anything unknown (including `0`) is no mirror.
    pub fn parse(token: Option<&str>) -> Self {
        match token.map(str::to_ascii_uppercase).as_deref() {
            Some("MIRRORX") => Mirror::X,
            Some("MIRRORY") => Mirror::Y,
            Some("MIRRORXY") => Mirror::XY,
            _ => Mirror::None,
        }
    }

    /// Mirror from independent X / Y flags.
    pub fn from_flags(mirror_x: bool, mirror_y: bool) -> Self {
        match (mirror_x, mirror_y) {
            (false, false) => Mirror::None,
            (true, false) => Mirror::X,
            (false, true) => Mirror::Y,
            (true, true) => Mirror::XY,
        }
    }
}

/// Mirrors `position` (before rotation), rotates it counter-clockwise by `rotation_deg` and adds `offset`.
pub fn transform(position: Point, rotation_deg: f64, mirror: Mirror, offset: Point) -> Point {
    let x = position.x * if matches!(mirror, Mirror::Y | Mirror::XY) { -1.0 } else { 1.0 };
    let y = position.y * if matches!(mirror, Mirror::X | Mirror::XY) { -1.0 } else { 1.0 };
    let angle = rotation_deg * PI / 180.0;
    let (sin, cos) = angle.sin_cos();
    Point::new(offset.x + x * cos - y * sin, offset.y + x * sin + y * cos)
}

/// Kind of GenCAD geometry record a path came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathKind {
    Line,
    Arc,
    Circle,
    Rectangle,
}

/// Polyline built from one GenCAD geometry record.
#[derive(Debug, Clone, PartialEq)]
pub struct GeomPath {
    pub points: Vec<Point>,
    pub kind: PathKind,
}

/// Closed rectangle (five points, first repeated last), counter-clockwise from the lower-left corner.
pub fn rectangle(bounds: &Rect) -> Vec<Point> {
    vec![
        Point::new(bounds.min_x, bounds.min_y),
        Point::new(bounds.max_x, bounds.min_y),
        Point::new(bounds.max_x, bounds.max_y),
        Point::new(bounds.min_x, bounds.max_y),
        Point::new(bounds.min_x, bounds.min_y),
    ]
}

/// Parses a `LINE`, `ARC`, `CIRCLE` or `RECTANGLE` record (`tokens[0]` is the command).
/// `to_board` converts one file length into board units. Malformed records yield `None`.
pub fn parse_geometry(tokens: &[&str], to_board: impl Fn(f64) -> f64) -> Option<GeomPath> {
    let command = tokens.first()?.to_ascii_uppercase();
    let num = |i: usize| -> Option<f64> {
        let v = tokens.get(i)?.parse::<f64>().ok()?;
        v.is_finite().then(|| to_board(v))
    };
    let point = |i: usize| -> Option<Point> { Some(Point::new(num(i)?, num(i + 1)?)) };
    match command.as_str() {
        "LINE" => Some(GeomPath { points: vec![point(1)?, point(3)?], kind: PathKind::Line }),
        "RECTANGLE" => {
            // GenCAD rectangles store a starting corner and a size, not two corners.
            let start = point(1)?;
            let (w, h) = (num(3)?, num(4)?);
            let bounds = Rect {
                min_x: start.x.min(start.x + w),
                min_y: start.y.min(start.y + h),
                max_x: start.x.max(start.x + w),
                max_y: start.y.max(start.y + h),
            };
            Some(GeomPath { points: rectangle(&bounds), kind: PathKind::Rectangle })
        }
        "CIRCLE" => {
            let center = point(1)?;
            let radius = num(3)?;
            if radius < 0.0 {
                return None;
            }
            let points = (0..=64)
                .map(|i| {
                    let a = i as f64 * TAU / 64.0;
                    Point::new(center.x + radius * a.cos(), center.y + radius * a.sin())
                })
                .collect();
            Some(GeomPath { points, kind: PathKind::Circle })
        }
        "ARC" => {
            let (start, end, center) = (point(1)?, point(3)?, point(5)?);
            let radius = start.dist(center);
            let end_radius = end.dist(center);
            if radius == 0.0 || (radius - end_radius).abs() > (0.01f64).max(radius * 0.001) {
                return None;
            }
            let start_angle = (start.y - center.y).atan2(start.x - center.x);
            let end_angle = (end.y - center.y).atan2(end.x - center.x);
            // Arcs are counter-clockwise in the file's Y-up coordinate system.
            let mut sweep = (end_angle - start_angle + TAU) % TAU;
            if sweep == 0.0 {
                sweep = TAU;
            }
            let mut stops = vec![0.0, sweep];
            let steps = ((sweep / (PI / 32.0)).ceil() as usize).max(2);
            stops.extend((1..steps).map(|i| sweep * i as f64 / steps as f64));
            // Include cardinal extrema so bounds stay exact.
            for k in 0..4 {
                let distance = (k as f64 * PI / 2.0 - start_angle + TAU) % TAU;
                if distance > 0.0 && distance < sweep {
                    stops.push(distance);
                }
            }
            stops.sort_by(f64::total_cmp);
            let mut points: Vec<Point> = stops
                .iter()
                .map(|o| Point::new(center.x + radius * (start_angle + o).cos(), center.y + radius * (start_angle + o).sin()))
                .collect();
            points[0] = start;
            let last = points.len() - 1;
            points[last] = end;
            Some(GeomPath { points, kind: PathKind::Arc })
        }
        _ => None,
    }
}

fn same_point(a: Point, b: Point) -> bool {
    (a.x - b.x).hypot(a.y - b.y) < 1e-5
}

fn bucket(p: Point) -> (i64, i64) {
    ((p.x * 1e5).round() as i64, (p.y * 1e5).round() as i64)
}

const CONTOUR_BUDGET_BASE: usize = 1_000_000;
const CONTOUR_BUDGET_PER_PATH: usize = 200;

struct BudgetExceeded;

/// Joins unordered segments and returns the outer (largest-area) closed contour, or an empty
/// vector when no closed contour exists. Paths that already close stand alone and dangling
/// branches are pruned before chaining, so a stray open edge never consumes the edges of a loop.
pub fn outer_contour(paths: &[GeomPath]) -> Vec<Point> {
    let segments: Vec<Vec<Point>> = paths
        .iter()
        .filter(|p| p.points.len() > 1 && !p.points.iter().all(|q| same_point(*q, p.points[0])))
        .map(|p| p.points.clone())
        .collect();
    let mut contours: Vec<Vec<Point>> = Vec::new();
    let mut open: Vec<Vec<Point>> = Vec::new();
    for segment in segments {
        if same_point(segment[0], segment[segment.len() - 1]) {
            contours.push(segment);
        } else {
            open.push(segment);
        }
    }
    let mut chainer = Chainer::new(&open);
    match chainer.chain(&open) {
        Ok(chained) => contours.extend(chained),
        Err(BudgetExceeded) => return Vec::new(),
    }

    let area = |pts: &[Point]| -> f64 {
        pts.windows(2).map(|w| w[0].x * w[1].y - w[1].x * w[0].y).sum::<f64>().abs()
    };
    let mut best: Option<(f64, Vec<Point>)> = None;
    for points in contours {
        if points.len() < 4 || !same_point(points[0], points[points.len() - 1]) {
            continue;
        }
        let a = area(&points);
        if best.as_ref().is_none_or(|(b, _)| a > *b) {
            best = Some((a, points));
        }
    }
    best.map(|(_, p)| p).unwrap_or_default()
}

/// Live open paths indexed by the bucket of each of their end points, in path order.
struct Chainer {
    ends: Vec<[Point; 2]>,
    keys: Vec<[(i64, i64); 2]>,
    endpoints: HashMap<(i64, i64), Vec<usize>>,
    live: Vec<bool>,
    remaining: usize,
    budget: usize,
    work: usize,
}

impl Chainer {
    fn new(open: &[Vec<Point>]) -> Self {
        let ends: Vec<[Point; 2]> = open.iter().map(|s| [s[0], s[s.len() - 1]]).collect();
        let keys: Vec<[(i64, i64); 2]> = ends.iter().map(|e| [bucket(e[0]), bucket(e[1])]).collect();
        let mut endpoints: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
        for (index, k) in keys.iter().enumerate() {
            for key in k {
                let cell = endpoints.entry(*key).or_default();
                if !cell.contains(&index) {
                    cell.push(index);
                }
            }
        }
        Self {
            ends,
            keys,
            endpoints,
            live: vec![true; open.len()],
            remaining: open.len(),
            budget: CONTOUR_BUDGET_BASE + CONTOUR_BUDGET_PER_PATH * open.len(),
            work: 0,
        }
    }

    fn retire(&mut self, index: usize) {
        if !self.live[index] {
            return;
        }
        self.live[index] = false;
        self.remaining -= 1;
        for key in self.keys[index] {
            if let Some(cell) = self.endpoints.get_mut(&key) {
                cell.retain(|&i| i != index);
            }
        }
    }

    /// Calls `take` for each other live path with an end at `p`; stops when `take` returns true.
    fn scan(&mut self, p: Point, self_index: Option<usize>, mut take: impl FnMut(usize) -> bool) -> Result<(), BudgetExceeded> {
        let (x, y) = bucket(p);
        for dx in -1..=1 {
            for dy in -1..=1 {
                let Some(cell) = self.endpoints.get(&(x + dx, y + dy)) else { continue };
                for &index in cell {
                    self.work += 1;
                    if self.work > self.budget {
                        return Err(BudgetExceeded);
                    }
                    if Some(index) != self_index && self.ends[index].iter().any(|e| same_point(p, *e)) && take(index) {
                        return Ok(());
                    }
                }
            }
        }
        Ok(())
    }

    fn touching(&mut self, p: Point, self_index: usize) -> Result<Vec<usize>, BudgetExceeded> {
        let mut result: Vec<usize> = Vec::new();
        self.scan(p, Some(self_index), |i| {
            if !result.contains(&i) {
                result.push(i);
            }
            false
        })?;
        Ok(result)
    }

    fn touched(&mut self, p: Point, self_index: usize) -> Result<bool, BudgetExceeded> {
        let mut found = false;
        self.scan(p, Some(self_index), |_| {
            found = true;
            true
        })?;
        Ok(found)
    }

    fn first_touching(&mut self, p: Point) -> Result<Option<usize>, BudgetExceeded> {
        let mut first = None;
        self.scan(p, None, |i| {
            first = Some(i);
            true
        })?;
        Ok(first)
    }

    /// An open path with a free end cannot belong to any loop.
    fn dangling(&mut self, index: usize) -> Result<bool, BudgetExceeded> {
        let [a, b] = self.ends[index];
        Ok(!self.touched(a, index)? || !self.touched(b, index)?)
    }

    fn chain(&mut self, open: &[Vec<Point>]) -> Result<Vec<Vec<Point>>, BudgetExceeded> {
        // Prune dangling branches; removing one may free its neighbour's end.
        let mut queue: Vec<usize> = Vec::new();
        for index in 0..open.len() {
            if self.dangling(index)? {
                queue.push(index);
            }
        }
        let mut queued: HashSet<usize> = queue.iter().copied().collect();
        while let Some(index) = queue.pop() {
            queued.remove(&index);
            if !self.live[index] {
                continue;
            }
            self.retire(index);
            for p in self.ends[index] {
                for other in self.touching(p, index)? {
                    if !queued.contains(&other) && self.dangling(other)? {
                        queue.push(other);
                        queued.insert(other);
                    }
                }
            }
        }

        let mut contours = Vec::new();
        let mut cursor = 0;
        while self.remaining > 0 {
            while !self.live[cursor] {
                cursor += 1;
            }
            let first = cursor;
            self.retire(first);
            // The contour grows at its end (`tail`) and at its start (`head`, kept reversed).
            let mut tail = open[first].clone();
            let mut head: Vec<Point> = Vec::new();
            loop {
                let start = *head.last().unwrap_or(&tail[0]);
                let last = tail[tail.len() - 1];
                if same_point(start, last) {
                    break;
                }
                let append_index = self.first_touching(last)?;
                let index = match append_index {
                    Some(i) => Some(i),
                    None => self.first_touching(start)?,
                };
                let Some(index) = index else { break };
                let mut candidate = open[index].clone();
                self.retire(index);
                if append_index.is_some() {
                    if !same_point(last, candidate[0]) {
                        candidate.reverse();
                    }
                    tail.extend_from_slice(&candidate[1..]);
                } else {
                    if !same_point(start, candidate[candidate.len() - 1]) {
                        candidate.reverse();
                    }
                    head.extend(candidate[..candidate.len() - 1].iter().rev());
                }
            }
            if head.is_empty() {
                contours.push(tail);
            } else {
                head.reverse();
                head.extend(tail);
                contours.push(head);
            }
        }
        Ok(contours)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(x1: f64, y1: f64, x2: f64, y2: f64) -> GeomPath {
        GeomPath { points: vec![Point::new(x1, y1), Point::new(x2, y2)], kind: PathKind::Line }
    }

    #[test]
    fn transform_matches_mirror_then_rotate() {
        let p = Point::new(10.0, 5.0);
        let r = transform(p, 90.0, Mirror::None, Point::new(100.0, 0.0));
        assert!((r.x - 95.0).abs() < 1e-9 && (r.y - 10.0).abs() < 1e-9);
        let r = transform(p, 0.0, Mirror::X, Point::ZERO);
        assert_eq!(r, Point::new(10.0, -5.0));
        let r = transform(p, 0.0, Mirror::Y, Point::ZERO);
        assert_eq!(r, Point::new(-10.0, 5.0));
    }

    #[test]
    fn transform_equals_openboardview_mirror_formula() {
        // OpenBoardView: angle' = PI - angle when exactly one mirror; signs applied after rotation.
        let p = Point::new(3.0, 7.0);
        for (mx, my) in [(false, false), (true, false), (false, true), (true, true)] {
            for deg in [0.0, 30.0, 90.0, 270.0] {
                let sx: f64 = if mx { -1.0 } else { 1.0 };
                let sy: f64 = if my { -1.0 } else { 1.0 };
                let mut a = deg * PI / 180.0;
                if sx * sy < 0.0 {
                    a = PI - a;
                }
                let expected = Point::new(sx * (p.x * a.cos() - p.y * a.sin()), sy * (p.x * a.sin() + p.y * a.cos()));
                let got = transform(p, deg, Mirror::from_flags(mx, my), Point::ZERO);
                assert!(expected.dist(got) < 1e-9, "mx={mx} my={my} deg={deg}: {expected:?} vs {got:?}");
            }
        }
    }

    #[test]
    fn rectangle_record_uses_corner_and_size() {
        let path = parse_geometry(&["RECTANGLE", "-6.890", "-12.795", "13.780", "25.591"], |v| v).unwrap();
        let b = Rect::from_points(&path.points).unwrap();
        assert!((b.width() - 13.78).abs() < 1e-9 && (b.height() - 25.591).abs() < 1e-9);
    }

    #[test]
    fn arc_bounds_include_cardinal_extrema() {
        // Half circle from (1,0) counter-clockwise to (-1,0) through (0,1).
        let path = parse_geometry(&["ARC", "1", "0", "-1", "0", "0", "0"], |v| v).unwrap();
        let b = Rect::from_points(&path.points).unwrap();
        assert!((b.max_y - 1.0).abs() < 1e-12 && b.min_y.abs() < 1e-12);
    }

    #[test]
    fn contour_joins_unordered_segments_and_drops_branches() {
        let paths = vec![
            line(10.0, 0.0, 10.0, 5.0),
            line(0.0, 0.0, 10.0, 0.0),
            line(0.0, 5.0, 0.0, 0.0),
            line(10.0, 5.0, 20.0, 9.0), // dangling branch
            line(0.0, 5.0, 10.0, 5.0),  // reversed direction
        ];
        let outline = outer_contour(&paths);
        assert_eq!(outline.len(), 5);
        assert!(same_point(outline[0], outline[4]));
        let b = Rect::from_points(&outline).unwrap();
        assert_eq!((b.min_x, b.min_y, b.max_x, b.max_y), (0.0, 0.0, 10.0, 5.0));
    }

    #[test]
    fn contour_prefers_the_largest_closed_loop() {
        let small = parse_geometry(&["RECTANGLE", "1", "1", "2", "2"], |v| v).unwrap();
        let big = parse_geometry(&["RECTANGLE", "0", "0", "10", "10"], |v| v).unwrap();
        let outline = outer_contour(&[small, big]);
        let b = Rect::from_points(&outline).unwrap();
        assert_eq!(b.width(), 10.0);
    }

    #[test]
    fn open_artwork_has_no_contour() {
        assert!(outer_contour(&[line(0.0, 0.0, 1.0, 0.0)]).is_empty());
    }
}
