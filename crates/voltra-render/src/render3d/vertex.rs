//! A 3D mesh's vertex, as the GPU reads it.

use bytemuck::{Pod, Zeroable};

/// One vertex of a [`Mesh3D`](super::Mesh3D): 48 bytes, four attributes.
///
/// Every field a surface will read lands now, so the layout is fixed once
/// against WebGL2's budget (see [`Instance3D`](super::Instance3D)): the
/// unlit pass reads `position` only; normals, UVs and tangents are 137's
/// lit materials and normal maps. glTF's layout, and Bevy's.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct Vertex3D {
    /// Where it is, in the mesh's own space, metres.
    pub position: [f32; 3],
    /// The surface's outward normal there, unit length.
    pub normal: [f32; 3],
    /// Texture coordinates, `(0, 0)` the image's top-left.
    pub uv: [f32; 2],
    /// The direction of increasing U along the surface, unit length, and in
    /// `w` the handedness (±1) the bitangent is `cross(normal, tangent) * w`
    /// with — glTF's convention.
    pub tangent: [f32; 4],
}

impl Vertex3D {
    /// The shader locations, 0 to 3.
    pub const ATTRIBUTES: [wgpu::VertexAttribute; 4] = wgpu::vertex_attr_array![
        0 => Float32x3,
        1 => Float32x3,
        2 => Float32x2,
        3 => Float32x4,
    ];

    /// The vertex buffer's layout: buffer slot 0, stepped per vertex.
    pub fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBUTES,
        }
    }
}
