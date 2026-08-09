//! Line segments with a width measured in pixels.
//!
//! Nothing here knows what the lines are *for*. The editor's translate gizmo is
//! the first caller; the viewport grid and a collision-shape overlay are the
//! reasons this is a pipeline in the render crate rather than a few calls to
//! `egui::Painter` in a panel.
//!
//! ## Why a line is a quad
//!
//! WebGPU has no line width, and neither does wgpu 30 — `PrimitiveState`
//! carries `topology`, `strip_index_format`, `front_face`, `cull_mode`,
//! `unclipped_depth`, `polygon_mode` and `conservative`, and nothing else.
//! `PrimitiveTopology::LineList` therefore draws one-pixel hairlines, which are
//! invisible on a high-DPI display and impossible to hit-test generously.
//!
//! So each segment is uploaded as a quad — four vertices all carrying *both*
//! endpoints — and `shaders/lines.wgsl` gives it its width after projection,
//! perpendicular to the direction the segment runs in on screen. Bevy solved it
//! the same way, by adopting `bevy_polyline` into `bevy_gizmos`.
//!
//! Doing the widening after projection is what makes the width a number of
//! pixels rather than a number of world units: a gizmo has to stay the same
//! size on screen at every zoom, and a grid line has to stay hairline-thin when
//! the camera pulls back far enough to show a thousand of them.

use bytemuck::{Pod, Zeroable};
use glam::Vec2;

use crate::mesh::Mesh;

/// Shortest segment worth drawing, in world units.
///
/// Below this the direction `normalize(b - a)` is numerically meaningless, and
/// a NaN there propagates into every vertex of the quad.
const MIN_LENGTH: f32 = 1e-6;

/// One corner of one segment's quad.
///
/// Both endpoints are repeated on all four corners on purpose: the shader has
/// to project *both* before it knows which way the segment runs on screen, and
/// a vertex shader cannot see its neighbours.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct LineVertex {
    pub a: [f32; 2],
    pub b: [f32; 2],
    /// `.x` picks the endpoint — `0.0` for `a`, `1.0` for `b`. `.y` picks the
    /// side, `-1.0` or `+1.0`, which the shader multiplies by half the width.
    pub corner: [f32; 2],
    /// Width in logical pixels, applied after projection.
    pub width: f32,
    pub color: [f32; 4],
}

impl LineVertex {
    /// Attribute layout matching the `@location` bindings in `lines.wgsl`.
    pub const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: size_of::<Self>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &wgpu::vertex_attr_array![
            0 => Float32x2, 1 => Float32x2, 2 => Float32x2, 3 => Float32, 4 => Float32x4
        ],
    };
}

/// Segments accumulated on the CPU, then uploaded as one mesh.
///
/// Rebuilt every frame by whoever draws an overlay, so [`Self::clear`] keeps
/// the allocation rather than dropping it.
#[derive(Debug, Default)]
pub struct LineBatch {
    vertices: Vec<LineVertex>,
    indices: Vec<u32>,
}

