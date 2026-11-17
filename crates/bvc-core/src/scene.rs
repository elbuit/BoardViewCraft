//! Both-faces scene: one board holding the top face and the mirrored bottom face side by side.
//!
//! The scene is an ordinary [`Board`] in a single world space, so a viewer can draw, pan, zoom
//! and hit-test both faces as one picture:
//! - items only on the top face keep their position and index;
//! - items only on the bottom face are mirrored in X within the board's bounding box (as a
//!   bottom view shows them) and shifted by the face offset, keeping their index;
//! - items on both faces (through-hole pins, vias) keep their top-face copy at their index and
//!   get a mirrored bottom-face duplicate appended at the end; [`BothFacesScene::original_pin`]
//!   and [`BothFacesScene::original_via`] map a duplicate back to the real item.

use crate::board::Board;
use crate::model::{Point, Rect, Side};

/// A board laid out as both faces in one world space (see the module docs).
#[derive(Debug, Clone)]
pub struct BothFacesScene {
    pub board: Board,
    /// Original index of each appended bottom-face pin duplicate (scene index `pin_count + k`).
    pin_dups: Vec<usize>,
    via_dups: Vec<usize>,
    pin_count: usize,
    via_count: usize,
    /// `min_x + max_x` of the source board: mirroring maps `x` to `mirror_sum - x`.
    mirror_sum: f64,
    /// Shift of the mirrored bottom face.
    offset: Point,
}

fn mirror_angle(deg: f64) -> f64 {
    (-deg).rem_euclid(360.0)
}

impl BothFacesScene {
    /// Builds the scene of `source`, its bottom face shifted by `offset` (world units) after
    /// mirroring. `None` when the board has no bounding box.
    pub fn build(source: &Board, offset: Point) -> Option<Self> {
        let bbox = source.bounding_box?;
        let mirror_sum = bbox.min_x + bbox.max_x;
        let flip = |p: Point| Point::new(mirror_sum - p.x + offset.x, p.y + offset.y);

        let mut board = source.clone();
        let (pin_count, via_count) = (board.pins.len(), board.vias.len());

        for part in &mut board.parts {
            if part.side != Side::Bottom {
                continue;
            }
            part.p1 = flip(part.p1);
            part.p2 = flip(part.p2);
            part.rotation_deg = mirror_angle(part.rotation_deg);
            for p in &mut part.outline {
                *p = flip(*p);
            }
            part.bounding_box = part.bounding_box.map(|r| Rect {
                min_x: mirror_sum - r.max_x + offset.x,
                max_x: mirror_sum - r.min_x + offset.x,
                min_y: r.min_y + offset.y,
                max_y: r.max_y + offset.y,
            });
        }

        let mut pin_dups = Vec::new();
        for idx in 0..pin_count {
            let pin = &mut board.pins[idx];
            match pin.side {
                Side::Bottom => {
                    pin.pos = flip(pin.pos);
                    pin.rotation_deg = mirror_angle(pin.rotation_deg);
                }
                Side::Both => {
                    let mut dup = pin.clone();
                    dup.pos = flip(dup.pos);
                    dup.rotation_deg = mirror_angle(dup.rotation_deg);
                    dup.id = pin_count + pin_dups.len();
                    board.pins.push(dup);
                    pin_dups.push(idx);
                }
                Side::Top => {}
            }
        }

        let mut via_dups = Vec::new();
        for idx in 0..via_count {
            let via = &mut board.vias[idx];
            match via.side {
                Side::Bottom => via.pos = flip(via.pos),
                Side::Both => {
                    let mut dup = via.clone();
                    dup.pos = flip(dup.pos);
                    dup.id = via_count + via_dups.len();
                    board.vias.push(dup);
                    via_dups.push(idx);
                }
                Side::Top => {}
            }
        }

        let outline: Vec<Point> = if source.outline_polygon.len() >= 3 {
            source.outline_polygon.clone()
        } else {
            vec![
                Point::new(bbox.min_x, bbox.min_y),
                Point::new(bbox.max_x, bbox.min_y),
                Point::new(bbox.max_x, bbox.max_y),
                Point::new(bbox.min_x, bbox.max_y),
            ]
        };
        board.extra_outlines = vec![outline.iter().rev().map(|p| flip(*p)).collect()];
        board.outline_polygon = outline;

        board.rebuild_indices();
        Some(Self { board, pin_dups, via_dups, pin_count, via_count, mirror_sum, offset })
    }

    /// The real pin behind scene pin `idx` (itself unless it is a bottom-face duplicate).
    pub fn original_pin(&self, idx: usize) -> usize {
        idx.checked_sub(self.pin_count).and_then(|k| self.pin_dups.get(k).copied()).unwrap_or(idx)
    }

    /// The real via behind scene via `idx` (itself unless it is a bottom-face duplicate).
    pub fn original_via(&self, idx: usize) -> usize {
        idx.checked_sub(self.via_count).and_then(|k| self.via_dups.get(k).copied()).unwrap_or(idx)
    }

