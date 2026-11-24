//! One skinned mesh on the GPU: its vertices and triangles, and the uniform
//! block of bone matrices its vertex shader blends.

use bytemuck::{Pod, Zeroable};
use glam::Mat3;
use wgpu::util::DeviceExt;

use super::pipeline::SkinPipeline;
use super::{MAX_BONES, RIGID_SLOT};

/// A vertex of a skinned mesh: where it is in the mesh's bind pose, where
/// in the texture, and which bones hold it how much.
///
/// `#[repr(C)]`, the order `skinned.wgsl`'s `VertexInput` reads.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct SkinVertex {
    /// In the mesh's own space, as it was bound.
    pub position: [f32; 2],
    /// Texture coordinates, `(0, 0)` top-left.
    pub uv: [f32; 2],
    /// Up to four bone slots; one at or past [`RIGID_SLOT`] is the mesh's
    /// own placement.
    pub bones: [u32; 4],
    /// How much each of `bones` holds it; summing to one.
    pub weights: [f32; 4],
}

impl SkinVertex {
    /// Attribute layout matching `skinned.wgsl`'s `@location`s.
    pub const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: size_of::<Self>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &wgpu::vertex_attr_array![
            0 => Float32x2,
            1 => Float32x2,
            2 => Uint32x4,
            3 => Float32x4
        ],
    };

    /// The vertex with its weights made safe to draw: every bone index in
    /// range, weights finite, not negative and summing to one — or, held by
    /// nothing, wholly the mesh's own placement.
    pub fn sanitized(self) -> Self {
        let mut bones = self.bones.map(|b| b.min(RIGID_SLOT));
        let mut weights = self
            .weights
            .map(|w| if w.is_finite() { w.max(0.0) } else { 0.0 });
        let total: f32 = weights.iter().sum();
        if total > 1e-6 {
            weights = weights.map(|w| w / total);
        } else {
            bones = [RIGID_SLOT, 0, 0, 0];
            weights = [1.0, 0.0, 0.0, 0.0];
        }
        Self {
            bones,
            weights,
            ..self
        }
    }
}

/// One bone's matrix as the shader reads it: x and y axes, then the
/// translation.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
struct BoneUniform {
    axes: [f32; 4],
    translation: [f32; 4],
}

impl BoneUniform {
    fn new(m: Mat3) -> Self {
        let m = if m.is_finite() { m } else { Mat3::IDENTITY };
        Self {
            axes: [m.x_axis.x, m.x_axis.y, m.y_axis.x, m.y_axis.y],
            translation: [m.z_axis.x, m.z_axis.y, 0.0, 0.0],
        }
    }
}

/// The uniform block: a tint, then the bones and the rigid slot.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct SkinUniform {
    tint: [f32; 4],
    bones: [BoneUniform; MAX_BONES + 1],
}

/// A skinned mesh's buffers and the group its pose is bound through.
pub struct SkinnedMesh {
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    index_count: u32,
    uniform: wgpu::Buffer,
    group: wgpu::BindGroup,
}

impl SkinnedMesh {
    /// Uploads `vertices` (each [sanitized](SkinVertex::sanitized)) and
    /// `indices`, posed at rest with a white tint until
    /// [`set_pose`](Self::set_pose). `None` for nothing to draw, or an index
    /// past the last vertex.
    pub fn new(
        device: &wgpu::Device,
        pipeline: &SkinPipeline,
        vertices: &[SkinVertex],
        indices: &[u32],
    ) -> Option<Self> {
        let count = u32::try_from(vertices.len()).ok()?;
        if indices.is_empty()
            || !indices.len().is_multiple_of(3)
            || indices.iter().any(|&i| i >= count)
        {
            return None;
        }
        let clean: Vec<SkinVertex> = vertices.iter().map(|v| v.sanitized()).collect();
        let vertices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("skinned-vertices"),
            contents: bytemuck::cast_slice(&clean),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_count = u32::try_from(indices.len()).ok()?;
        let indices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("skinned-indices"),
            contents: bytemuck::cast_slice(indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("skinned-pose"),
            contents: bytemuck::bytes_of(&pose(&[], Mat3::IDENTITY, [1.0; 4])),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("skinned-pose"),
            layout: &pipeline.layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            }],
        });
        Some(Self {
            vertices,
            indices,
            index_count,
            uniform,
            group,
        })
    }

    /// Sets the pose drawn from the next submission on: each bone slot's
    /// world matrix (bind pose to now; at most [`MAX_BONES`], the rest
    /// ignored), the mesh's own world matrix for the vertices no bone
    /// holds, and a linear RGBA tint.
    pub fn set_pose(&self, queue: &wgpu::Queue, bones: &[Mat3], rigid: Mat3, tint: [f32; 4]) {
        queue.write_buffer(
            &self.uniform,
            0,
            bytemuck::bytes_of(&pose(bones, rigid, tint)),
        );
    }

    /// Draws into `pass`, which has the skin pipeline, the camera at group 0
    /// and a texture at group 1 bound.
    pub(crate) fn draw(&self, pass: &mut wgpu::RenderPass<'_>) {
        pass.set_bind_group(2, &self.group, &[]);
        pass.set_vertex_buffer(0, self.vertices.slice(..));
        pass.set_index_buffer(self.indices.slice(..), wgpu::IndexFormat::Uint32);
        pass.draw_indexed(0..self.index_count, 0, 0..1);
    }
}

fn pose(bones: &[Mat3], rigid: Mat3, tint: [f32; 4]) -> SkinUniform {
    let mut block = SkinUniform {
        tint: tint.map(|c| if c.is_finite() { c } else { 0.0 }),
        bones: [BoneUniform::new(rigid); MAX_BONES + 1],
    };
    for (slot, matrix) in block.bones.iter_mut().zip(bones.iter().take(MAX_BONES)) {
        *slot = BoneUniform::new(*matrix);
    }
    block.bones[RIGID_SLOT as usize] = BoneUniform::new(rigid);
    block
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weights_are_made_safe_to_draw() {
        let v = SkinVertex {
            position: [0.0; 2],
            uv: [0.0; 2],
            bones: [1, 99, 0, 0],
            weights: [3.0, 1.0, f32::NAN, -1.0],
        }
        .sanitized();
        assert_eq!(
            v.bones[1], RIGID_SLOT,
            "an index past the bones is the mesh's own"
        );
        assert_eq!(v.weights, [0.75, 0.25, 0.0, 0.0]);
        let none = SkinVertex {
            weights: [0.0; 4],
            ..v
        }
        .sanitized();
        assert_eq!((none.bones[0], none.weights[0]), (RIGID_SLOT, 1.0));
    }

    #[test]
    fn a_pose_fills_the_unused_slots_with_the_mesh_s_own_matrix() {
        let rigid = Mat3::from_translation(glam::Vec2::new(5.0, 0.0));
        let block = pose(&[Mat3::IDENTITY], rigid, [1.0; 4]);
        assert_eq!(block.bones[0].translation[0], 0.0);
        assert_eq!(block.bones[1].translation[0], 5.0);
        assert_eq!(block.bones[RIGID_SLOT as usize].translation[0], 5.0);
        assert_eq!(size_of::<SkinUniform>(), 16 + 32 * (MAX_BONES + 1));
    }
}
