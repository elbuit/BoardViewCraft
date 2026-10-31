//! Tiled raster planning and tile texture cache for the PDF schematic viewer.
//!
//! Mirrors pdfcraft's canvas cache (`pdfcraft/crates/ui-egui/src/canvas/cache.rs`), reduced to a
//! single-page viewer: a page whose raster would exceed [`TILE_THRESHOLD`] device pixels on a side
//! is drawn as a low-res backdrop ([`BASE_SIDE`]) plus the tiles that intersect the viewport.
//! Everything here is pure (no egui context) so it can be unit tested; the cache is generic over
//! the texture type for the same reason.

use pdfcraft_render::{device_pixels, Tile};
use std::collections::{HashMap, HashSet};

/// Pages whose raster would exceed this many device pixels on a side are drawn in tiles.
pub const TILE_THRESHOLD: f32 = 4096.0;

/// Base device-pixel side of a page tile. The actual side grows with the zoom ([`tile_side`]).
pub const TILE: u32 = 1024;

/// Longest side of the low-res backdrop drawn under tiles.
pub const BASE_SIDE: f32 = 2048.0;

/// Byte allowance of retained tile textures (RGBA8).
pub const TILE_BYTES: usize = 128 * 1024 * 1024;

/// Tiles of earlier zooms kept for the page while its current tiles are rendered.
pub const OLD_TILES: usize = 64;

/// RGBA8 byte size of a `width` x `height` raster.
pub fn rgba_bytes(width: usize, height: usize) -> usize {
    width.saturating_mul(height).saturating_mul(4)
}

/// The scale of a raster tagged by `scale_tag` (inverse of `pdf::scale_tag`).
pub fn tag_scale(tag: u64) -> f32 {
    (tag as f64 / 65536.0) as f32
}

/// Whether a page `page_pts` (width, height in PDF points) at `scale` device pixels per point is
/// drawn in tiles: its long side exceeds the tiling threshold or the GPU texture limit.
pub fn is_tiled(page_pts: (f32, f32), scale: f32, max_texture_side: f32) -> bool {
    page_pts.0.max(page_pts.1).max(1.0) * scale > TILE_THRESHOLD.min(max_texture_side)
}

/// Scale of the low-res backdrop of a tiled page: its long side is [`BASE_SIDE`] (within the GPU limit).
pub fn base_scale(page_pts: (f32, f32), max_texture_side: f32) -> f32 {
    BASE_SIDE.min(max_texture_side) / page_pts.0.max(page_pts.1).max(1.0)
}

/// Device-pixel side of a page tile at `scale` for a page `page_max_px` device pixels on its long
/// side. Tiles grow with the zoom and the page so a viewport is covered by a few large tiles
/// (each tile costs a full page interpret); at most 4096, at least [`TILE`].
pub fn tile_side(scale: f32, page_max_px: u32) -> u32 {
    if !scale.is_finite() || scale <= 0.0 {
        return TILE;
    }
    let by_zoom = (TILE as f32 * (scale / 6.0)).clamp(TILE as f32, 4096.0) as u32;
    // At most four tiles along the long side, so a giant sheet needs a handful of interprets.
    by_zoom.max(page_max_px.div_ceil(4).min(4096))
}

/// Largest tile side worth rendering for a viewport whose long side is `viewport_px` device
/// pixels: the power of two covering it, within [`TILE`]..=4096. With tiles that size any viewport
/// spans at most 2 x 2 tiles, so a heavy page still costs at most four (parallel) interprets.
///
/// Deviation from pdfcraft, whose tile side depends on the zoom and page only: at deep zoom that
/// side reaches 4096 px while the viewport shows about 1 Mpx. On the MS-V375 fixture a page interpret
/// costs ~10 ms but a 4096 px tile ~80-115 ms (its pixels), and each one is a 64 MiB texture upload,
/// so a viewport-sized cap cuts the worker time and uploads of a deep-zoom jump several fold. A power
/// of two keeps the tile grid stable while the panel is resized within the same range.
pub fn viewport_tile_cap(viewport_px: f32) -> u32 {
    if !viewport_px.is_finite() || viewport_px >= 4096.0 {
        return 4096;
    }
    (viewport_px.max(1.0).ceil() as u32).next_power_of_two().clamp(TILE, 4096)
}

