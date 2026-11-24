//! Vertex, index and quad-instance buffers.

use std::ops::Range;

use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

pub use crate::quad::QuadInstance;
pub use buffers::MeshBuffers;

mod buffers;

/// A single vertex of 2D geometry.
///
/// `repr(C)` is not cosmetic: the GPU reads this struct by byte offset, so the
/// field order must be exactly what [`Vertex::LAYOUT`] declares. Rust makes no
/// layout promise without it.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct Vertex {
    /// Where it is, in the space of the camera drawing it.
    pub position: [f32; 2],
    /// Multiplied with the texture, alpha included: an alpha below one
    /// fades the texel, which is how a sprite turns translucent and a
    /// particle fades out.
    pub color: [f32; 4],
    /// Texture coordinates. `(0, 0)` is the top-left of the image, matching
    /// how the pixels are laid out in the file.
    pub uv: [f32; 2],
}

impl Vertex {
    /// Attribute layout matching the `@location` bindings in the WGSL.
    pub const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: size_of::<Self>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x4, 2 => Float32x2],
    };

    /// A vertex from its position, colour and texture coordinates.
    pub const fn new(position: [f32; 2], color: [f32; 4], uv: [f32; 2]) -> Self {
        Self {
            position,
            color,
            uv,
        }
    }
}

/// What one draw of a [`Mesh`] covers: a range of its triangle indices, or
/// a range of its quad instances.
///
/// One kind a draw, because the two go through different pipelines (the
/// quads' vertex stage makes their corners; see
/// [`SpritePipeline`](crate::pipeline::SpritePipeline)).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Primitives {
    /// Indices into the mesh's index buffer, three a triangle.
    Triangles(Range<u32>),
    /// Instances of the mesh's quad buffer, one a quad.
    Quads(Range<u32>),
}

impl Primitives {
    /// Whether the range covers nothing.
    pub fn is_empty(&self) -> bool {
        match self {
            Self::Triangles(range) | Self::Quads(range) => range.is_empty(),
        }
    }

    /// Whether these are quads.
    pub fn is_quads(&self) -> bool {
        matches!(self, Self::Quads(_))
    }
}

/// Geometry uploaded to the GPU: triangles, drawn directly or through
/// indices, and quads drawn as instances ([`QuadInstance`]).
pub struct Mesh {
    /// `None` when the mesh is quads alone.
    vertices: Option<wgpu::Buffer>,
    /// `None` means a non-indexed draw.
    indices: Option<wgpu::Buffer>,
    /// Vertex count for a direct draw, index count for an indexed one.
    count: u32,
    /// `None` when the mesh has no quads.
    quads: Option<wgpu::Buffer>,
    /// Quads in `quads`.
    quad_count: u32,
}

/// Destroys the buffers rather than leaving them to the drop: on the web a
/// dropped buffer's memory waits for JavaScript's collector, and meshes are
/// built afresh every frame (see `frame_buffers.rs`). A mesh owns its
/// buffers alone, and is dropped after the frame that drew it is submitted.
impl Drop for Mesh {
    fn drop(&mut self) {
        for buffer in [&self.vertices, &self.indices, &self.quads]
            .into_iter()
            .flatten()
        {
            buffer.destroy();
        }
    }
}

impl Mesh {
    /// A mesh of nothing, which [`MeshBuffers`] fills.
    fn empty() -> Self {
        Self {
            vertices: None,
            indices: None,
            count: 0,
            quads: None,
            quad_count: 0,
        }
    }

    /// Uploads vertices for a direct (non-indexed) draw.
    pub fn new<V: Pod>(device: &wgpu::Device, label: &str, vertices: &[V]) -> Self {
        Self {
            vertices: Some(init(device, label, vertices, wgpu::BufferUsages::VERTEX)),
            indices: None,
            count: vertices.len() as u32,
            quads: None,
            quad_count: 0,
        }
    }

