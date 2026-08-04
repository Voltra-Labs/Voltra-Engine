//! Vertex and index buffers.

use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

/// A single vertex of 2D geometry.
///
/// `repr(C)` is not cosmetic: the GPU reads this struct by byte offset, so the
/// field order must be exactly what [`Vertex::LAYOUT`] declares. Rust makes no
/// layout promise without it.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct Vertex {
    pub position: [f32; 2],
    pub color: [f32; 3],
    /// Texture coordinates. `(0, 0)` is the top-left of the image, matching
    /// how the pixels are laid out in the file.
    pub uv: [f32; 2],
}

impl Vertex {
    /// Attribute layout matching the `@location` bindings in the WGSL.
    pub const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: size_of::<Self>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x3, 2 => Float32x2],
    };

    pub const fn new(position: [f32; 2], color: [f32; 3], uv: [f32; 2]) -> Self {
        Self {
            position,
            color,
            uv,
        }
    }
}

/// Geometry uploaded to the GPU, drawn either directly or through indices.
pub struct Mesh {
    vertices: wgpu::Buffer,
    /// `None` means a non-indexed draw.
    indices: Option<wgpu::Buffer>,
    /// Vertex count for a direct draw, index count for an indexed one.
    count: u32,
}

impl Mesh {
    /// Uploads vertices for a direct (non-indexed) draw.
    pub fn new(device: &wgpu::Device, label: &str, vertices: &[Vertex]) -> Self {
        Self {
            vertices: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(label),
                contents: bytemuck::cast_slice(vertices),
                usage: wgpu::BufferUsages::VERTEX,
            }),
            indices: None,
            count: vertices.len() as u32,
        }
    }

    /// Uploads vertices plus a `u16` index buffer.
    ///
    /// `u16` covers 65k vertices per mesh, which is far past the point where
    /// the mesh should have been split anyway. It halves index bandwidth
    /// against `u32`.
    pub fn indexed(
        device: &wgpu::Device,
        label: &str,
        vertices: &[Vertex],
        indices: &[u16],
    ) -> Self {
        Self {
            vertices: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(label),
                contents: bytemuck::cast_slice(vertices),
                usage: wgpu::BufferUsages::VERTEX,
            }),
            indices: Some(
                device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some(label),
                    contents: bytemuck::cast_slice(indices),
                    usage: wgpu::BufferUsages::INDEX,
                }),
            ),
            count: indices.len() as u32,
        }
    }

    /// Binds this mesh and issues its draw call into an open pass.
    pub fn draw(&self, pass: &mut wgpu::RenderPass<'_>) {
        pass.set_vertex_buffer(0, self.vertices.slice(..));
        match &self.indices {
            Some(indices) => {
                pass.set_index_buffer(indices.slice(..), wgpu::IndexFormat::Uint16);
                pass.draw_indexed(0..self.count, 0, 0..1);
            }
            None => pass.draw(0..self.count, 0..1),
        }
    }

    /// Vertex count for a direct draw, index count for an indexed one.
    pub fn count(&self) -> u32 {
        self.count
    }

    pub fn is_indexed(&self) -> bool {
        self.indices.is_some()
    }
}

/// The built-in triangle, wound counter-clockwise in clip space.
pub const TRIANGLE: [Vertex; 3] = [
    Vertex::new([0.0, 0.5], [1.0, 0.0, 0.0], [0.5, 0.0]),
    Vertex::new([-0.5, -0.5], [0.0, 1.0, 0.0], [0.0, 1.0]),
    Vertex::new([0.5, -0.5], [0.0, 0.0, 1.0], [1.0, 1.0]),
];

/// A full-clip-space quad as four vertices, drawn with [`QUAD_INDICES`].
///
/// V runs opposite to Y: clip space points up, image rows go down.
pub const QUAD: [Vertex; 4] = [
    Vertex::new([-1.0, 1.0], [1.0, 0.0, 0.0], [0.0, 0.0]),
    Vertex::new([-1.0, -1.0], [0.0, 1.0, 0.0], [0.0, 1.0]),
    Vertex::new([1.0, -1.0], [0.0, 0.0, 1.0], [1.0, 1.0]),
    Vertex::new([1.0, 1.0], [1.0, 1.0, 0.0], [1.0, 0.0]),
];

/// Two triangles sharing the diagonal, so four vertices cover a quad.
pub const QUAD_INDICES: [u16; 6] = [0, 1, 2, 0, 2, 3];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_stride_matches_the_struct() {
        assert_eq!(Vertex::LAYOUT.array_stride as usize, size_of::<Vertex>());
        assert_eq!(size_of::<Vertex>(), 7 * size_of::<f32>());
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
        assert_eq!(attrs[1].format, wgpu::VertexFormat::Float32x3);

        assert_eq!(attrs[2].shader_location, 2);
        assert_eq!(attrs[2].offset, 5 * f32_size);
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
