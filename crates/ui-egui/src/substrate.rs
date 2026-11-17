//! Board substrate patterns rendered as a repeating texture tile.
//!
//! The pattern is anchored to the board in world space, so it grows and shrinks with the
//! zoom like the rest of the board. Each pattern is rasterized (supersampled, so edges are
//! antialiased) into a chain of tiles from `MIN_LEVEL_PX` to `MAX_LEVEL_PX` pixels, a
//! hand-made mipmap chain because the wgpu backend uploads textures without mip levels.
//! The level closest to the on-screen period is picked, so the texture is never minified
//! more than 2x (no aliasing): zooming out blurs the lines into the substrate tint, and
//! below `FADE_END_PX` the pattern fades away entirely.

use std::collections::HashMap;

use egui::{Color32, ColorImage, Context, Mesh, Pos2, TextureHandle, TextureId, TextureOptions};

use crate::theme::BoardHatchingPattern;

/// Samples per pixel axis used when rasterizing a tile (4x4 = 16 samples per pixel).
const SUPERSAMPLE: u32 = 4;

/// Smallest and largest tile edge of the level chain, in physical pixels (powers of two).
const MIN_LEVEL_PX: usize = 4;
const MAX_LEVEL_PX: usize = 256;

/// On-screen pattern period (physical pixels) at which the pattern is fully gone / fully shown.
const FADE_END_PX: f32 = 4.0;
const FADE_START_PX: f32 = 12.0;

/// Board size (screen points along its shortest side) at which each pattern has its nominal
/// period from `tile_period_pts`; the pattern scales with the board from there.
const REFERENCE_BOARD_PTS: f32 = 600.0;

/// Stroke width as a fraction of the pattern period.
const LINE_FRACTION: f32 = 0.06;

/// Uploaded tile chain of one pattern, ready to be mapped onto the board fill.
#[derive(Clone, Debug, PartialEq)]
pub struct SubstrateTileSet {
    pub pattern: BoardHatchingPattern,
    /// `(edge in physical pixels, texture)` from `MIN_LEVEL_PX` up to `MAX_LEVEL_PX`.
    pub levels: Vec<(usize, TextureId)>,
    /// Physical pixels per screen point.
    pub pixels_per_point: f32,
}

impl SubstrateTileSet {
    /// Identity of the uploaded chain (changes with pattern and DPI scale).
    pub fn id(&self) -> Option<TextureId> {
        self.levels.first().map(|(_, id)| *id)
    }

    /// Smallest level whose edge is at least `period_px`, else the largest one.
    fn level_for(&self, period_px: f32) -> Option<TextureId> {
        self.levels.iter().find(|(px, _)| *px as f32 >= period_px).or(self.levels.last()).map(|(_, id)| *id)
    }
}

/// Nominal repetition period of each pattern in screen points (see `REFERENCE_BOARD_PTS`).
fn tile_period_pts(pattern: BoardHatchingPattern) -> f32 {
    match pattern {
        BoardHatchingPattern::None => 0.0,
        BoardHatchingPattern::Horizontal | BoardHatchingPattern::Grid => 8.0,
        // 45° lines repeating every P along X are P/√2 apart; 11 keeps them ~8 points apart.
        BoardHatchingPattern::Diagonal | BoardHatchingPattern::CrossHatch => 11.0,
    }
}

/// Pattern period in world units for a board whose shortest side measures `board_min_world`.
pub fn world_period(pattern: BoardHatchingPattern, board_min_world: f32) -> f32 {
    board_min_world * tile_period_pts(pattern) / REFERENCE_BOARD_PTS
}

/// Coverage in `0.0..=1.0` of `pattern` at tile-local position (`x`, `y`), both in `0..size`.
/// `line` is the stroke width in tile pixels.
fn coverage(pattern: BoardHatchingPattern, x: f32, y: f32, size: f32, line: f32) -> f32 {
    let band = |d: f32| if d.rem_euclid(size) < line { 1.0 } else { 0.0 };
    // Perpendicular distance test for 45° lines repeating every `size` along X.
    let diag = |d: f32| {
        let m = d.rem_euclid(size);
        let dist = m.min(size - m) / std::f32::consts::SQRT_2;
        if dist < line * 0.5 { 1.0 } else { 0.0 }
    };
    match pattern {
        BoardHatchingPattern::None => 0.0,
        BoardHatchingPattern::Horizontal => band(y),
        BoardHatchingPattern::Grid => band(x).max(band(y)),
        BoardHatchingPattern::Diagonal => diag(x - y),
        BoardHatchingPattern::CrossHatch => diag(x - y).max(diag(x + y)),
    }
}