/// The tiles of a page `page_pts` at `scale` that intersect `visible`, the visible part of the page
/// in normalized page coordinates (`[u0, v0, u1, v1]`, 0..1 across the page), no larger than
/// [`viewport_tile_cap`] of `viewport_px` (the viewport's long side in device pixels). Sorted
/// nearest the viewport centre first, so workers sharpen what is looked at before the edges.
/// Empty when nothing of the page is visible.
pub fn visible_tiles(
    page_pts: (f32, f32),
    scale: f32,
    max_texture_side: f32,
    visible: [f32; 4],
    viewport_px: f32,
) -> Vec<Tile> {
    let [u0, v0, u1, v1] = visible;
    let (u0, v0, u1, v1) = (u0.max(0.0), v0.max(0.0), u1.min(1.0), v1.min(1.0));
    if !(u1 > u0 && v1 > v0) {
        return Vec::new();
    }
    // Sides round up as whole-page rasters do, so the last partial row and column get tiles.
    let (dw, dh) = (
        device_pixels(page_pts.0.max(1.0), scale),
        device_pixels(page_pts.1.max(1.0), scale),
    );
    // Never zero (u32 division below), never above the GPU texture limit.
    let ts = tile_side(scale, dw.max(dh))
        .min(viewport_tile_cap(viewport_px))
        .min(max_texture_side as u32)
        .max(64);
    let (vx0, vy0) = ((u0 * dw as f32) as u32, (v0 * dh as f32) as u32);
    let (vx1, vy1) = (
        ((u1 * dw as f32).ceil() as u32).min(dw),
        ((v1 * dh as f32).ceil() as u32).min(dh),
    );
    if vx1 <= vx0 || vy1 <= vy0 {
        return Vec::new();
    }
    let mut tiles = Vec::new();
    for ty in vy0 / ts..=(vy1 - 1) / ts {
        for tx in vx0 / ts..=(vx1 - 1) / ts {
            let (x, y) = (tx * ts, ty * ts);
            let (w, h) = (ts.min(dw.saturating_sub(x)), ts.min(dh.saturating_sub(y)));
            if w > 0 && h > 0 {
                tiles.push(Tile { x, y, w, h });
            }
        }
    }
    // Nearest the viewport centre first: each tile costs a full page interpret.
    let (cx, cy) = ((vx0 + vx1) as f32 / 2.0, (vy0 + vy1) as f32 / 2.0);
    let dist = |t: &Tile| ((t.x + t.w / 2) as f32 - cx).abs() + ((t.y + t.h / 2) as f32 - cy).abs();
    tiles.sort_by(|a, b| dist(a).total_cmp(&dist(b)));
    tiles
}

/// A cached tile: page (1-based), scale tag, and device-pixel origin at that scale.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TileKey {
    pub page: usize,
    pub tag: u64,
    pub x: u32,
    pub y: u32,
}

impl TileKey {
    pub fn new(page: usize, tag: u64, tile: Tile) -> Self {
        Self {
            page,
            tag,
            x: tile.x,
            y: tile.y,
        }
    }
}

struct CachedTile<T> {
    tex: T,
    tile: Tile,
    bytes: usize,
}

/// Tile textures keyed by (page, scale tag, tile origin), bounded by [`TILE_BYTES`].
pub struct TileCache<T> {
    tiles: HashMap<TileKey, CachedTile<T>>,
}

impl<T> Default for TileCache<T> {
    fn default() -> Self {
        Self {
            tiles: HashMap::new(),
        }
    }
}

