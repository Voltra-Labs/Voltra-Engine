//! The scene's buffers as the light passes read them, as one bind group:
//! its normals (binding 0) and the light layers each texel is not on
//! (binding 1), each a 1×1 stand-in when the frame did not draw it.
//!
//! One group rather than one each: the accumulate pass is already at the
//! four bind groups WebGPU guarantees, and both buffers are the frame's size
//! and drawn from the same batch.

use crate::light_layers::LAYER_FORMAT;
use crate::normals::NORMAL_FORMAT;

/// The layout, and the stand-ins for a frame without one buffer or both.
pub(super) struct SceneInput {
    layout: wgpu::BindGroupLayout,
    /// Zeroed: alpha 0, "no normal map here", which reads as facing the
    /// screen.
    flat: wgpu::TextureView,
    /// Zeroed: no layer left out, so every light reaches it.
    every_layer: wgpu::TextureView,
}

impl SceneInput {
    pub(super) fn new(device: &wgpu::Device) -> Self {
        let entry = |binding, sample_type| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type,
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("light-scene-layout"),
            entries: &[
                entry(0, wgpu::TextureSampleType::Float { filterable: false }),
                entry(1, wgpu::TextureSampleType::Uint),
            ],
        });
        Self {
            layout,
            flat: stand_in(device, "light-normals-empty", NORMAL_FORMAT),
            every_layer: stand_in(device, "light-layers-empty", LAYER_FORMAT),
        }
    }

    pub(super) fn layout(&self) -> &wgpu::BindGroupLayout {
        &self.layout
    }

    /// `normals` and `layers` bound, a stand-in for whichever is `None`.
    pub(super) fn bind(
        &self,
        device: &wgpu::Device,
        normals: Option<&wgpu::TextureView>,
        layers: Option<&wgpu::TextureView>,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("light-scene"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(normals.unwrap_or(&self.flat)),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(
                        layers.unwrap_or(&self.every_layer),
                    ),
                },
            ],
        })
    }
}

/// A 1×1 texture of `format`, zeroed by wgpu.
fn stand_in(device: &wgpu::Device, label: &str, format: wgpu::TextureFormat) -> wgpu::TextureView {
    device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        })
        .create_view(&wgpu::TextureViewDescriptor::default())
}
