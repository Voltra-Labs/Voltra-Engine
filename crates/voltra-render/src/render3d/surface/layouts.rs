//! What every surface pipeline and material binds against, made once per
//! renderer: the two group layouts, the sampler and the white texture an
//! empty slot binds.

use super::compose::CUSTOM_TEXTURES;
use crate::texture::Texture;

/// How many textures a standard surface samples: base colour,
/// metallic-roughness, normal, occlusion, emissive. Of WebGL2's 16 a
/// fragment stage may sample, the plan keeps 12 (ARCHITECTURE "Materials").
pub const SURFACE_TEXTURES: u32 = 5;

/// Where a custom surface's uniform block is bound in group 1, after the
/// standard block, its sampler and its five textures.
pub const CUSTOM_BLOCK_BINDING: u32 = 2 + SURFACE_TEXTURES;

/// Where a custom surface's first texture is bound in group 1; the rest
/// follow, [`CUSTOM_TEXTURES`] in all.
pub const CUSTOM_TEXTURE_BINDING: u32 = CUSTOM_BLOCK_BINDING + 1;

/// The group layouts the surface pipelines are built with and the
/// materials bound against, the sampler every surface texture is read
/// through, and the white texture standing in for one not given.
///
/// Cheap to clone: every field is reference-counted.
#[derive(Debug, Clone)]
pub struct SurfaceLayouts {
    /// Group 0: the frame's view, ambient, fog and sky (binding 0) and its
    /// light array (binding 1).
    pub frame: wgpu::BindGroupLayout,
    /// Group 1: a material's numbers, the sampler and its five textures,
    /// then a custom surface's uniform block and its four textures — bound
    /// by every material, a standard one's empty (a zeroed block, white).
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
        let uniform = |binding, visibility| wgpu::BindGroupLayoutEntry {
            binding,
            visibility,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let frame = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("surface-frame-layout"),
            entries: &[
                // The view, the ambient, the fog and the sky.
                uniform(0, wgpu::ShaderStages::VERTEX_FRAGMENT),
                // The light array, read where a surface is lit.
                uniform(1, wgpu::ShaderStages::FRAGMENT),
            ],
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
        entries.push(wgpu::BindGroupLayoutEntry {
            binding: CUSTOM_BLOCK_BINDING,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        });
        entries.extend((0..CUSTOM_TEXTURES as u32).map(|i| texture(CUSTOM_TEXTURE_BINDING + i)));
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