impl<T> TileCache<T> {
    pub fn insert(&mut self, key: TileKey, tile: Tile, tex: T) {
        let bytes = rgba_bytes(tile.w as usize, tile.h as usize);
        self.tiles.insert(key, CachedTile { tex, tile, bytes });
    }

    pub fn contains(&self, key: &TileKey) -> bool {
        self.tiles.contains_key(key)
    }

    pub fn len(&self) -> usize {
        self.tiles.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tiles.is_empty()
    }

    pub fn clear(&mut self) {
        self.tiles.clear();
    }

    /// Retained texture bytes.
    pub fn bytes(&self) -> usize {
        self.tiles
            .values()
            .fold(0usize, |n, t| n.saturating_add(t.bytes))
    }

    /// The tiles of `wanted` not cached yet at `tag`, in the order given.
    pub fn missing(&self, page: usize, tag: u64, wanted: &[Tile]) -> Vec<Tile> {
        wanted
            .iter()
            .copied()
            .filter(|t| !self.tiles.contains_key(&TileKey::new(page, tag, *t)))
            .collect()
    }

    /// The cached tiles of `page` in paint order: earlier zooms first, farthest scale from
    /// `tag` first (so the nearest one ends up on top), then the tiles at `tag` last.
    pub fn paint_order(&self, page: usize, tag: u64) -> Vec<(TileKey, Tile, &T)> {
        let scale = tag_scale(tag);
        let mut order: Vec<(TileKey, Tile, &T)> = self
            .tiles
            .iter()
            .filter(|(k, _)| k.page == page)
            .map(|(k, c)| (*k, c.tile, &c.tex))
            .collect();
        // Current tiles sort last (distance -1); older ones by descending scale distance.
        let distance = |k: &TileKey| {
            if k.tag == tag {
                -1.0
            } else {
                scale_distance(tag_scale(k.tag), scale)
            }
        };
        order.sort_by(|a, b| {
            distance(&b.0)
                .total_cmp(&distance(&a.0))
                .then(a.0.cmp(&b.0))
        });
        order
    }

    /// Evict down to what the view can use: tiles of other pages go; the `visible` tiles of
    /// `page` at `tag` always stay; then, within [`TILE_BYTES`], tiles of earlier zooms (nearest
    /// scale first, at most [`OLD_TILES`]) stay drawable until their replacements arrive, and
    /// finally off-screen tiles at `tag` are kept for panning back.
    pub fn evict(&mut self, page: usize, tag: u64, visible: &HashSet<TileKey>) {
        self.tiles.retain(|k, _| k.page == page);
        let mut budget = self
            .tiles
            .iter()
            .filter(|(k, _)| k.tag == tag && visible.contains(k))
            .fold(0usize, |n, (_, c)| n.saturating_add(c.bytes));
        let scale = tag_scale(tag);
        let mut older: Vec<(f32, TileKey)> = self
            .tiles
            .keys()
            .filter(|k| k.tag != tag)
            .map(|k| (scale_distance(tag_scale(k.tag), scale), *k))
            .collect();
        older.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        let mut offscreen: Vec<TileKey> = self
            .tiles
            .keys()
            .filter(|k| k.tag == tag && !visible.contains(k))
            .copied()
            .collect();
        offscreen.sort_unstable();

        let mut drop = Vec::new();
        for (kept, (_, key)) in older.into_iter().enumerate() {
            let bytes = self.tiles.get(&key).map_or(0, |c| c.bytes);
            if kept >= OLD_TILES || budget.saturating_add(bytes) > TILE_BYTES {
                drop.push(key);
            } else {
                budget = budget.saturating_add(bytes);
            }
        }
        for key in offscreen {
            let bytes = self.tiles.get(&key).map_or(0, |c| c.bytes);
            if budget.saturating_add(bytes) > TILE_BYTES {
                drop.push(key);
            } else {
                budget = budget.saturating_add(bytes);
            }
        }
        for key in drop {
            self.tiles.remove(&key);
        }
    }
}