/// Rasterizes one period of `pattern` into a `size_px`-square white tile whose alpha carries
/// the coverage. Strokes scale with the tile, so smaller levels hold the same pattern averaged
/// down, exactly what a mip level would contain.
pub fn pattern_tile_image(pattern: BoardHatchingPattern, size_px: usize) -> ColorImage {
    let size = size_px as f32;
    let line = size * LINE_FRACTION;
    let step = 1.0 / SUPERSAMPLE as f32;
    let samples = (SUPERSAMPLE * SUPERSAMPLE) as f32;
    let mut rgba = Vec::with_capacity(size_px * size_px * 4);
    for py in 0..size_px {
        for px in 0..size_px {
            let mut sum = 0.0;
            for sy in 0..SUPERSAMPLE {
                for sx in 0..SUPERSAMPLE {
                    let x = px as f32 + (sx as f32 + 0.5) * step;
                    let y = py as f32 + (sy as f32 + 0.5) * step;
                    sum += coverage(pattern, x, y, size, line);
                }
            }
            let a = ((sum / samples).clamp(0.0, 1.0) * 255.0).round() as u8;
            rgba.extend_from_slice(&[255, 255, 255, a]);
        }
    }
    ColorImage::from_rgba_unmultiplied([size_px, size_px], &rgba)
}

/// Uploaded tile chains, keyed by pattern.
#[derive(Default)]
pub struct SubstrateTextures {
    handles: HashMap<BoardHatchingPattern, Vec<(usize, TextureHandle)>>,
}

impl SubstrateTextures {
    /// The tile chain for `pattern`, uploading it on first use.
    pub fn tile_set(&mut self, ctx: &Context, pattern: BoardHatchingPattern) -> Option<SubstrateTileSet> {
        if tile_period_pts(pattern) <= 0.0 {
            return None;
        }
        let handles = self.handles.entry(pattern).or_insert_with(|| {
            std::iter::successors(Some(MIN_LEVEL_PX), |px| (*px < MAX_LEVEL_PX).then_some(px * 2))
                .map(|px| {
                    let image = pattern_tile_image(pattern, px);
                    (px, ctx.load_texture(format!("substrate_{pattern:?}_{px}"), image, TextureOptions::LINEAR_REPEAT))
                })
                .collect()
        });
        Some(SubstrateTileSet {
            pattern,
            levels: handles.iter().map(|(px, h)| (*px, h.id())).collect(),
            pixels_per_point: ctx.pixels_per_point(),
        })
    }
}

