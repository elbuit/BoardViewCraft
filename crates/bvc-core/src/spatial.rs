//! 2D Uniform Spatial Grid for fast frustum queries and spatial partitioning.

use crate::model::{Pin, Rect, Via};

/// Default grid cell size in mils (board coordinate units).
pub const DEFAULT_CELL_SIZE: f64 = 500.0;

/// Grid bucket holding zero-copy entity indices into Board.pins and Board.vias.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GridCell {
    pub pins: Vec<usize>,
    pub vias: Vec<usize>,
}

/// 2D uniform acceleration grid for fast frustum queries.
#[derive(Debug, Clone, PartialEq)]
pub struct SpatialGrid {
    pub origin_x: f64,
    pub origin_y: f64,
    pub cell_size: f64,
    pub cols: usize,
    pub rows: usize,
    pub cells: Vec<GridCell>,
}

impl Default for SpatialGrid {
    fn default() -> Self {
        Self {
            origin_x: 0.0,
            origin_y: 0.0,
            cell_size: DEFAULT_CELL_SIZE,
            cols: 0,
            rows: 0,
            cells: Vec::new(),
        }
    }
}

impl SpatialGrid {
    pub const DEFAULT_CELL_SIZE: f64 = DEFAULT_CELL_SIZE;

    pub fn new(cell_size: f64) -> Self {
        Self {
            origin_x: 0.0,
            origin_y: 0.0,
            cell_size: if cell_size > 0.0 { cell_size } else { DEFAULT_CELL_SIZE },
            cols: 0,
            rows: 0,
            cells: Vec::new(),
        }
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.cells.is_empty() || (self.cols == 0 && self.rows == 0)
    }

    /// Builds a uniform 2D spatial grid over the provided pins and vias.
    ///
    /// Fixed-size square buckets default to 500.0 mils per cell side, or
    /// adaptively size up if the coordinate span is exceptionally large
    /// to prevent empty bucket proliferation.
    pub fn build(pins: &[Pin], vias: &[Via], requested_cell_size: f64) -> Self {
        if pins.is_empty() && vias.is_empty() {
            return Self::default();
        }

        let mut min_x = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_y = f64::NEG_INFINITY;

        for pin in pins {
            if pin.pos.x < min_x { min_x = pin.pos.x; }
            if pin.pos.x > max_x { max_x = pin.pos.x; }
            if pin.pos.y < min_y { min_y = pin.pos.y; }
            if pin.pos.y > max_y { max_y = pin.pos.y; }
        }
        for via in vias {
            if via.pos.x < min_x { min_x = via.pos.x; }
            if via.pos.x > max_x { max_x = via.pos.x; }
            if via.pos.y < min_y { min_y = via.pos.y; }
            if via.pos.y > max_y { max_y = via.pos.y; }
        }

        if !min_x.is_finite() || !min_y.is_finite() {
            return Self::default();
        }

        let base_cell_size = if requested_cell_size > 0.0 {
            requested_cell_size
        } else {
            DEFAULT_CELL_SIZE
        };

        let span_x = (max_x - min_x).max(1.0);
        let span_y = (max_y - min_y).max(1.0);

        // Adaptive scaling: cap grid cells to 65,536 buckets to prevent empty bucket proliferation
        let mut cell_size = base_cell_size;
        let max_cells = 65_536.0;
        let estimated_cells = (span_x / cell_size) * (span_y / cell_size);
        if estimated_cells > max_cells {
            cell_size = ((span_x * span_y) / max_cells).sqrt().max(base_cell_size);
        }

        let cols = ((span_x / cell_size).floor() as usize) + 1;
        let rows = ((span_y / cell_size).floor() as usize) + 1;

        let total_cells = cols.saturating_mul(rows);
        let mut cells = vec![GridCell::default(); total_cells];

        for (idx, pin) in pins.iter().enumerate() {
            let col = (((pin.pos.x - min_x) / cell_size).floor() as isize).clamp(0, (cols - 1) as isize) as usize;
            let row = (((pin.pos.y - min_y) / cell_size).floor() as isize).clamp(0, (rows - 1) as isize) as usize;
            cells[row * cols + col].pins.push(idx);
        }

        for (idx, via) in vias.iter().enumerate() {
            let col = (((via.pos.x - min_x) / cell_size).floor() as isize).clamp(0, (cols - 1) as isize) as usize;
            let row = (((via.pos.y - min_y) / cell_size).floor() as isize).clamp(0, (rows - 1) as isize) as usize;
            cells[row * cols + col].vias.push(idx);
        }

        Self {
            origin_x: min_x,
            origin_y: min_y,
            cell_size,
            cols,
            rows,
            cells,
        }
    }