    /// Uploads vertices plus a `u32` index buffer.
    ///
    /// `u32` rather than `u16`: sprite batches are split by texture into
    /// ranges over one mesh, so a batch is not free to stop at 65 536 vertices
    /// the way a single-object mesh would be, and a wrapped index draws the
    /// wrong geometry without any validation error. Bevy binds its sprite
    /// indices as `Uint32` for the same reason. The cost is four bytes per
    /// index instead of two.
    ///
    /// Generic over the vertex type because a `Mesh` is buffers and a count:
    /// what the bytes mean is the pipeline's business, declared by whichever
    /// [`wgpu::VertexBufferLayout`] it was built with. [`Vertex`] and
    /// `LineVertex` are two such meanings, and there is no third structure to
    /// justify.
    pub fn indexed<V: Pod>(
        device: &wgpu::Device,
        label: &str,
        vertices: &[V],
        indices: &[u32],
    ) -> Self {
        Self::batch(device, label, vertices, indices, &[])
    }

    /// Uploads a sprite batch's geometry: indexed triangles and quad
    /// instances, either of which may be empty (and is then not made).
    pub fn batch<V: Pod>(
        device: &wgpu::Device,
        label: &str,
        vertices: &[V],
        indices: &[u32],
        quads: &[QuadInstance],
    ) -> Self {
        let triangles = !indices.is_empty();
        Self {
            vertices: triangles.then(|| init(device, label, vertices, wgpu::BufferUsages::VERTEX)),
            indices: triangles.then(|| init(device, label, indices, wgpu::BufferUsages::INDEX)),
            count: indices.len() as u32,
            quads: (!quads.is_empty())
                .then(|| init(device, label, quads, wgpu::BufferUsages::VERTEX)),
            quad_count: quads.len() as u32,
        }
    }

    /// Binds this mesh's triangles and issues their draw call into an open
    /// pass. Quads are drawn through [`draw_quads`](Self::draw_quads), with
    /// a pipeline of their own.
    pub fn draw(&self, pass: &mut wgpu::RenderPass<'_>) {
        let Some(vertices) = &self.vertices else {
            return;
        };
        pass.set_vertex_buffer(0, vertices.slice(..));
        match &self.indices {
            Some(indices) => {
                pass.set_index_buffer(indices.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..self.count, 0, 0..1);
            }
            None => pass.draw(0..self.count, 0..1),
        }
    }

    /// Binds this mesh and issues an indexed draw over `indices` only.
    ///
    /// Lets one uploaded buffer serve several draw calls: a batch that groups
    /// its geometry into contiguous per-texture runs draws each run through
    /// its own call here without re-uploading anything. Panics if this mesh
    /// has no index buffer — callers pick ranges from data they built
    /// themselves, so a mismatch here is a caller bug, not a runtime case to
    /// recover from.
    pub fn draw_range(&self, pass: &mut wgpu::RenderPass<'_>, indices: Range<u32>) {
        let (Some(vertices), Some(index_buffer)) = (&self.vertices, &self.indices) else {
            panic!("draw_range requires an indexed mesh");
        };
        pass.set_vertex_buffer(0, vertices.slice(..));
        pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        pass.draw_indexed(indices, 0, 0..1);
    }

    /// Binds this mesh's quads and draws the instances in `quads`, six
    /// vertices each, through a pipeline whose vertex stage makes the
    /// corners ([`QuadInstance::LAYOUT`] in slot 0). Panics if the mesh has
    /// no quads, for the reason [`draw_range`](Self::draw_range) does.
    ///
    /// The range is the draw's first instance and count. WebGL2 has no base
    /// instance; wgpu's GL backend offsets the instance buffer's binding
    /// instead, so the same call serves every backend.
    pub fn draw_quads(&self, pass: &mut wgpu::RenderPass<'_>, quads: Range<u32>) {
        let buffer = self
            .quads
            .as_ref()
            .expect("draw_quads requires a mesh with quads");
        pass.set_vertex_buffer(0, buffer.slice(..));
        pass.draw(0..QuadInstance::VERTICES, quads);
    }

    /// [`draw_range`](Self::draw_range) or [`draw_quads`](Self::draw_quads),
    /// as `primitives` says.
    pub fn draw_primitives(&self, pass: &mut wgpu::RenderPass<'_>, primitives: &Primitives) {
        match primitives {
            Primitives::Triangles(indices) => self.draw_range(pass, indices.clone()),
            Primitives::Quads(quads) => self.draw_quads(pass, quads.clone()),
        }
    }

