//! Textures, samplers and their bind group.

use std::error::Error;
use std::fmt;

/// Why a texture could not be created.
#[derive(Debug)]
pub enum TextureError {
    /// The bytes were not a PNG, or were a corrupt one.
    Decode(image::ImageError),
    /// The pixel data does not match the stated dimensions.
    SizeMismatch {
        expected: usize,
        actual: usize,
        width: u32,
        height: u32,
    },
}

impl fmt::Display for TextureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Decode(e) => write!(f, "failed to decode image: {e}"),
            Self::SizeMismatch {
                expected,
                actual,
                width,
                height,
            } => write!(
                f,
                "pixel data is {actual} bytes but {width}x{height} RGBA needs {expected}"
            ),
        }
    }
}

impl Error for TextureError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Decode(e) => Some(e),
            Self::SizeMismatch { .. } => None,
        }
    }
}

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

/// A 2D texture with the sampler it is drawn through.
#[derive(Debug)]
pub struct Texture {
    view: wgpu::TextureView,
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

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            // Srgb, not Unorm: image files store gamma-encoded colour, and
            // this format makes the GPU convert to linear on every sample.
            // Getting it wrong makes everything look washed out.
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        // `write_texture` has no 256-byte row alignment requirement, unlike
        // `copy_buffer_to_texture`.
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * width),
                rows_per_image: Some(height),
            },
            size,
        );

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
        let image = image::load_from_memory_with_format(bytes, image::ImageFormat::Png)
            .map_err(TextureError::Decode)?;
        let rgba = image.to_rgba8();
        let (width, height) = rgba.dimensions();
        Self::from_rgba8(device, queue, label, &rgba, width, height, filter)
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

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    /// Binds this texture and its sampler against [`bind_group_layout`].
    pub fn create_bind_group(
        &self,
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("texture-bind-group"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&self.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        })
    }
}

/// Layout for group 1: a sampled texture at binding 0, its sampler at 1.
///
/// Free-standing rather than a method so the pipeline can be built before any
/// texture exists.
pub fn bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("texture-bind-group-layout"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                // Filtering, matching `filterable: true` above. A mismatch
                // between the two is a validation error, not a visual bug.
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_maps_onto_wgpu() {
        assert_eq!(
            wgpu::FilterMode::from(Filter::Linear),
            wgpu::FilterMode::Linear
        );
        assert_eq!(
            wgpu::FilterMode::from(Filter::Nearest),
            wgpu::FilterMode::Nearest
        );
        assert_eq!(Filter::default(), Filter::Linear);
    }

    #[test]
    fn size_mismatch_message_names_both_sizes() {
        let err = TextureError::SizeMismatch {
            expected: 16,
            actual: 12,
            width: 2,
            height: 2,
        };
        let text = err.to_string();
        assert!(text.contains("12"), "{text}");
        assert!(text.contains("16"), "{text}");
        assert!(text.contains("2x2"), "{text}");
    }
}
