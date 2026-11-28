//! A standard surface on the GPU: its uniform block and its textures,
//! bound once and drawn by every instance naming it.

use std::sync::Arc;

use wgpu::util::DeviceExt;

use super::bank::AlphaClass;
use super::compose::CUSTOM_TEXTURES;
use super::custom::{CustomImage, CustomShading};
use super::layouts::{SurfaceLayouts, CUSTOM_BLOCK_BINDING, CUSTOM_TEXTURE_BINDING};
use super::shader::SurfaceShader;
use super::standard::StandardSurface;
use super::uniform::SurfaceUniform;
use crate::material::BLOCK_SIZE;
use crate::texture::Texture;

/// Every view a surface binds: the five standard slots, then the four
/// custom ones.
pub const SURFACE_VIEWS: usize = 5 + CUSTOM_TEXTURES;

/// The images a standard surface samples, each optional: an empty slot
/// binds white, which leaves its factor as it is (a missing normal map
/// leaves the vertex normal).
///
/// **Colour space**: the base colour and the emissive texture are colour,
/// sampled through the sRGB view so they reach the shader linear; the
/// metallic-roughness, normal and occlusion textures are data, sampled
/// through the raw view so their bytes arrive as written ("Data textures"
/// in ARCHITECTURE's wgpu notes). Decoding a normal map from sRGB bends
/// every normal; nothing reports it. A custom surface's own textures say
/// which they are ([`CustomImage::space`]).
#[derive(Debug, Clone, Copy, Default)]
pub struct SurfaceTextures<'a> {
    /// RGBA colour, multiplying `base_color`.
    pub base_color: Option<&'a Texture>,
    /// G roughness, B metallic, multiplying their factors.
    pub metallic_roughness: Option<&'a Texture>,
    /// A tangent-space normal, `(0.5, 0.5, 1)` straight out.
    pub normal: Option<&'a Texture>,
    /// R: how much ambient reaches the point.
    pub occlusion: Option<&'a Texture>,
    /// RGB colour, multiplying `emissive`.
    pub emissive: Option<&'a Texture>,
    /// A custom surface's own, in the order its shader names them.
    pub custom: [Option<CustomImage<'a>>; CUSTOM_TEXTURES],
}

impl SurfaceTextures<'_> {
    /// No textures: every factor as it is.
    pub const NONE: Self = Self {
        base_color: None,
        metallic_roughness: None,
        normal: None,
        occlusion: None,
        emissive: None,
        custom: [None; CUSTOM_TEXTURES],
    };

    /// Every view in binding order, white for an empty slot: colour
    /// through each texture's sRGB view, data through its raw one.
    pub fn views(&self, white: &wgpu::TextureView) -> [wgpu::TextureView; SURFACE_VIEWS] {
        let colour = |t: Option<&Texture>| t.map_or(white, Texture::view).clone();
        let data = |t: Option<&Texture>| t.map_or(white, Texture::raw_view).clone();
        let standard = [
            colour(self.base_color),
            data(self.metallic_roughness),
            data(self.normal),
            data(self.occlusion),
            colour(self.emissive),
        ];
        std::array::from_fn(|i| match i.checked_sub(standard.len()) {
            None => standard[i].clone(),
            Some(c) => self.custom[c]
                .as_ref()
                .map_or(white, CustomImage::view)
                .clone(),
        })
    }
}

/// A surface made for drawing: its blocks uploaded and bound with its
/// textures against [`SurfaceLayouts::material`], and what picks its
/// pipeline — its shader, its alpha mode and whether it is double-sided.
/// A standard surface's shader is [`SurfaceShader::standard`], its custom
/// block zeroed.
///
/// Made once per material and kept by whoever caches materials (the frame
/// keeps one per material handle and remakes it when the material or one
/// of its textures changes: [`views`](Self::views) are what it compares).
#[derive(Debug)]
pub struct SurfaceMaterial3D {
    bind_group: wgpu::BindGroup,
    alpha: AlphaClass,
    double_sided: bool,
    views: [wgpu::TextureView; SURFACE_VIEWS],
    surface: StandardSurface,
    shader: Arc<SurfaceShader>,
    custom: Option<CustomShading>,
}

impl SurfaceMaterial3D {
    /// `surface` with `textures`, shaded by `custom` (the standard shader
    /// for `None`), bound against `layouts`.
    pub fn new(
        device: &wgpu::Device,
        layouts: &SurfaceLayouts,
        surface: &StandardSurface,
        textures: &SurfaceTextures<'_>,
        custom: Option<&CustomShading>,
    ) -> Self {
        let block = SurfaceUniform::new(surface, textures.normal.is_some());
        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("surface"),
            contents: bytemuck::bytes_of(&block),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let custom_block = custom.map_or([0; BLOCK_SIZE], |c| c.block);
        let custom_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("surface-custom"),
            contents: &custom_block,
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let views = textures.views(&layouts.white);
        let mut entries = vec![
            wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&layouts.sampler),
            },
        ];
        let (standard, own) = views.split_at(5);
        entries.extend(
            standard
                .iter()
                .enumerate()
                .map(|(i, view)| wgpu::BindGroupEntry {
                    binding: 2 + i as u32,
                    resource: wgpu::BindingResource::TextureView(view),
                }),
        );
        entries.push(wgpu::BindGroupEntry {
            binding: CUSTOM_BLOCK_BINDING,
            resource: custom_buffer.as_entire_binding(),
        });
        entries.extend(
            own.iter()
                .enumerate()
                .map(|(i, view)| wgpu::BindGroupEntry {
                    binding: CUSTOM_TEXTURE_BINDING + i as u32,
                    resource: wgpu::BindingResource::TextureView(view),
                }),
        );
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("surface"),
            layout: &layouts.material,
            entries: &entries,
        });
        Self {
            bind_group,
            alpha: AlphaClass::of(surface.alpha),
            double_sided: surface.double_sided,
            views,
            surface: *surface,
            shader: custom.map_or_else(SurfaceShader::standard, |c| Arc::clone(&c.shader)),
            custom: custom.cloned(),
        }
    }

    /// The shader it is drawn through: what, with its alpha and sides,
    /// picks its pipeline.
    pub fn shader(&self) -> &Arc<SurfaceShader> {
        &self.shader
    }

    /// Group 1 of a surface pipeline.
    pub fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group
    }

    /// What its alpha does to the pipeline.
    pub fn alpha(&self) -> AlphaClass {
        self.alpha
    }

    /// Whether both faces are drawn.
    pub fn double_sided(&self) -> bool {
        self.double_sided
    }

    /// The numbers it was made from.
    pub fn surface(&self) -> &StandardSurface {
        &self.surface
    }

    /// The texture views it binds, in binding order: whether it is still
    /// the material `surface` and `textures` would make.
    pub fn views(&self) -> &[wgpu::TextureView; SURFACE_VIEWS] {
        &self.views
    }

    /// Whether it is what [`new`](Self::new) would make of `surface`,
    /// `textures` and `custom` now: the same numbers, shader and block, and
    /// the same views — a texture reloaded under its handle has a new view.
    pub fn is_current(
        &self,
        layouts: &SurfaceLayouts,
        surface: &StandardSurface,
        textures: &SurfaceTextures<'_>,
        custom: Option<&CustomShading>,
    ) -> bool {
        self.surface == *surface
            && self.custom.as_ref() == custom
            && self.views == textures.views(&layouts.white)
    }
}
