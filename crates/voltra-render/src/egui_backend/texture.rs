//! Egui's textures: the deltas it uploads, and the views the application
//! registers back for things like the viewport image.
//!
//! `BoundTexture` pairs a bind group with the wgpu texture behind it, when
//! there is one — a registered view does not own its texture, so nothing here
//! is dropped when the entry is freed. `filter_of` and `create_sampler` turn
//! egui's texture options into the wgpu state a sampler needs.

use epaint::textures::{TextureFilter, TextureOptions, TextureWrapMode};
use epaint::{ImageData, ImageDelta, TextureId};

use super::EguiBackend;

/// One texture egui asked us to hold, with the bind group that draws it.
pub(super) struct BoundTexture {
    /// `None` for a view registered by the application: the bind group borrows
    /// it, but the texture is not ours to keep alive or to write into.
    texture: Option<wgpu::Texture>,
    pub(super) bind_group: wgpu::BindGroup,
    /// Kept so a delta that changes only the filtering can be detected.
    options: Option<TextureOptions>,
}

impl EguiBackend {
    /// Hands egui a texture the application owns, such as the viewport target.
    ///
    /// The returned id stays valid until [`Self::free_view`]; after a resize
    /// call [`Self::update_view`], because the old view points at freed memory.
    pub fn register_view(
        &mut self,
        device: &wgpu::Device,
        view: &wgpu::TextureView,
        filter: crate::Filter,
    ) -> TextureId {
        let id = TextureId::User(self.next_user_id);
        self.next_user_id += 1;
        self.set_view(device, id, view, filter);
        id
    }

    /// Points an id registered by [`Self::register_view`] at a different view.
    pub fn update_view(
        &mut self,
        device: &wgpu::Device,
        id: TextureId,
        view: &wgpu::TextureView,
        filter: crate::Filter,
    ) {
        self.set_view(device, id, view, filter);
    }

    pub fn free_view(&mut self, id: TextureId) {
        self.textures.remove(&id);
    }

    fn set_view(
        &mut self,
        device: &wgpu::Device,
        id: TextureId,
        view: &wgpu::TextureView,
        filter: crate::Filter,
    ) {
        let options = TextureOptions {
            magnification: filter_of(filter),
            minification: filter_of(filter),
            ..Default::default()
        };
        let bind_group = self.bind(device, view, options);
        self.textures.insert(
            id,
            BoundTexture {
                texture: None,
                bind_group,
                options: None,
            },
        );
    }

    pub(super) fn set_texture(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        id: TextureId,
        delta: &ImageDelta,
    ) {
        let ImageData::Color(image) = &delta.image;
        let width = image.width() as u32;
        let height = image.height() as u32;
        let pixels: &[u8] = bytemuck::cast_slice(&image.pixels);

        // `pos` means this is a patch of an existing texture — egui grows its
        // font atlas by re-uploading only the new glyphs.
        let existing = delta.pos.and_then(|_| self.textures.remove(&id));

        let (texture, origin, reuse) = match (existing, delta.pos) {
            (Some(bound), Some(pos)) => {
                let Some(texture) = bound.texture else {
                    log::warn!("egui tried to patch {id:?}, which the application owns");
                    return;
                };
                let origin = wgpu::Origin3d {
                    x: pos[0] as u32,
                    y: pos[1] as u32,
                    z: 0,
                };
                // Only the filtering decides the bind group, so an unchanged
                // patch keeps the one it already had.
                let reuse = (bound.options == Some(delta.options)).then_some(bound.bind_group);
                (texture, origin, reuse)
            }
            _ => {
                let texture = device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("egui-texture"),
                    size: wgpu::Extent3d {
                        width,
                        height,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    // Unorm, not UnormSrgb: egui's pixels are already gamma
                    // encoded and its blending happens in that space, so the
                    // shader converts at the end rather than the sampler at the
                    // start.
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                    view_formats: &[],
                });
                (texture, wgpu::Origin3d::ZERO, None)
            }
        };

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin,
                aspect: wgpu::TextureAspect::All,
            },
            pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * width),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        let bind_group = reuse.unwrap_or_else(|| {
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            self.bind(device, &view, delta.options)
        });

        self.textures.insert(
            id,
            BoundTexture {
                texture: Some(texture),
                bind_group,
                options: Some(delta.options),
            },
        );
    }

    fn bind(
        &mut self,
        device: &wgpu::Device,
        view: &wgpu::TextureView,
        options: TextureOptions,
    ) -> wgpu::BindGroup {
        // egui uses a handful of option sets at most, and a sampler per texture
        // would mean a new one for every glyph atlas patch.
        let sampler = self
            .samplers
            .entry(options)
            .or_insert_with(|| create_sampler(device, options));

        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("egui-texture-bind-group"),
            layout: &self.texture_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        })
    }
}

fn filter_of(filter: crate::Filter) -> TextureFilter {
    match filter {
        crate::Filter::Linear => TextureFilter::Linear,
        crate::Filter::Nearest => TextureFilter::Nearest,
    }
}

fn create_sampler(device: &wgpu::Device, options: TextureOptions) -> wgpu::Sampler {
    let address_mode = match options.wrap_mode {
        TextureWrapMode::ClampToEdge => wgpu::AddressMode::ClampToEdge,
        TextureWrapMode::Repeat => wgpu::AddressMode::Repeat,
        TextureWrapMode::MirroredRepeat => wgpu::AddressMode::MirrorRepeat,
    };
    let mode = |filter| match filter {
        TextureFilter::Nearest => wgpu::FilterMode::Nearest,
        TextureFilter::Linear => wgpu::FilterMode::Linear,
    };

    device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("egui-sampler"),
        address_mode_u: address_mode,
        address_mode_v: address_mode,
        address_mode_w: address_mode,
        mag_filter: mode(options.magnification),
        min_filter: mode(options.minification),
        mipmap_filter: wgpu::MipmapFilterMode::Nearest,
        ..Default::default()
    })
}
