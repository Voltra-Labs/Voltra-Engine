//! What every surface pipeline and material binds against, made once per
//! renderer: the two group layouts, the sampler and the white texture an
//! empty slot binds.

use crate::texture::Texture;

/// How many textures a standard surface samples: base colour,
/// metallic-roughness, normal, occlusion, emissive. Of WebGL2's 16 a
/// fragment stage may sample, the plan keeps 12 (ARCHITECTURE "Materials").
pub const SURFACE_TEXTURES: u32 = 5;

/// The group layouts the surface pipelines are built with and the
/// materials bound against, the sampler every surface texture is read
/// through, and the white texture standing in for one not given.
///
/// Cheap to clone: every field is reference-counted.
#[derive(Debug, Clone)]
pub struct SurfaceLayouts {
    /// Group 0: the frame's view and light.
    pub frame: wgpu::BindGroupLayout,
    /// Group 1: a material's numbers, the sampler and its five textures.
    pub material: wgpu::BindGroupLayout,
    /// Linear, repeating: a UV transform may tile.
    pub sampler: wgpu::Sampler,
    /// 1 × 1 white: every factor's texture where none is named. White
    /// reads 1 through either view, so one stands for colour and data.
    pub white: wgpu::TextureView,
}

impl SurfaceLayouts {
    /// The layouts on `device`, and the white texture uploaded through
    /// `queue`.
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let frame = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("surface-frame-layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let texture = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let mut entries = vec![
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ];
        entries.extend((0..SURFACE_TEXTURES).map(|i| texture(2 + i)));
        let material = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("surface-material-layout"),
            entries: &entries,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("surface"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            // One mip level: textures have no chain yet.
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });
        Self {
            frame,
            material,
            sampler,
            white: Texture::white(device, queue).view().clone(),
        }
    }
}
