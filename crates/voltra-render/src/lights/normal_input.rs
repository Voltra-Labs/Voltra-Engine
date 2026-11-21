//! The scene's normal buffer as the accumulate pass reads it: a layout, a
//! flat stand-in for frames without one, and a binding for those with one.

use crate::normals::NORMAL_FORMAT;

/// Binds the normal buffer ([`crate::normals::NormalPass`]'s target) for
/// the accumulate pass, or a stand-in that reads as "no normal map here".
pub(super) struct NormalInput {
    layout: wgpu::BindGroupLayout,
    empty: wgpu::BindGroup,
}

impl NormalInput {
    pub(super) fn new(device: &wgpu::Device) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("light-normals-layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            }],
        });
        // Zeroed by wgpu: alpha 0, "flat", which is all the shader asks.
        let stand_in = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("light-normals-empty"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: NORMAL_FORMAT,
            usage: wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let empty = Self::bind_with(
            device,
            &layout,
            &stand_in.create_view(&wgpu::TextureViewDescriptor::default()),
        );
        Self { layout, empty }
    }

    pub(super) fn layout(&self) -> &wgpu::BindGroupLayout {
        &self.layout
    }

    /// The stand-in, for a frame whose normals were not drawn.
    pub(super) fn empty(&self) -> &wgpu::BindGroup {
        &self.empty
    }

    /// `normals`, bound.
    pub(super) fn bind(
        &self,
        device: &wgpu::Device,
        normals: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        Self::bind_with(device, &self.layout, normals)
    }

    fn bind_with(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        view: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("light-normals"),
            layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(view),
            }],
        })
    }
}