/// How far apart two raster scales are, independent of direction (2x and 0.5x are equally far).
fn scale_distance(a: f32, b: f32) -> f32 {
    (a.max(f32::MIN_POSITIVE) / b.max(f32::MIN_POSITIVE))
        .ln()
        .abs()
}

#[cfg(test)]
mod tests {
    use super::*;

    const LETTER: (f32, f32) = (612.0, 792.0);
    const MAX_SIDE: f32 = 16384.0;

    fn tile(x: u32, y: u32, w: u32, h: u32) -> Tile {
        Tile { x, y, w, h }
    }

    #[test]
    fn small_pages_render_whole_and_large_ones_tile() {
        // 792 pt * 4 = 3168 px: whole page.
        assert!(!is_tiled(LETTER, 4.0, MAX_SIDE));
        // 792 pt * 6 = 4752 px: past the threshold.
        assert!(is_tiled(LETTER, 6.0, MAX_SIDE));
        // A GPU with a 2048 px texture limit tiles much earlier.
        assert!(is_tiled(LETTER, 3.0, 2048.0));
    }

    #[test]
    fn backdrop_long_side_is_base_side_within_gpu_limit() {
        let s = base_scale(LETTER, MAX_SIDE);
        assert!((792.0 * s - BASE_SIDE).abs() < 0.01);
        let s = base_scale(LETTER, 1024.0);
        assert!((792.0 * s - 1024.0).abs() < 0.01);
    }

    #[test]
    fn tile_side_grows_with_zoom_and_page_within_bounds() {
        assert_eq!(tile_side(2.0, 5000), TILE.max(5000u32.div_ceil(4)));
        assert_eq!(tile_side(12.0, 4000), 2048);
        assert_eq!(tile_side(100.0, 4000), 4096);
        assert_eq!(tile_side(f32::NAN, 4000), TILE);
        assert_eq!(tile_side(2.0, 100_000), 4096);
    }

    #[test]
    fn only_tiles_intersecting_the_viewport_are_planned() {
        // Scale 8: 4896 x 6336 px, tile side max(1365, 1224) = 1365.
        let scale = 8.0;
        let ts = tile_side(scale, device_pixels(792.0, scale));
        let whole = visible_tiles(LETTER, scale, MAX_SIDE, [0.0, 0.0, 1.0, 1.0], f32::INFINITY);
        let cols = device_pixels(612.0, scale).div_ceil(ts) as usize;
        let rows = device_pixels(792.0, scale).div_ceil(ts) as usize;
        assert_eq!(whole.len(), cols * rows);

        // A small window in the middle of the page needs only the tiles under it.
        let part = visible_tiles(LETTER, scale, MAX_SIDE, [0.45, 0.45, 0.55, 0.55], f32::INFINITY);
        assert!(!part.is_empty() && part.len() < whole.len());
        let (dw, dh) = (
            device_pixels(612.0, scale) as f32,
            device_pixels(792.0, scale) as f32,
        );
        for t in &part {
            let (x0, y0, x1, y1) = (
                t.x as f32,
                t.y as f32,
                (t.x + t.w) as f32,
                (t.y + t.h) as f32,
            );
            assert!(
                x1 > 0.45 * dw && x0 < 0.55 * dw && y1 > 0.45 * dh && y0 < 0.55 * dh,
                "{t:?} is off screen"
            );
        }
    }

    #[test]
    fn edge_tiles_are_clipped_to_the_page_and_nearest_centre_comes_first() {
        let scale = 8.0;
        let (dw, dh) = (device_pixels(612.0, scale), device_pixels(792.0, scale));
        let tiles = visible_tiles(LETTER, scale, MAX_SIDE, [0.0, 0.0, 1.0, 1.0], f32::INFINITY);
        for t in &tiles {
            assert!(t.w > 0 && t.h > 0 && t.x + t.w <= dw && t.y + t.h <= dh);
        }
        let (cx, cy) = (dw as f32 / 2.0, dh as f32 / 2.0);
        let dist =
            |t: &Tile| ((t.x + t.w / 2) as f32 - cx).abs() + ((t.y + t.h / 2) as f32 - cy).abs();
        for pair in tiles.windows(2) {
            assert!(dist(&pair[0]) <= dist(&pair[1]));
        }
    }

