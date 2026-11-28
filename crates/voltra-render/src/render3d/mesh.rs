//! A 3D mesh on the GPU.

use wgpu::util::DeviceExt;

use super::bounds::Aabb3;
use super::mesh_data::{MeshData3D, MeshDataError};

/// A mesh's vertex and `u32` index buffers, and its local bounds for
/// culling. Made once per mesh and drawn by every instance of it.
#[derive(Debug)]
pub struct Mesh3D {
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    index_count: u32,
    bounds: Aabb3,
}

impl Mesh3D {
    /// Uploads `data`, refused when it cannot be drawn
    /// ([`MeshData3D::check`]).
    pub fn new(
        device: &wgpu::Device,
        label: &str,
        data: &MeshData3D,
    ) -> Result<Self, MeshDataError> {
        data.check()?;
        let bounds = data.bounds().ok_or(MeshDataError::Empty)?;
        let index_count =
            u32::try_from(data.indices.len()).map_err(|_| MeshDataError::IndexOutOfRange {
                index: u32::MAX,
                vertices: data.vertices.len(),
            })?;
        let vertices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(label),
            contents: bytemuck::cast_slice(&data.vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let indices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(label),
            contents: bytemuck::cast_slice(&data.indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        Ok(Self {
            vertices,
            indices,
            index_count,
            bounds,
        })
    }

    /// The vertex buffer, of [`Vertex3D`](super::Vertex3D)s.
    pub fn vertices(&self) -> &wgpu::Buffer {
        &self.vertices
    }

    /// The index buffer, `u32`s.
    pub fn indices(&self) -> &wgpu::Buffer {
        &self.indices
    }

    /// How many indices it draws: three per triangle.
    pub fn index_count(&self) -> u32 {
        self.index_count
    }

    /// The box around it in its own space.
    pub fn bounds(&self) -> Aabb3 {
        self.bounds
    }
}