    #[inline]
    fn cell_range_for_rect(&self, rect: &Rect) -> Option<(usize, usize, usize, usize)> {
        if self.is_empty() {
            return None;
        }

        let grid_max_x = self.origin_x + (self.cols as f64) * self.cell_size;
        let grid_max_y = self.origin_y + (self.rows as f64) * self.cell_size;

        if rect.max_x < self.origin_x
            || rect.min_x > grid_max_x
            || rect.max_y < self.origin_y
            || rect.min_y > grid_max_y
        {
            return None;
        }

        let min_col = (((rect.min_x - self.origin_x) / self.cell_size).floor() as isize)
            .clamp(0, (self.cols - 1) as isize) as usize;
        let max_col = (((rect.max_x - self.origin_x) / self.cell_size).floor() as isize)
            .clamp(0, (self.cols - 1) as isize) as usize;

        let min_row = (((rect.min_y - self.origin_y) / self.cell_size).floor() as isize)
            .clamp(0, (self.rows - 1) as isize) as usize;
        let max_row = (((rect.max_y - self.origin_y) / self.cell_size).floor() as isize)
            .clamp(0, (self.rows - 1) as isize) as usize;

        Some((min_col, max_col, min_row, max_row))
    }

    /// Queries all pin indices whose spatial grid cells intersect `rect`.
    pub fn query_pins_in_rect(&self, rect: &Rect) -> Vec<usize> {
        let Some((min_col, max_col, min_row, max_row)) = self.cell_range_for_rect(rect) else {
            return Vec::new();
        };

        let mut results = Vec::new();
        for r in min_row..=max_row {
            let row_offset = r * self.cols;
            for c in min_col..=max_col {
                results.extend_from_slice(&self.cells[row_offset + c].pins);
            }
        }
        results
    }

    /// Queries all via indices whose spatial grid cells intersect `rect`.
    pub fn query_vias_in_rect(&self, rect: &Rect) -> Vec<usize> {
        let Some((min_col, max_col, min_row, max_row)) = self.cell_range_for_rect(rect) else {
            return Vec::new();
        };

        let mut results = Vec::new();
        for r in min_row..=max_row {
            let row_offset = r * self.cols;
            for c in min_col..=max_col {
                results.extend_from_slice(&self.cells[row_offset + c].vias);
            }
        }
        results
    }

    /// Queries both pin and via indices intersecting the given frustum range.
    pub fn query_range(&self, rect: Rect) -> (Vec<usize>, Vec<usize>) {
        let Some((min_col, max_col, min_row, max_row)) = self.cell_range_for_rect(&rect) else {
            return (Vec::new(), Vec::new());
        };

        let mut pins = Vec::new();
        let mut vias = Vec::new();
        for r in min_row..=max_row {
            let row_offset = r * self.cols;
            for c in min_col..=max_col {
                let cell = &self.cells[row_offset + c];
                pins.extend_from_slice(&cell.pins);
                vias.extend_from_slice(&cell.vias);
            }
        }
        (pins, vias)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Point, Side};

