//! Geometry that is not a quad — a line's stroke, a filled polygon, a
//! trail — appended to the batch as indexed triangles, in the same runs and
//! painter's order as the sprites.

use voltra_render::glam::{Mat3, Vec2};
use voltra_render::Vertex;

use super::{SpriteBatch, Surface};
use crate::sprite::sheets::Sheets;

/// One corner of a [`TriangleMesh`], in its owner's space.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MeshVertex {
    /// Where, before the owner's matrix.
    pub position: Vec2,
    /// Where in the texture.
    pub uv: [f32; 2],
    /// Linear RGBA, multiplied into the texture.
    pub color: [f32; 4],
}

/// Indexed triangles in an owner's space: what a line, a polygon or a trail
/// becomes before it joins the batch.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TriangleMesh {
    /// The corners.
    pub vertices: Vec<MeshVertex>,
    /// Three per triangle, into `vertices`.
    pub indices: Vec<u32>,
}

impl TriangleMesh {
    /// Adds a corner, returning its index.
    pub fn vertex(&mut self, position: Vec2, uv: [f32; 2], color: [f32; 4]) -> u32 {
        self.vertices.push(MeshVertex {
            position,
            uv,
            color,
        });
        (self.vertices.len() - 1) as u32
    }

    /// Adds the triangle `a b c`.
    pub fn triangle(&mut self, a: u32, b: u32, c: u32) {
        self.indices.extend([a, b, c]);
    }

    /// No triangle.
    pub fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }

    /// The box round every corner, `(min, max)`; `None` with no corner.
    pub fn bounds(&self) -> Option<(Vec2, Vec2)> {
        let first = self.vertices.first()?.position;
        Some(self.vertices.iter().fold((first, first), |(lo, hi), v| {
            (lo.min(v.position), hi.max(v.position))
        }))
    }
}

impl SpriteBatch {
    /// Appends `mesh`, placed by `matrix`, drawn with `surface` — extending
    /// the last run when it has the same surface, as a quad does. Left out
    /// whole when its box misses the view `sheets` sees, or when an index
    /// points past its corners or a corner is not finite (nothing half
    /// drawn from bad data).
    pub fn push_triangles(
        &mut self,
        matrix: Mat3,
        mesh: &TriangleMesh,
        surface: Surface,
        sheets: Sheets<'_>,
    ) {
        if mesh.is_empty() || !mesh.indices.len().is_multiple_of(3) {
            return;
        }
        let count = mesh.vertices.len() as u32;
        if mesh.indices.iter().any(|&i| i >= count) {
            log::warn!("a mesh index past its {count} corners; not drawn");
            return;
        }
        let placed: Vec<Vec2> = mesh
            .vertices
            .iter()
            .map(|v| matrix.transform_point2(v.position))
            .collect();
        if !placed.iter().all(|p| p.is_finite()) {
            return;
        }
        let min = placed.iter().copied().fold(Vec2::INFINITY, Vec2::min);
        let max = placed.iter().copied().fold(Vec2::NEG_INFINITY, Vec2::max);
        let view = Sheets {
            visible: self.visible,
            ..sheets
        };
        if !view.sees(min, max) {
            return;
        }
        let base = self.vertices.len() as u32;
        self.vertices.extend(
            mesh.vertices
                .iter()
                .zip(&placed)
                .map(|(v, at)| Vertex::new([at.x, at.y], v.color, v.uv)),
        );
        let start = self.indices.len() as u32;
        self.indices.extend(mesh.indices.iter().map(|i| base + i));
        let end = self.indices.len() as u32;
        self.extend_ranges(surface, voltra_render::Primitives::Triangles(start..end));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn triangle() -> TriangleMesh {
        let mut mesh = TriangleMesh::default();
        let a = mesh.vertex(Vec2::ZERO, [0.0, 0.0], [1.0; 4]);
        let b = mesh.vertex(Vec2::X, [1.0, 0.0], [1.0; 4]);
        let c = mesh.vertex(Vec2::Y, [0.0, 1.0], [1.0; 4]);
        mesh.triangle(a, b, c);
        mesh
    }

    #[test]
    fn triangles_join_the_last_run_and_are_placed() {
        let mut batch = SpriteBatch::default();
        let surface = Surface::textured(None);
        batch.push_triangles(
            Mat3::from_translation(Vec2::new(2.0, 0.0)),
            &triangle(),
            surface,
            Sheets::default(),
        );
        batch.push_triangles(Mat3::IDENTITY, &triangle(), surface, Sheets::default());
        assert_eq!(batch.vertices.len(), 6);
        assert_eq!(batch.indices, vec![0, 1, 2, 3, 4, 5]);
        assert_eq!(batch.ranges.len(), 1);
        assert_eq!(batch.vertices[1].position, [3.0, 0.0]);
    }

    #[test]
    fn bad_meshes_and_meshes_out_of_view_are_left_out() {
        let mut batch = SpriteBatch::default();
        let mut broken = triangle();
        broken.indices[2] = 9;
        let surface = Surface::textured(None);
        batch.push_triangles(Mat3::IDENTITY, &broken, surface, Sheets::default());
        let away = Sheets::default().visible_in(Vec2::splat(10.0), Vec2::splat(11.0));
        batch.visible = away.visible;
        batch.push_triangles(Mat3::IDENTITY, &triangle(), surface, away);
        assert!(batch.vertices.is_empty());
    }
}