    /// Scene position of source point `p` seen on `side` (`Both` counts as the top face).
    pub fn to_scene(&self, p: Point, side: Side) -> Point {
        if side == Side::Bottom {
            Point::new(self.mirror_sum - p.x + self.offset.x, p.y + self.offset.y)
        } else {
            p
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Part, Pin, Via};

    /// 100x50 board: U1 on top, U2 on the bottom, a through-hole pin and a via on both faces.
    fn board() -> Board {
        let mut b = Board::new();
        b.outline_polygon = vec![Point::new(0.0, 0.0), Point::new(100.0, 0.0), Point::new(100.0, 50.0), Point::new(0.0, 50.0)];
        let bbox = |x: f64| Some(Rect { min_x: x - 5.0, min_y: 20.0, max_x: x + 5.0, max_y: 30.0 });
        b.parts.push(Part { id: 0, name: "U1".into(), side: Side::Top, bounding_box: bbox(20.0), rotation_deg: 30.0, ..Default::default() });
        b.parts.push(Part { id: 1, name: "U2".into(), side: Side::Bottom, bounding_box: bbox(20.0), rotation_deg: 30.0, ..Default::default() });
        b.pins.push(Pin { id: 0, part_id: 0, name: "1".into(), pos: Point::new(20.0, 25.0), side: Side::Top, net_name: "A".into(), ..Default::default() });
        b.pins.push(Pin { id: 1, part_id: 1, name: "1".into(), pos: Point::new(20.0, 25.0), side: Side::Bottom, net_name: "A".into(), rotation_deg: 90.0, ..Default::default() });
        b.pins.push(Pin { id: 2, part_id: 0, name: "2".into(), pos: Point::new(10.0, 10.0), side: Side::Both, net_name: "B".into(), ..Default::default() });
        b.vias.push(Via { id: 0, pos: Point::new(70.0, 40.0), side: Side::Both, net_name: "B".into(), radius: 2.0 });
        b.rebuild_indices();
        b
    }

    #[test]
    fn top_items_stay_and_bottom_items_are_mirrored_then_shifted() {
        let src = board();
        let scene = BothFacesScene::build(&src, Point::new(120.0, 0.0)).expect("scene");
        let b = &scene.board;
        // Top part and pin untouched, same indices.
        assert_eq!(b.parts[0].bounding_box, src.parts[0].bounding_box);
        assert_eq!(b.pins[0].pos, Point::new(20.0, 25.0));
        // Bottom: x -> 100 - x + 120.
        assert_eq!(b.pins[1].pos, Point::new(200.0, 25.0));
        assert_eq!(b.pins[1].rotation_deg, 270.0);
        let r = b.parts[1].bounding_box.expect("bbox");
        assert_eq!((r.min_x, r.max_x, r.min_y, r.max_y), (195.0, 205.0, 20.0, 30.0));
        assert_eq!(b.parts[1].rotation_deg, 330.0);
        // Second outline is the mirrored board, and the scene bbox covers both faces.
        assert_eq!(b.extra_outlines.len(), 1);
        let xs: Vec<f64> = b.extra_outlines[0].iter().map(|p| p.x).collect();
        assert!(xs.iter().all(|x| (120.0..=220.0).contains(x)));
        let bbox = b.bounding_box.expect("scene bbox");
        assert_eq!((bbox.min_x, bbox.max_x), (0.0, 220.0));
    }

    #[test]
    fn items_on_both_faces_are_duplicated_and_map_back() {
        let src = board();
        let scene = BothFacesScene::build(&src, Point::new(0.0, -70.0)).expect("scene");
        let b = &scene.board;
        assert_eq!(b.pins.len(), 4);
        assert_eq!(b.vias.len(), 2);
        // Top copy unchanged, bottom duplicate mirrored and shifted down.
        assert_eq!(b.pins[2].pos, Point::new(10.0, 10.0));
        assert_eq!(b.pins[3].pos, Point::new(90.0, -60.0));
        assert_eq!(b.vias[1].pos, Point::new(30.0, -30.0));
        assert_eq!(scene.original_pin(3), 2);
        assert_eq!(scene.original_pin(1), 1);
        assert_eq!(scene.original_via(1), 0);
        // Nets see both copies; part lookups still work.
        assert_eq!(b.get_net("B").expect("net").pin_indices.len(), 2);
        assert_eq!(b.part_by_name.get("U2"), Some(&1));
    }

    #[test]
    fn scene_points_follow_the_face() {
        let scene = BothFacesScene::build(&board(), Point::new(120.0, 0.0)).expect("scene");
        assert_eq!(scene.to_scene(Point::new(20.0, 25.0), Side::Top), Point::new(20.0, 25.0));
        assert_eq!(scene.to_scene(Point::new(20.0, 25.0), Side::Both), Point::new(20.0, 25.0));
        assert_eq!(scene.to_scene(Point::new(20.0, 25.0), Side::Bottom), Point::new(200.0, 25.0));
    }

    #[test]
    fn boards_without_bounds_have_no_scene() {
        assert!(BothFacesScene::build(&Board::new(), Point::new(0.0, 0.0)).is_none());
    }
}