/// Visibility of the pattern for an on-screen period of `period_px` physical pixels.
fn fade_factor(period_px: f32) -> f32 {
    let t = ((period_px - FADE_END_PX) / (FADE_START_PX - FADE_END_PX)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// The board `fill` mesh re-textured with the level of `tiles` matching the on-screen period.
///
/// `to_world` maps a screen position to world coordinates; `anchor` (world) is where the
/// pattern starts, `period_world` its repetition in world units and `screen_per_world` the
/// current zoom. UVs are world-anchored, so a zoom-remapped cached mesh stays correct.
/// Returns `None` when the pattern has faded out or the tint is invisible.
pub fn board_substrate_mesh(
    fill: &Mesh,
    tiles: &SubstrateTileSet,
    color: Color32,
    anchor: Pos2,
    period_world: f32,
    screen_per_world: f32,
    to_world: impl Fn(Pos2) -> Pos2,
) -> Option<Mesh> {
    if fill.is_empty() || color.a() == 0 || period_world <= 0.0 {
        return None;
    }
    let period_px = period_world * screen_per_world * tiles.pixels_per_point;
    let tint = color.gamma_multiply(fade_factor(period_px));
    if tint.a() == 0 {
        return None;
    }
    let texture_id = tiles.level_for(period_px)?;

    let mut mesh = fill.clone();
    mesh.texture_id = texture_id;
    for v in &mut mesh.vertices {
        let w = to_world(v.pos);
        v.uv = Pos2::new((w.x - anchor.x) / period_world, (w.y - anchor.y) / period_world);
        v.color = tint;
    }
    Some(mesh)
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::epaint::Vertex;

    fn alpha(img: &ColorImage, x: usize, y: usize) -> u8 {
        img.pixels[y * img.size[0] + x].a()
    }

    #[test]
    fn horizontal_tile_draws_a_band_proportional_to_the_period() {
        let img = pattern_tile_image(BoardHatchingPattern::Horizontal, 64);
        // 6 % of 64 px = 3.84 px: 3 solid rows, one partial row, then empty.
        for x in 0..64 {
            assert_eq!(alpha(&img, x, 0), 255);
            assert_eq!(alpha(&img, x, 2), 255);
            let partial = alpha(&img, x, 3);
            assert!(partial > 0 && partial < 255, "row 3 should be antialiased, got {partial}");
            assert_eq!(alpha(&img, x, 4), 0);
            assert_eq!(alpha(&img, x, 40), 0);
        }
    }

    #[test]
    fn every_level_holds_the_same_average_ink() {
        // Like mip levels, smaller tiles must carry the same mean coverage as larger ones.
        for p in BoardHatchingPattern::ALL {
            let mean = |px: usize| {
                let img = pattern_tile_image(p, px);
                img.pixels.iter().map(|c| c.a() as f32).sum::<f32>() / (px * px) as f32
            };
            let (small, large) = (mean(8), mean(128));
            assert!((small - large).abs() < 6.0, "{p:?}: 8px mean {small} vs 128px mean {large}");
        }
    }

    #[test]
    fn removed_patterns_load_as_none() {
        for legacy in ["\"Dots\"", "\"Weave\""] {
            let p: BoardHatchingPattern = serde_json::from_str(legacy).expect("legacy value");
            assert_eq!(p, BoardHatchingPattern::None, "{legacy}");
        }
    }

    #[test]
    fn every_pattern_tiles_seamlessly() {
        // A periodic pattern must have the same coverage one period apart.
        for p in BoardHatchingPattern::ALL {
            let size = 16.0;
            for i in 0..40 {
                let (x, y) = (i as f32 * 0.37 + 0.11, i as f32 * 0.53 + 0.07);
                let a = coverage(p, x, y, size, 2.0);
                let b = coverage(p, x + size, y + size, size, 2.0);
                assert!((a - b).abs() < 1e-4, "{p:?} not periodic at ({x},{y})");
            }
        }
    }

    #[test]
    fn non_empty_patterns_have_ink() {
        for p in BoardHatchingPattern::ALL {
            let img = pattern_tile_image(p, 16);
            let ink = img.pixels.iter().any(|c| c.a() > 0);
            assert_eq!(ink, p != BoardHatchingPattern::None, "{p:?}");
        }
    }

    fn square_fill(lo: f32, hi: f32) -> Mesh {
        let pts = [Pos2::new(lo, lo), Pos2::new(hi, lo), Pos2::new(hi, hi), Pos2::new(lo, hi)];
        let mut mesh = Mesh::default();
        for p in pts {
            mesh.vertices.push(Vertex { pos: p, uv: Pos2::ZERO, color: Color32::WHITE });
        }
        mesh.indices = vec![0, 1, 2, 0, 2, 3];
        mesh
    }

    fn tiles() -> SubstrateTileSet {
        let levels = [4, 8, 16, 32, 64, 128, 256].iter().enumerate().map(|(i, px)| (*px, TextureId::User(i as u64))).collect();
        SubstrateTileSet { pattern: BoardHatchingPattern::Grid, levels, pixels_per_point: 1.0 }
    }

    #[test]
    fn substrate_uvs_follow_world_coordinates() {
        let fill = square_fill(100.0, 300.0);
        let color = Color32::from_rgba_premultiplied(0x35, 0x40, 0x50, 0x50);
        // 2 screen points per world unit, period 10 world units (20 px on screen).
        let to_world = |p: Pos2| Pos2::new(p.x / 2.0, p.y / 2.0);
        let mesh = board_substrate_mesh(&fill, &tiles(), color, Pos2::new(50.0, 50.0), 10.0, 2.0, to_world).expect("mesh");
        assert_eq!(mesh.indices, fill.indices);
        assert_eq!(mesh.vertices[0].uv, Pos2::ZERO);
        assert_eq!(mesh.vertices[2].uv, Pos2::new(10.0, 10.0));
        assert!(mesh.vertices.iter().all(|v| v.color == color));
        // 20 px period -> smallest level >= 20 px is 32 px (index 3).
        assert_eq!(mesh.texture_id, TextureId::User(3));
    }

    #[test]
    fn zoom_picks_the_level_and_fades_out_dense_patterns() {
        let fill = square_fill(0.0, 100.0);
        let id = |p: Pos2| p;
        let at = |scale: f32| board_substrate_mesh(&fill, &tiles(), Color32::WHITE, Pos2::ZERO, 10.0, scale, id);
        // Zoomed far in: larger than the largest level, still the 256 px tile.
        assert_eq!(at(100.0).unwrap().texture_id, TextureId::User(6));
        // Fully visible above FADE_START_PX.
        assert_eq!(at(2.0).unwrap().vertices[0].color, Color32::WHITE);
        // Fading between FADE_END_PX and FADE_START_PX (8 px period).
        let a = at(0.8).unwrap().vertices[0].color.a();
        assert!(a > 0 && a < 255, "expected partial fade, got {a}");
        // Gone below FADE_END_PX (3 px period).
        assert!(at(0.3).is_none());
        // HiDPI doubles the physical period: 3 pt at 2x = 6 px, still partly visible.
        let hidpi = SubstrateTileSet { pixels_per_point: 2.0, ..tiles() };
        assert!(board_substrate_mesh(&fill, &hidpi, Color32::WHITE, Pos2::ZERO, 10.0, 0.3, id).is_some());
        // Invisible tint draws nothing.
        assert!(board_substrate_mesh(&fill, &tiles(), Color32::TRANSPARENT, Pos2::ZERO, 10.0, 2.0, id).is_none());
    }
}
