//! Textures, samplers and their bind group.

mod decode;
mod error;
mod layout;
mod upload;

pub use decode::{decode_png, DecodedImage};
pub use error::TextureError;
pub use layout::bind_group_layout;
pub use upload::reinterprets_srgb;
use upload::upload;

/// How a texture is filtered when it does not map 1:1 to pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Filter {
    /// Smooth. The right default for photographic art.
    #[default]
    Linear,
    /// Blocky. The right choice for pixel art, where `Linear` turns crisp
    /// sprites into mush.
    Nearest,
}

impl From<Filter> for wgpu::FilterMode {
    fn from(filter: Filter) -> Self {
        match filter {
            Filter::Linear => Self::Linear,
            Filter::Nearest => Self::Nearest,
        }
    }
}

/// The format every loaded texture takes.
///
/// Srgb, not Unorm: image files store gamma-encoded colour, and this format
/// makes the GPU convert to linear on every sample. Getting it wrong makes
/// everything look washed out.
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

/// A 2D texture with the sampler it is drawn through.
#[derive(Debug)]
pub struct Texture {
    view: wgpu::TextureView,
    /// The same pixels seen as non-sRGB, for a consumer that converts itself.
    ///
    /// The pair exists for the same reason [`RenderTarget`](crate::RenderTarget)
    /// carries one: egui blends in gamma space and converts at the end of its
    /// shader, so letting the sampler convert as well darkens every mid-tone,
    /// and nothing reports it.
    raw_view: wgpu::TextureView,
    sampler: wgpu::Sampler,
    width: u32,
    height: u32,
}

impl Texture {
    /// Uploads tightly packed RGBA8 pixels.
    pub fn from_rgba8(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        label: &str,
        rgba: &[u8],
        width: u32,
        height: u32,
        filter: Filter,
    ) -> Result<Self, TextureError> {
        let expected = (width as usize) * (height as usize) * 4;
        if rgba.len() != expected {
            return Err(TextureError::SizeMismatch {
                expected,
                actual: rgba.len(),
                width,
                height,
            });
        }

        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };

        // The raw view reinterprets these pixels where the device can;
        // where it cannot (WebGL2), the same pixels go up a second time as
        // plain `Rgba8Unorm` — twice the memory, on the fallback path only,
        // rather than normal maps decoded as colour.
        let reinterpret = reinterprets_srgb(device);
        let texture = upload(device, queue, label, FORMAT, reinterpret, rgba, size);
        let raw_view = if reinterpret {
            texture.create_view(&wgpu::TextureViewDescriptor {
                format: Some(FORMAT.remove_srgb_suffix()),
                ..Default::default()
            })
        } else {
            upload(
                device,
                queue,
                label,
                FORMAT.remove_srgb_suffix(),
                false,
                rgba,
                size,
            )
            .create_view(&wgpu::TextureViewDescriptor::default())
        };

        let filter = wgpu::FilterMode::from(filter);
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some(label),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: filter,
            min_filter: filter,
            // Distinct type in wgpu 30, not FilterMode. Nearest is correct
            // while there is only one mip level to choose between.
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        Ok(Self {
            view: texture.create_view(&wgpu::TextureViewDescriptor::default()),
            raw_view,
            sampler,
            width,
            height,
        })
    }

    /// Decodes a PNG and uploads it.
    pub fn from_png(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        label: &str,
        bytes: &[u8],
        filter: Filter,
    ) -> Result<Self, TextureError> {
        Self::from_decoded(device, queue, label, &decode_png(bytes)?, filter)
    }

    /// Uploads pixels [`decode_png`] produced.
    pub fn from_decoded(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        label: &str,
        image: &DecodedImage,
        filter: Filter,
    ) -> Result<Self, TextureError> {
        Self::from_rgba8(
            device,
            queue,
            label,
            &image.rgba,
            image.width,
            image.height,
            filter,
        )
    }

    /// A 1x1 opaque white texture.
    ///
    /// Multiplying vertex colour by white is a no-op, so untextured geometry
    /// can go through the textured pipeline unchanged. That is what keeps
    /// there from being a second pipeline just for flat colour.
    pub fn white(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        Self::from_rgba8(
            device,
            queue,
            "white",
            &[255, 255, 255, 255],
            1,
            1,
            Filter::Nearest,
        )
        .expect("1x1 white texture has a valid size")
    }

    /// The pixels of `target`, sampled as a sprite's texture: what a
    /// camera drawing into a render texture is shown through. The texture is
    /// the target's own — what the camera draws, the sprite shows — so it
    /// must not outlive the target, nor be drawn while the target is drawn
    /// into.
    pub fn of_target(device: &wgpu::Device, target: &crate::RenderTarget, filter: Filter) -> Self {
        let texture = target.texture().clone();
        let format = target.format();
        let raw = if reinterprets_srgb(device) {
            format.remove_srgb_suffix()
        } else {
            format
        };
        let filter = wgpu::FilterMode::from(filter);
        Self {
            view: texture.create_view(&wgpu::TextureViewDescriptor::default()),
            raw_view: texture.create_view(&wgpu::TextureViewDescriptor {
                format: Some(raw),
                ..Default::default()
            }),
            sampler: device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("render-texture"),
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                address_mode_w: wgpu::AddressMode::ClampToEdge,
                mag_filter: filter,
                min_filter: filter,
                mipmap_filter: wgpu::MipmapFilterMode::Nearest,
                ..Default::default()
            }),
            width: target.width(),
            height: target.height(),
        }
    }

    /// Width, in texels.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Height, in texels.
    pub fn height(&self) -> u32 {
        self.height
    }

    /// The view a draw samples colour through: sRGB, so a texel reaches the
    /// shader linear. A base-colour or emissive map is read this way; data
    /// (a normal or metallic-roughness map) through [`raw_view`](Self::raw_view).
    pub fn view(&self) -> &wgpu::TextureView {
        &self.view
    }

    /// The non-sRGB view of the same pixels.
    ///
    /// What egui must sample — an editor thumbnail goes through egui's shader,
    /// which converts at the end, so sampling the sRGB view there converts
    /// twice. The scene's own draws use the bind group, which holds the sRGB
    /// view, because the sprite pipeline does not convert.
    pub fn raw_view(&self) -> &wgpu::TextureView {
        &self.raw_view
    }

    /// Binds this texture and its sampler against [`bind_group_layout`].
    pub fn create_bind_group(
        &self,
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
    ) -> wgpu::BindGroup {
        self.bind(device, layout, &self.view, "texture-bind-group")
    }

    /// Binds the [`raw_view`](Self::raw_view) and the sampler against
    /// [`bind_group_layout`]: what a texture holding data rather than colour
    /// — a normal map — is sampled through, so its bytes arrive as written
    /// instead of decoded from sRGB.
    pub fn create_raw_bind_group(
        &self,
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
    ) -> wgpu::BindGroup {
        self.bind(device, layout, &self.raw_view, "raw-texture-bind-group")
    }

    fn bind(
        &self,
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        view: &wgpu::TextureView,
        label: &str,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(label),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        })
    }
}

#[cfg(test)]
mod tests;
