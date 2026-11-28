//! A standard surface on the GPU: its uniform block and its textures,
//! bound once and drawn by every instance naming it.

use wgpu::util::DeviceExt;

use super::bank::AlphaClass;
use super::layouts::SurfaceLayouts;
use super::standard::StandardSurface;
use super::uniform::SurfaceUniform;
use crate::texture::Texture;

/// The images a standard surface samples, each optional: an empty slot
/// binds white, which leaves its factor as it is (a missing normal map
/// leaves the vertex normal).
///
/// **Colour space**: the base colour and the emissive texture are colour,
/// sampled through the sRGB view so they reach the shader linear; the
/// metallic-roughness, normal and occlusion textures are data, sampled
/// through the raw view so their bytes arrive as written ("Data textures"
/// in ARCHITECTURE's wgpu notes). Decoding a normal map from sRGB bends
/// every normal; nothing reports it.
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
}

impl SurfaceTextures<'_> {
    /// No textures: every factor as it is.
    pub const NONE: Self = Self {
        base_color: None,
        metallic_roughness: None,
        normal: None,
        occlusion: None,
        emissive: None,
    };

    /// The five views in binding order, white for an empty slot: colour
    /// through each texture's sRGB view, data through its raw one.
    pub fn views(&self, white: &wgpu::TextureView) -> [wgpu::TextureView; 5] {
        let colour = |t: Option<&Texture>| t.map_or(white, Texture::view).clone();
        let data = |t: Option<&Texture>| t.map_or(white, Texture::raw_view).clone();
        [
            colour(self.base_color),
            data(self.metallic_roughness),
            data(self.normal),
            data(self.occlusion),
            colour(self.emissive),
        ]
    }
}

/// A standard surface made for drawing: its block uploaded and bound with
/// its textures against [`SurfaceLayouts::material`], and what picks its
/// pipeline — its alpha mode and whether it is double-sided.
///
/// Made once per material and kept by whoever caches materials (the frame
/// keeps one per material handle and remakes it when the material or one
/// of its textures changes: [`views`](Self::views) are what it compares).
#[derive(Debug)]
pub struct SurfaceMaterial3D {
    bind_group: wgpu::BindGroup,
    alpha: AlphaClass,
    double_sided: bool,
    views: [wgpu::TextureView; 5],
    surface: StandardSurface,
}

impl SurfaceMaterial3D {
    /// `surface` with `textures`, bound against `layouts`.
    pub fn new(
        device: &wgpu::Device,
        layouts: &SurfaceLayouts,
        surface: &StandardSurface,
        textures: &SurfaceTextures<'_>,
    ) -> Self {
        let block = SurfaceUniform::new(surface, textures.normal.is_some());
        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("surface"),
            contents: bytemuck::bytes_of(&block),
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
        entries.extend(
            views
                .iter()
                .enumerate()
                .map(|(i, view)| wgpu::BindGroupEntry {
                    binding: 2 + i as u32,
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
        }
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
    pub fn views(&self) -> &[wgpu::TextureView; 5] {
        &self.views
    }

    /// Whether it is what [`new`](Self::new) would make of `surface` and
    /// `textures` now: the same numbers and the same views — a texture
    /// reloaded under its handle has a new view.
    pub fn is_current(
        &self,
        layouts: &SurfaceLayouts,
        surface: &StandardSurface,
        textures: &SurfaceTextures<'_>,
    ) -> bool {
        self.surface == *surface && self.views == textures.views(&layouts.white)
    }
}
