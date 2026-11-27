//! The camera's GPU side: its uniform, buffer and bind group.

use bytemuck::{Pod, Zeroable};
use glam::Mat4;

use super::View2D;

/// The camera matrix as the GPU sees it.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct CameraUniform {
    pub(super) view_proj: [[f32; 4]; 4],
}

impl From<&View2D> for CameraUniform {
    fn from(camera: &View2D) -> Self {
        Self {
            view_proj: camera.view_projection().to_cols_array_2d(),
        }
    }
}

/// Uniform buffer, layout and bind group for a [`View2D`].
///
/// The layout is created here rather than derived from the shader so the
/// pipeline can be built before any camera exists.
pub struct CameraBinding {
    buffer: wgpu::Buffer,
    layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
}

impl CameraBinding {
    /// Creates the camera uniform buffer with its layout and bind group.
    pub fn new(device: &wgpu::Device) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("camera-bind-group-layout"),
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
        Self::with_layout(device, &layout)
    }

    /// A second binding - its own buffer - against `self`'s layout, so it
    /// binds to every pipeline built for this one.
    ///
    /// Two cameras drawn in one frame need two buffers: `write_buffer` lands
    /// at the next submit, so a second write to one buffer would replace the
    /// first before either pass ran.
    pub fn sharing_layout(&self, device: &wgpu::Device) -> Self {
        Self::with_layout(device, &self.layout)
    }

    fn with_layout(device: &wgpu::Device, layout: &wgpu::BindGroupLayout) -> Self {
        let layout = layout.clone();
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("camera-uniform"),
            size: size_of::<CameraUniform>() as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("camera-bind-group"),
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

    /// Writes the camera matrix to the GPU. Cheap enough to call every frame.
    pub fn upload(&self, queue: &wgpu::Queue, camera: &View2D) {
        let uniform = CameraUniform::from(camera);
        queue.write_buffer(&self.buffer, 0, bytemuck::bytes_of(&uniform));
    }

    /// Writes an arbitrary matrix - [`screen_projection`] - instead of a
    /// camera's.
    pub fn upload_matrix(&self, queue: &wgpu::Queue, view_proj: Mat4) {
        let uniform = CameraUniform {
            view_proj: view_proj.to_cols_array_2d(),
        };
        queue.write_buffer(&self.buffer, 0, bytemuck::bytes_of(&uniform));
    }

    /// The layout of the camera's bind group (group 0).
    pub fn layout(&self) -> &wgpu::BindGroupLayout {
        &self.layout
    }

    /// The bind group holding the camera's view-projection matrix.
    pub fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group
    }
}
