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

/// The viewport's pixel size, on the GPU, with the bind group that reaches it.
///
/// Group 1 — the slot the sprite pipeline gives its texture — so that group 0
/// is the camera in every pipeline this engine has. Shaped like
/// [`CameraBinding`](crate::camera::CameraBinding), for the same reason: a
/// uniform, its buffer and its bind group belong to one another, and handing
/// them out separately is how a bind group ends up built against a layout the
/// pipeline was not.
pub struct ViewportBinding {
    buffer: wgpu::Buffer,
    layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
}

impl ViewportBinding {
    pub fn new(device: &wgpu::Device) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("viewport-layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        // A `vec4` rather than a `vec2`: a uniform buffer binding is aligned to
        // 16 bytes, and two unused floats are cheaper than a padding field
        // nobody remembers the reason for.
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("viewport-uniform"),
            size: size_of::<[f32; 4]>() as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("viewport-bind-group"),
            layout: &layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        });

        Self {
            buffer,
            layout,
            bind_group,
        }
    }

    /// Writes the size the next line pass widens against.
    ///
    /// Both dimensions are clamped to at least one. A minimised window reports
    /// zero, and the shader divides by half of this — a NaN there goes through
    /// every vertex of every line.
    pub fn upload(&self, queue: &wgpu::Queue, width: u32, height: u32) {
        let size = [width.max(1) as f32, height.max(1) as f32, 0.0, 0.0];
        queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(&size));
    }

    /// The layout the line pipeline must be built against.
    pub fn layout(&self) -> &wgpu::BindGroupLayout {
        &self.layout
    }

    pub fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group
    }
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

    /// The vertices pushed so far, four per segment.
    ///
    /// For a caller that wants to look at what an overlay drew without a GPU
    /// to draw it on — which is the only way a colour or a position picked by
    /// a debug pass can be asserted at all.
    pub fn vertices(&self) -> &[LineVertex] {
        &self.vertices
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