    #[test]
    fn viewport_tile_cap_is_a_power_of_two_covering_the_viewport() {
        assert_eq!(viewport_tile_cap(100.0), TILE);
        assert_eq!(viewport_tile_cap(900.0), 1024);
        assert_eq!(viewport_tile_cap(1024.0), 1024);
        assert_eq!(viewport_tile_cap(1800.0), 2048);
        assert_eq!(viewport_tile_cap(5000.0), 4096);
        assert_eq!(viewport_tile_cap(f32::INFINITY), 4096);
        assert_eq!(viewport_tile_cap(f32::NAN), 4096);
    }

    #[test]
    fn deep_zoom_tiles_are_no_larger_than_the_viewport_needs() {
        // Scale 40: 24480 x 31680 px, where pdfcraft's tile side alone is 4096.
        let scale = 40.0;
        assert_eq!(tile_side(scale, device_pixels(792.0, scale)), 4096);
        let (dw, dh) = (device_pixels(612.0, scale) as f32, device_pixels(792.0, scale) as f32);
        // A 900 x 860 px viewport anywhere on the page: at most 2 x 2 tiles of 1024 px.
        for (x, y) in [(0.0, 0.0), (5000.0, 7000.0), (12345.0, 20001.0)] {
            let window = [x / dw, y / dh, (x + 900.0) / dw, (y + 860.0) / dh];
            let tiles = visible_tiles(LETTER, scale, MAX_SIDE, window, 900.0);
            assert!(!tiles.is_empty() && tiles.len() <= 4, "{tiles:?}");
            assert!(tiles.iter().all(|t| t.w <= 1024 && t.h <= 1024), "{tiles:?}");
            let pixels: u32 = tiles.iter().map(|t| t.w * t.h).sum();
            assert!(pixels <= 4 * 1024 * 1024);
        }
    }

    #[test]
    fn a_page_shown_whole_keeps_its_few_large_tiles() {
        // The page fits the viewport: the cap never binds, so a giant sheet stays a handful of interprets.
        let scale = 8.0;
        let side = device_pixels(792.0, scale) as f32;
        let capped = visible_tiles(LETTER, scale, MAX_SIDE, [0.0, 0.0, 1.0, 1.0], side);
        let uncapped = visible_tiles(LETTER, scale, MAX_SIDE, [0.0, 0.0, 1.0, 1.0], f32::INFINITY);
        assert_eq!(capped, uncapped);
    }

    #[test]
    fn nothing_visible_plans_no_tiles() {
        assert!(visible_tiles(LETTER, 8.0, MAX_SIDE, [1.2, 0.0, 1.5, 1.0], f32::INFINITY).is_empty());
        assert!(visible_tiles(LETTER, 8.0, MAX_SIDE, [0.5, 0.5, 0.5, 0.9], f32::INFINITY).is_empty());
    }

    #[test]
    fn cached_tiles_are_not_requested_again() {
        let mut cache = TileCache::default();
        let a = tile(0, 0, 1024, 1024);
        let b = tile(1024, 0, 1024, 1024);
        cache.insert(TileKey::new(1, 7, a), a, ());
        assert_eq!(cache.missing(1, 7, &[a, b]), vec![b]);
        // Same origin at another scale or page is a different tile.
        assert_eq!(cache.missing(1, 8, &[a]), vec![a]);
        assert_eq!(cache.missing(2, 7, &[a]), vec![a]);
    }

