//! Fast 2D geometric renderer for BoardView using batched meshes, direct circle tessellation, and lyon.

use std::f32::consts::TAU;
use egui::epaint::{Mesh, Vertex};
use egui::{Color32, Pos2};
use lyon_tessellation::geometry_builder::{BuffersBuilder, VertexBuffers};
use lyon_tessellation::math::Point as LPoint;
use lyon_tessellation::path::Path;
use lyon_tessellation::{FillOptions, FillTessellator, StrokeOptions, StrokeTessellator};

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
