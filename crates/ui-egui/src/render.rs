//! Fast 2D geometric renderer for BoardView using batched meshes, direct circle tessellation, and lyon.

use std::f32::consts::TAU;
use egui::epaint::{Mesh, Vertex};
use egui::{Color32, Pos2};
use lyon_tessellation::geometry_builder::{BuffersBuilder, VertexBuffers};
use lyon_tessellation::math::Point as LPoint;
use lyon_tessellation::path::Path;
use lyon_tessellation::{FillOptions, FillTessellator, StrokeOptions, StrokeTessellator};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LodLevel {
    /// Full circular fan tessellation (r_screen >= 1.5 px)
    FullCircle,
    /// Simplified quad primitive (0.75 px <= r_screen < 1.5 px)
    SimplifiedQuad,
    /// Fast 1-pixel quad (r_screen < 0.75 px on dense nets, or < 1.0 px)
    SubpixelPoint,
    /// Culled (radius <= 0 or negligible)
    Culled,
}

impl LodLevel {
    #[inline]
    pub fn select(raw_screen_r: f32, is_high_density_net: bool) -> Self {
        if raw_screen_r <= 0.0 {
            LodLevel::Culled
        } else if is_high_density_net {
            if raw_screen_r < 0.75 {
                LodLevel::SubpixelPoint
            } else if raw_screen_r < 1.5 {
                LodLevel::SimplifiedQuad
            } else {
                LodLevel::FullCircle
            }
        } else {
            if raw_screen_r < 1.0 {
                LodLevel::SubpixelPoint
            } else if raw_screen_r < 2.0 {
                LodLevel::SimplifiedQuad
            } else {
                LodLevel::FullCircle
            }
        }
    }
}

pub struct Renderer {
    fill_tess: FillTessellator,
    stroke_tess: StrokeTessellator,
}

impl Default for Renderer {
    fn default() -> Self {
        Self {
            fill_tess: FillTessellator::new(),
            stroke_tess: StrokeTessellator::new(),
        }
    }
}