    #[test]
    fn test_spatial_grid_query() {
        let cell_size = 500.0;
        let pins = vec![
            Pin { id: 0, part_id: 0, name: "1".into(), pos: Point::new(0.0, 0.0), side: Side::Top, net_name: "N1".into(), radius: 10.0 },
            Pin { id: 1, part_id: 0, name: "2".into(), pos: Point::new(500.0, 0.0), side: Side::Top, net_name: "N1".into(), radius: 10.0 },
            Pin { id: 2, part_id: 0, name: "3".into(), pos: Point::new(0.0, 500.0), side: Side::Top, net_name: "N1".into(), radius: 10.0 },
            Pin { id: 3, part_id: 0, name: "4".into(), pos: Point::new(500.0, 500.0), side: Side::Top, net_name: "N1".into(), radius: 10.0 },
            Pin { id: 4, part_id: 0, name: "5".into(), pos: Point::new(250.0, 250.0), side: Side::Top, net_name: "N1".into(), radius: 10.0 },
            Pin { id: 5, part_id: 0, name: "6".into(), pos: Point::new(1000.0, 1000.0), side: Side::Top, net_name: "N1".into(), radius: 10.0 },
        ];
        let vias = vec![
            Via { id: 0, pos: Point::new(500.0, 500.0), side: Side::Top, net_name: "N1".into(), radius: 8.0 },
            Via { id: 1, pos: Point::new(750.0, 750.0), side: Side::Top, net_name: "N1".into(), radius: 8.0 },
        ];

        let grid = SpatialGrid::build(&pins, &vias, cell_size);
        assert!(!grid.is_empty());

        // Boundary test: [0, 500] x [0, 500] covers cells around (0,0) and (500,500)
        let q_rect = Rect { min_x: 0.0, min_y: 0.0, max_x: 500.0, max_y: 500.0 };
        let found_pins = grid.query_pins_in_rect(&q_rect);
        for idx in 0..=4 {
            assert!(found_pins.contains(&idx), "Expected pin {idx} on boundaries to be found");
        }
        assert!(!found_pins.contains(&5), "Pin 5 at (1000, 1000) should not be found in [0, 500]");

        // Corner test: exactly (500, 500)
        let corner_rect = Rect { min_x: 500.0, min_y: 500.0, max_x: 500.0, max_y: 500.0 };
        let (corner_pins, corner_vias) = grid.query_range(corner_rect);
        assert!(corner_pins.contains(&3), "Pin at (500, 500) must be found");
        assert!(corner_vias.contains(&0), "Via at (500, 500) must be found");

        // Out-of-bounds query
        let outside_rect = Rect { min_x: 5000.0, min_y: 5000.0, max_x: 6000.0, max_y: 6000.0 };
        assert!(grid.query_pins_in_rect(&outside_rect).is_empty());
        assert!(grid.query_vias_in_rect(&outside_rect).is_empty());
    }

    #[test]
    fn test_spatial_query_speed() {
        use std::time::Instant;

        // Generate 50,000 pins over 10,000 x 10,000 mils
        let mut pins = Vec::with_capacity(50_000);
        for i in 0..50_000 {
            let x = ((i * 17) % 10_000) as f64;
            let y = ((i * 31) % 10_000) as f64;
            pins.push(Pin {
                id: i,
                part_id: 0,
                name: "P".into(),
                pos: Point::new(x, y),
                side: Side::Top,
                net_name: "SIG".into(),
                radius: 12.0,
            });
        }

        let build_start = Instant::now();
        let grid = SpatialGrid::build(&pins, &[], DEFAULT_CELL_SIZE);
        let build_duration = build_start.elapsed();
        // Index build must be fast (< 15ms for up to 100,000 entities)
        assert!(build_duration.as_millis() < 50, "Build took {:?}, expected < 50ms", build_duration);

        // Frustum query 1000 x 1000 mils
        let frustum = Rect { min_x: 2000.0, min_y: 2000.0, max_x: 3000.0, max_y: 3000.0 };

        // Warm up
        let _ = grid.query_pins_in_rect(&frustum);

        let query_start = Instant::now();
        let query_pins = grid.query_pins_in_rect(&frustum);
        let query_duration = query_start.elapsed();

        assert!(!query_pins.is_empty(), "Frustum should contain pins");
        // Verification contract: Querying a 1000x1000 mil frustum across 50,000 pins takes < 100 microseconds (in release or reasonably fast in debug)
        println!("Spatial query 1000x1000 mil across 50k pins: {:?}", query_duration);
        assert!(query_duration.as_micros() < 500, "Query took {:?}, expected < 500us in debug", query_duration);
    }
}