impl LineBatch {
    /// Adds one segment, in world units, `width` pixels thick.
    ///
    /// Silently drops a segment that cannot be drawn — zero length, or a width
    /// that is not positive. Both produce degenerate geometry rather than an
    /// error, and a caller emitting an overlay has nothing useful to do with a
    /// `Result` per segment.
    pub fn push(&mut self, a: Vec2, b: Vec2, width: f32, color: [f32; 4]) {
        if width <= 0.0 || a.distance_squared(b) < MIN_LENGTH * MIN_LENGTH {
            return;
        }

        let base = self.vertices.len() as u32;
        for corner in [[0.0, -1.0], [0.0, 1.0], [1.0, 1.0], [1.0, -1.0]] {
            self.vertices.push(LineVertex {
                a: a.to_array(),
                b: b.to_array(),
                corner,
                width,
                color,
            });
        }

        self.indices
            .extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    /// Empties the batch without giving back its memory.
    pub fn clear(&mut self) {
        self.vertices.clear();
        self.indices.clear();
    }

    /// How many segments are held.
    pub fn len(&self) -> usize {
        self.vertices.len() / 4
    }

    pub fn is_empty(&self) -> bool {
        self.vertices.is_empty()
    }

    /// Uploads the batch, or `None` when there is nothing in it.
    ///
    /// `None` rather than an empty [`Mesh`]: wgpu rejects a zero-sized buffer,
    /// and the passes already treat `None` as "draw nothing".
    pub fn upload(&self, device: &wgpu::Device) -> Option<Mesh> {
        if self.is_empty() {
            return None;
        }
        Some(Mesh::indexed(
            device,
            "lines",
            &self.vertices,
            &self.indices,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WHITE: [f32; 4] = [1.0, 1.0, 1.0, 1.0];

    #[test]
    fn a_segment_becomes_four_vertices_and_six_indices() {
        let mut batch = LineBatch::default();
        batch.push(Vec2::ZERO, Vec2::new(1.0, 0.0), 2.0, WHITE);

        assert_eq!(batch.vertices.len(), 4);
        assert_eq!(batch.indices, vec![0, 1, 2, 0, 2, 3]);
    }

    #[test]
    fn every_vertex_carries_both_endpoints() {
        // The shader needs the whole segment at each corner: it takes the
        // perpendicular of the screen-space direction, which needs both ends
        // after projection, not just the one this vertex sits at.
        let a = Vec2::new(-1.0, 2.0);
        let b = Vec2::new(3.0, 4.0);
        let mut batch = LineBatch::default();
        batch.push(a, b, 1.0, WHITE);

        for vertex in &batch.vertices {
            assert_eq!(vertex.a, [a.x, a.y]);
            assert_eq!(vertex.b, [b.x, b.y]);
        }
    }

    #[test]
    fn the_four_corners_are_both_ends_on_both_sides() {
        let mut batch = LineBatch::default();
        batch.push(Vec2::ZERO, Vec2::new(1.0, 0.0), 2.0, WHITE);

        let corners: Vec<[f32; 2]> = batch.vertices.iter().map(|v| v.corner).collect();
        assert!(corners.contains(&[0.0, -1.0]));
        assert!(corners.contains(&[0.0, 1.0]));
        assert!(corners.contains(&[1.0, 1.0]));
        assert!(corners.contains(&[1.0, -1.0]));
    }

    #[test]
    fn a_zero_length_segment_is_dropped() {
        // Both endpoints equal means the screen-space direction is undefined.
        // Normalising it gives NaN, and a NaN position takes the whole quad
        // with it — a triangle that renders as garbage rather than as nothing.
        let mut batch = LineBatch::default();
        batch.push(Vec2::splat(2.0), Vec2::splat(2.0), 2.0, WHITE);

        assert!(batch.is_empty());
    }

    #[test]
    fn a_segment_shorter_than_the_epsilon_is_dropped() {
        let mut batch = LineBatch::default();
        batch.push(Vec2::ZERO, Vec2::new(1e-9, 0.0), 2.0, WHITE);

        assert!(batch.is_empty());
    }

    #[test]
    fn a_non_positive_width_is_dropped() {
        // A zero-width quad is four coincident vertices: no pixels, and one
        // more degenerate triangle for the rasteriser to chew on every frame.
        let mut batch = LineBatch::default();
        batch.push(Vec2::ZERO, Vec2::X, 0.0, WHITE);
        batch.push(Vec2::ZERO, Vec2::X, -3.0, WHITE);

        assert!(batch.is_empty());
    }

    #[test]
    fn indices_of_the_second_segment_are_offset_by_its_own_base() {
        let mut batch = LineBatch::default();
        batch.push(Vec2::ZERO, Vec2::X, 1.0, WHITE);
        batch.push(Vec2::ZERO, Vec2::Y, 1.0, WHITE);

        assert_eq!(batch.vertices.len(), 8);
        assert_eq!(batch.indices[6..], [4, 5, 6, 4, 6, 7]);
    }

    #[test]
    fn clearing_keeps_the_allocation_and_empties_the_batch() {
        // Rebuilt every frame, so the point of `clear` is to reuse the Vec.
        let mut batch = LineBatch::default();
        batch.push(Vec2::ZERO, Vec2::X, 1.0, WHITE);
        let capacity = batch.vertices.capacity();

        batch.clear();

        assert!(batch.is_empty());
        assert_eq!(batch.vertices.capacity(), capacity);
    }

    #[test]
    fn an_empty_batch_has_no_segments() {
        // `upload` returns None for this, not a zero-sized buffer: wgpu
        // rejects those, and the passes already treat None as "draw nothing".
        assert!(LineBatch::default().is_empty());
        assert_eq!(LineBatch::default().len(), 0);
    }
}