impl Renderer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends a circle, quad, or sub-pixel point primitive to `mesh` according to LOD.
    #[inline]
    pub fn append_lod_circle_to_mesh(
        mesh: &mut Mesh,
        center: Pos2,
        raw_screen_r: f32,
        is_high_density_net: bool,
        color: Color32,
    ) -> LodLevel {
        let lod = LodLevel::select(raw_screen_r, is_high_density_net);
        match lod {
            LodLevel::Culled => {}
            LodLevel::SubpixelPoint => {
                Self::append_rect_filled_to_mesh(
                    mesh,
                    Pos2::new(center.x - 0.5, center.y - 0.5),
                    Pos2::new(center.x + 0.5, center.y + 0.5),
                    color,
                );
            }
            LodLevel::SimplifiedQuad => {
                let h = raw_screen_r.max(0.75);
                Self::append_rect_filled_to_mesh(
                    mesh,
                    Pos2::new(center.x - h, center.y - h),
                    Pos2::new(center.x + h, center.y + h),
                    color,
                );
            }
            LodLevel::FullCircle => {
                let screen_r = raw_screen_r.clamp(1.5, 40.0);
                Self::append_circle_to_mesh(mesh, center, screen_r, color);
            }
        }
        lod
    }

    /// Appends a direct regular polygon circle to an `egui::Mesh` with no path building or heap allocations.
    /// Uses adaptive segments: `clamp(radius.round(), 6, 24)`.
    #[inline]
    pub fn append_circle_to_mesh(mesh: &mut Mesh, center: Pos2, radius: f32, color: Color32) {
        if radius <= 0.0 {
            return;
        }

        let segments = (radius.round() as usize).clamp(6, 24);

        let start_idx = mesh.vertices.len() as u32;
        mesh.vertices.reserve(segments + 1);
        mesh.indices.reserve(segments * 3);

        // Center vertex
        mesh.vertices.push(Vertex {
            pos: center,
            uv: Pos2::ZERO,
            color,
        });

        // Perimeter vertices
        let step = TAU / segments as f32;
        for i in 0..segments {
            let angle = i as f32 * step;
            let p = Pos2::new(
                center.x + radius * angle.cos(),
                center.y + radius * angle.sin(),
            );
            mesh.vertices.push(Vertex {
                pos: p,
                uv: Pos2::ZERO,
                color,
            });
        }

        // Triangle fan
        for i in 0..segments {
            let next_i = (i + 1) % segments;
            mesh.indices.push(start_idx);
            mesh.indices.push(start_idx + 1 + i as u32);
            mesh.indices.push(start_idx + 1 + next_i as u32);
        }
    }

    /// Appends a stroked circle (annulus / ring) directly to `mesh` with hollow interior.
    #[inline]
    pub fn append_circle_outline_to_mesh(
        mesh: &mut Mesh,
        center: Pos2,
        radius: f32,
        stroke_width: f32,
        color: Color32,
    ) {
        if radius <= 0.0 || stroke_width <= 0.0 {
            return;
        }

        let r_outer = radius + stroke_width * 0.5;
        let r_inner = (radius - stroke_width * 0.5).max(0.0);
        let segments = (radius.round() as usize).clamp(8, 32);

        let start_idx = mesh.vertices.len() as u32;
        mesh.vertices.reserve(segments * 2);
        mesh.indices.reserve(segments * 6);

        let step = TAU / segments as f32;
        for i in 0..segments {
            let angle = i as f32 * step;
            let (sin_a, cos_a) = angle.sin_cos();

            // Inner vertex
            mesh.vertices.push(Vertex {
                pos: Pos2::new(center.x + r_inner * cos_a, center.y + r_inner * sin_a),
                uv: Pos2::ZERO,
                color,
            });
            // Outer vertex
            mesh.vertices.push(Vertex {
                pos: Pos2::new(center.x + r_outer * cos_a, center.y + r_outer * sin_a),
                uv: Pos2::ZERO,
                color,
            });
        }

        for i in 0..segments {
            let next_i = (i + 1) % segments;
            let inner_curr = start_idx + (i as u32 * 2);
            let outer_curr = inner_curr + 1;
            let inner_next = start_idx + (next_i as u32 * 2);
            let outer_next = inner_next + 1;

            // Quad composed of two triangles
            mesh.indices.push(inner_curr);
            mesh.indices.push(outer_curr);
            mesh.indices.push(outer_next);

            mesh.indices.push(inner_curr);
            mesh.indices.push(outer_next);
            mesh.indices.push(inner_next);
        }
    }

    /// Appends a fast 4-vertex axis-aligned filled rectangle (quad) directly to `mesh`.
    #[inline]
    pub fn append_rect_filled_to_mesh(
        mesh: &mut Mesh,
        min: Pos2,
        max: Pos2,
        color: Color32,
    ) {
        let start_idx = mesh.vertices.len() as u32;
        mesh.vertices.reserve(4);
        mesh.indices.reserve(6);

        mesh.vertices.push(Vertex {
            pos: min,
            uv: Pos2::ZERO,
            color,
        });
        mesh.vertices.push(Vertex {
            pos: Pos2::new(max.x, min.y),
            uv: Pos2::ZERO,
            color,
        });
        mesh.vertices.push(Vertex {
            pos: max,
            uv: Pos2::ZERO,
            color,
        });
        mesh.vertices.push(Vertex {
            pos: Pos2::new(min.x, max.y),
            uv: Pos2::ZERO,
            color,
        });

        mesh.indices.push(start_idx);
        mesh.indices.push(start_idx + 1);
        mesh.indices.push(start_idx + 2);

        mesh.indices.push(start_idx);
        mesh.indices.push(start_idx + 2);
        mesh.indices.push(start_idx + 3);
    }

    /// Appends a stroked rectangle (as 4 thick line segments or 8 triangles) directly to `mesh`.
    #[inline]
    pub fn append_rect_outline_to_mesh(
        mesh: &mut Mesh,
        p1: Pos2, // min_x, min_y
        p2: Pos2, // max_x, min_y
        p3: Pos2, // max_x, max_y
        p4: Pos2, // min_x, max_y
        half_w: f32,
        color: Color32,
    ) {
        let edges = [(p1, p2), (p2, p3), (p3, p4), (p4, p1)];
        for (a, b) in edges {
            Self::append_line_segment_to_mesh(mesh, a, b, half_w, color);
        }
    }

    /// Appends a thick line segment between two points to `mesh`.
    #[inline]
    pub fn append_line_segment_to_mesh(
        mesh: &mut Mesh,
        p1: Pos2,
        p2: Pos2,
        half_w: f32,
        color: Color32,
    ) {
        let dx = p2.x - p1.x;
        let dy = p2.y - p1.y;
        let len = (dx * dx + dy * dy).sqrt();
        if len < 1e-4 {
            return;
        }

        // Perpendicular unit vector scaled by half_w
        let nx = -dy / len * half_w;
        let ny = dx / len * half_w;

        let start_idx = mesh.vertices.len() as u32;
        mesh.vertices.reserve(4);
        mesh.indices.reserve(6);

        mesh.vertices.push(Vertex {
            pos: Pos2::new(p1.x + nx, p1.y + ny),
            uv: Pos2::ZERO,
            color,
        });
        mesh.vertices.push(Vertex {
            pos: Pos2::new(p1.x - nx, p1.y - ny),
            uv: Pos2::ZERO,
            color,
        });
        mesh.vertices.push(Vertex {
            pos: Pos2::new(p2.x - nx, p2.y - ny),
            uv: Pos2::ZERO,
            color,
        });
        mesh.vertices.push(Vertex {
            pos: Pos2::new(p2.x + nx, p2.y + ny),
            uv: Pos2::ZERO,
            color,
        });

        mesh.indices.push(start_idx);
        mesh.indices.push(start_idx + 1);
        mesh.indices.push(start_idx + 2);

        mesh.indices.push(start_idx);
        mesh.indices.push(start_idx + 2);
        mesh.indices.push(start_idx + 3);
    }

    /// Appends multiple line segments into `mesh` using pre-allocated batching.
    pub fn append_lines_to_mesh(
        mesh: &mut Mesh,
        segments: &[(Pos2, Pos2)],
        half_w: f32,
        color: Color32,
    ) {
        if segments.is_empty() {
            return;
        }
        mesh.vertices.reserve(segments.len() * 4);
        mesh.indices.reserve(segments.len() * 6);
        for &(p1, p2) in segments {
            Self::append_line_segment_to_mesh(mesh, p1, p2, half_w, color);
        }
    }

    /// Converts a lyon VertexBuffers into an egui::Mesh.
    pub fn buffers_to_egui_mesh(buffers: VertexBuffers<LPoint, u32>, color: Color32) -> Mesh {
        let mut mesh = Mesh::default();
        mesh.vertices.reserve(buffers.vertices.len());
        for v in buffers.vertices {
            mesh.vertices.push(Vertex {
                pos: Pos2::new(v.x, v.y),
                uv: Pos2::ZERO,
                color,
            });
        }
        mesh.indices = buffers.indices;
        mesh
    }

    /// Tessellates a filled circle (e.g., pin, via).
    #[allow(dead_code)]
    pub fn fill_circle(&mut self, center: Pos2, radius: f32, color: Color32) -> Mesh {
        let mut mesh = Mesh::default();
        Self::append_circle_to_mesh(&mut mesh, center, radius, color);
        mesh
    }

    /// Tessellates a stroked polygon (e.g., board outline).
    pub fn stroke_polygon(
        &mut self,
        points: &[Pos2],
        width: f32,
        color: Color32,
        closed: bool,
    ) -> Mesh {
        let mut buffers: VertexBuffers<LPoint, u32> = VertexBuffers::new();
        if points.len() < 2 {
            return Mesh::default();
        }

        let mut builder = Path::builder();
        builder.begin(LPoint::new(points[0].x, points[0].y));
        for p in &points[1..] {
            builder.line_to(LPoint::new(p.x, p.y));
        }
        if closed {
            builder.close();
        } else {
            builder.end(false);
        }
        let path = builder.build();

        let options = StrokeOptions::tolerance(0.2).with_line_width(width);
        self.stroke_tess
            .tessellate_path(
                &path,
                &options,
                &mut BuffersBuilder::new(&mut buffers, |vertex: lyon_tessellation::StrokeVertex| {
                    vertex.position()
                }),
            )
            .ok();

        Self::buffers_to_egui_mesh(buffers, color)
    }

    /// Tessellates a filled polygon (e.g., board body).
    pub fn fill_polygon(&mut self, points: &[Pos2], color: Color32) -> Mesh {
        let mut buffers: VertexBuffers<LPoint, u32> = VertexBuffers::new();
        if points.len() < 3 {
            return Mesh::default();
        }

        let mut builder = Path::builder();
        builder.begin(LPoint::new(points[0].x, points[0].y));
        for p in &points[1..] {
            builder.line_to(LPoint::new(p.x, p.y));
        }
        builder.close();
        let path = builder.build();

        let options = FillOptions::tolerance(0.2);
        self.fill_tess
            .tessellate_path(
                &path,
                &options,
                &mut BuffersBuilder::new(&mut buffers, |vertex: lyon_tessellation::FillVertex| {
                    vertex.position()
                }),
            )
            .ok();

        Self::buffers_to_egui_mesh(buffers, color)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lod_decimation_threshold() {
        let mut mesh_dense_subpixel = Mesh::default();
        let center = Pos2::new(10.0, 10.0);
        let color = Color32::WHITE;

        // Subpixel element on dense net (> 500 nodes) with r_screen < 0.75 px
        let lod = Renderer::append_lod_circle_to_mesh(
            &mut mesh_dense_subpixel,
            center,
            0.5,
            true,
            color,
        );
        assert_eq!(lod, LodLevel::SubpixelPoint);
        assert_eq!(mesh_dense_subpixel.vertices.len(), 4, "Subpixel LOD must use a single 4-vertex quad");
        assert_eq!(mesh_dense_subpixel.indices.len(), 6);

        // Intermediate element with 0.75 <= r_screen < 1.5 px
        let mut mesh_dense_mid = Mesh::default();
        let lod_mid = Renderer::append_lod_circle_to_mesh(
            &mut mesh_dense_mid,
            center,
            1.0,
            true,
            color,
        );
        assert_eq!(lod_mid, LodLevel::SimplifiedQuad);
        assert_eq!(mesh_dense_mid.vertices.len(), 4, "Intermediate LOD must use simplified quad");

        // Full circle with r_screen >= 1.5 px
        let mut mesh_dense_full = Mesh::default();
        let lod_full = Renderer::append_lod_circle_to_mesh(
            &mut mesh_dense_full,
            center,
            4.0,
            true,
            color,
        );
        assert_eq!(lod_full, LodLevel::FullCircle);
        assert!(mesh_dense_full.vertices.len() >= 7, "Full circle must tessellate fan vertices");

        // Vertex count reduction: quad has 4 vertices vs 7-25 for circle
        assert!(mesh_dense_subpixel.vertices.len() < mesh_dense_full.vertices.len());
    }

    #[test]
    fn test_circle_outline_tessellation() {
        let mut mesh = Mesh::default();
        let center = Pos2::new(50.0, 50.0);
        let color = Color32::RED;

        Renderer::append_circle_outline_to_mesh(&mut mesh, center, 10.0, 2.0, color);

        // Ring with radius 10 clamped segments (10 segments)
        // 10 inner vertices + 10 outer vertices = 20 vertices
        // 10 quads * 6 indices = 60 indices
        assert_eq!(mesh.vertices.len(), 20);
        assert_eq!(mesh.indices.len(), 60);

        // Zero or negative radius does not append
        let mut empty_mesh = Mesh::default();
        Renderer::append_circle_outline_to_mesh(&mut empty_mesh, center, 0.0, 2.0, color);
        assert!(empty_mesh.is_empty());
    }
}