    /// Vertex count for a direct draw, index count for an indexed one.
    pub fn count(&self) -> u32 {
        self.count
    }

    /// Quad instances the mesh holds.
    pub fn quad_count(&self) -> u32 {
        self.quad_count
    }

    /// Whether the mesh draws through an index buffer.
    pub fn is_indexed(&self) -> bool {
        self.indices.is_some()
    }
}

/// A buffer of `usage` holding `contents`.
fn init<T: Pod>(
    device: &wgpu::Device,
    label: &str,
    contents: &[T],
    usage: wgpu::BufferUsages,
) -> wgpu::Buffer {
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(label),
        contents: bytemuck::cast_slice(contents),
        usage,
    })
}

/// The built-in triangle, wound counter-clockwise in clip space.
pub const TRIANGLE: [Vertex; 3] = [
    Vertex::new([0.0, 0.5], [1.0, 0.0, 0.0, 1.0], [0.5, 0.0]),
    Vertex::new([-0.5, -0.5], [0.0, 1.0, 0.0, 1.0], [0.0, 1.0]),
    Vertex::new([0.5, -0.5], [0.0, 0.0, 1.0, 1.0], [1.0, 1.0]),
];

/// A full-clip-space quad as four vertices, drawn with [`QUAD_INDICES`].
///
/// V runs opposite to Y: clip space points up, image rows go down.
pub const QUAD: [Vertex; 4] = [
    Vertex::new([-1.0, 1.0], [1.0, 0.0, 0.0, 1.0], [0.0, 0.0]),
    Vertex::new([-1.0, -1.0], [0.0, 1.0, 0.0, 1.0], [0.0, 1.0]),
    Vertex::new([1.0, -1.0], [0.0, 0.0, 1.0, 1.0], [1.0, 1.0]),
    Vertex::new([1.0, 1.0], [1.0, 1.0, 0.0, 1.0], [1.0, 0.0]),
];

/// Two triangles sharing the diagonal, so four vertices cover a quad.
pub const QUAD_INDICES: [u32; 6] = [0, 1, 2, 0, 2, 3];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_stride_matches_the_struct() {
        assert_eq!(Vertex::LAYOUT.array_stride as usize, size_of::<Vertex>());
        assert_eq!(size_of::<Vertex>(), 8 * size_of::<f32>());
    }

    #[test]
    fn attributes_are_contiguous_and_ordered() {
        let attrs = Vertex::LAYOUT.attributes;
        assert_eq!(attrs.len(), 3);
        let f32_size = size_of::<f32>() as wgpu::BufferAddress;

        assert_eq!(attrs[0].shader_location, 0);
        assert_eq!(attrs[0].offset, 0);
        assert_eq!(attrs[0].format, wgpu::VertexFormat::Float32x2);

        assert_eq!(attrs[1].shader_location, 1);
        // Colour starts right after the two position floats.
        assert_eq!(attrs[1].offset, 2 * f32_size);
        assert_eq!(attrs[1].format, wgpu::VertexFormat::Float32x4);

        assert_eq!(attrs[2].shader_location, 2);
        assert_eq!(attrs[2].offset, 6 * f32_size);
        assert_eq!(attrs[2].format, wgpu::VertexFormat::Float32x2);
    }

    #[test]
    fn quad_uvs_cover_the_whole_image() {
        let us: Vec<f32> = QUAD.iter().map(|v| v.uv[0]).collect();
        let vs: Vec<f32> = QUAD.iter().map(|v| v.uv[1]).collect();
        assert!(us.contains(&0.0) && us.contains(&1.0));
        assert!(vs.contains(&0.0) && vs.contains(&1.0));

        // V is flipped against Y: the top of the quad samples the top row of
        // the image, which is v = 0.
        let top = QUAD.iter().find(|v| v.position[1] > 0.0).unwrap();
        assert_eq!(top.uv[1], 0.0);
    }

    #[test]
    fn quad_indices_describe_two_triangles() {
        assert_eq!(QUAD_INDICES.len() % 3, 0);
        assert_eq!(QUAD_INDICES.len() / 3, 2);
        // Every index must address a real vertex.
        assert!(QUAD_INDICES.iter().all(|&i| (i as usize) < QUAD.len()));
    }
}