    #[test]
    fn paint_order_draws_older_zooms_first_nearest_scale_on_top() {
        let mut cache = TileCache::default();
        let t = tile(0, 0, 64, 64);
        let tag = |s: f32| (f64::from(s) * 65536.0).round() as u64;
        cache.insert(TileKey::new(1, tag(8.0), t), t, "current");
        cache.insert(TileKey::new(1, tag(2.0), t), t, "far");
        cache.insert(TileKey::new(1, tag(6.0), t), t, "near");
        cache.insert(TileKey::new(2, tag(8.0), t), t, "other page");
        let order: Vec<&str> = cache
            .paint_order(1, tag(8.0))
            .into_iter()
            .map(|(_, _, tex)| *tex)
            .collect();
        assert_eq!(order, vec!["far", "near", "current"]);
    }

    #[test]
    fn eviction_drops_other_pages_and_keeps_visible_tiles() {
        let mut cache = TileCache::default();
        let t = tile(0, 0, 64, 64);
        let key = TileKey::new(1, 7, t);
        cache.insert(key, t, ());
        cache.insert(TileKey::new(2, 7, t), t, ());
        cache.evict(1, 7, &HashSet::from([key]));
        assert_eq!(cache.len(), 1);
        assert!(cache.contains(&key));
    }

    #[test]
    fn eviction_keeps_bounded_older_zooms_nearest_scale_first() {
        let mut cache = TileCache::default();
        let current = 100 << 16;
        // OLD_TILES + 8 tiles of earlier zooms; the nearest scales are the higher tags here.
        for i in 0..(OLD_TILES as u64 + 8) {
            let t = tile(0, 0, 16, 16);
            cache.insert(TileKey::new(1, (i + 1) << 16, t), t, ());
        }
        cache.evict(1, current, &HashSet::new());
        assert_eq!(cache.len(), OLD_TILES);
        // The farthest scales (lowest tags) went first.
        assert!(!cache.contains(&TileKey::new(1, 1 << 16, tile(0, 0, 16, 16))));
        assert!(cache.contains(&TileKey::new(
            1,
            (OLD_TILES as u64 + 8) << 16,
            tile(0, 0, 16, 16)
        )));
    }

    #[test]
    fn eviction_respects_the_byte_budget_but_never_drops_visible_tiles() {
        let mut cache = TileCache::default();
        // 4096^2 RGBA = 64 MiB per tile: two fit the 128 MiB allowance.
        let big = |x: u32| tile(x, 0, 4096, 4096);
        let visible: HashSet<TileKey> = (0..3).map(|i| TileKey::new(1, 7, big(i * 4096))).collect();
        for k in &visible {
            cache.insert(*k, big(k.x), ());
        }
        let offscreen = TileKey::new(1, 7, big(3 * 4096));
        cache.insert(offscreen, big(offscreen.x), ());
        let older = TileKey::new(1, 3, big(0));
        cache.insert(older, big(0), ());
        cache.evict(1, 7, &visible);
        // Visible tiles exceed the allowance on their own: they all stay, nothing else does.
        assert!(visible.iter().all(|k| cache.contains(k)));
        assert!(!cache.contains(&offscreen));
        assert!(!cache.contains(&older));
        assert_eq!(cache.len(), 3);
    }

    #[test]
    fn eviction_prefers_older_zooms_over_offscreen_current_tiles() {
        let mut cache = TileCache::default();
        let big = |x: u32| tile(x, 0, 4096, 4096);
        let visible = TileKey::new(1, 7, big(0));
        let older = TileKey::new(1, 6, big(0));
        let offscreen = TileKey::new(1, 7, big(4096));
        cache.insert(visible, big(0), ());
        cache.insert(older, big(0), ());
        cache.insert(offscreen, big(4096), ());
        cache.evict(1, 7, &HashSet::from([visible]));
        assert!(cache.contains(&visible) && cache.contains(&older));
        assert!(!cache.contains(&offscreen));
        assert!(cache.bytes() <= TILE_BYTES);
    }

    #[test]
    fn tag_scale_inverts_scale_tag() {
        let tag = (f64::from(3.25f32) * 65536.0).round() as u64;
        assert!((tag_scale(tag) - 3.25).abs() < 1e-4);
    }
}
