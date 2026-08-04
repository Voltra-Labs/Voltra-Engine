//! An offscreen texture the scene can be drawn into.
//!
//! The editor renders the world here rather than straight to the swapchain, so
//! the result can be shown inside a panel and the viewport can have its own
//! size, aspect and picking rect independent of the window.

use crate::texture::Filter;

/// A colour texture usable both as a render attachment and as a sampled
/// texture.
#[derive(Debug)]
pub struct RenderTarget {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    raw_view: wgpu::TextureView,
    sampler: wgpu::Sampler,
    /// Kept so a resize can recreate the texture under the same name.
    label: String,
    format: wgpu::TextureFormat,
    width: u32,
    height: u32,
}

impl RenderTarget {
    pub fn new(
        device: &wgpu::Device,
        label: &str,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
        filter: Filter,
    ) -> Self {
        let (width, height) = clamp_size(width, height);
        let filter = wgpu::FilterMode::from(filter);

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some(label),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: filter,
            min_filter: filter,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        let texture = create_texture(device, label, format, width, height);
        let (view, raw_view) = create_views(&texture, format);

        Self {
            texture,
            view,
            raw_view,
            sampler,
            label: label.to_owned(),
            format,
            width,
            height,
        }
    }

    /// Reallocates only when the size actually changed. Returns whether it did.
    ///
    /// A docked viewport hands its panel rect over every frame, so the common
    /// call is a no-op. Reallocating anyway would throw away a texture per
    /// frame and silently invalidate every bind group holding the old view.
    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) -> bool {
        let (width, height) = clamp_size(width, height);
        if (width, height) == (self.width, self.height) {
            return false;
        }

        self.texture = create_texture(device, &self.label, self.format, width, height);
        (self.view, self.raw_view) = create_views(&self.texture, self.format);
        self.width = width;
        self.height = height;
        true
    }

    pub fn texture(&self) -> &wgpu::Texture {
        &self.texture
    }

    /// The attachment to hand a render pass.
    pub fn view(&self) -> &wgpu::TextureView {
        &self.view
    }

    /// A view that hands back the stored bytes with no sRGB conversion.
    ///
    /// A shader that does its own gamma handling — egui's does — must sample
    /// through this one. Sampling the sRGB view instead makes the hardware
    /// convert as well, and the image comes out visibly dark. For a target that
    /// is not sRGB to begin with, this is the same view.
    pub fn raw_view(&self) -> &wgpu::TextureView {
        &self.raw_view
    }

    pub fn format(&self) -> wgpu::TextureFormat {
        self.format
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    /// Width over height, for the camera drawing into this target.
    pub fn aspect(&self) -> f32 {
        self.width as f32 / self.height as f32
    }

    /// Binds this target for sampling, against [`crate::texture::bind_group_layout`].
    ///
    /// Uses [`raw_view`](Self::raw_view), because the shaders that sample a
    /// target are the ones that manage gamma themselves.
    ///
    /// Rebuild it after any [`resize`](Self::resize) that returned `true`; the
    /// old one still points at the freed texture.
    pub fn create_bind_group(
        &self,
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(&self.label),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&self.raw_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        })
    }
}

/// wgpu rejects a zero extent, and a collapsed or mid-drag panel reports
/// exactly that. Clamping turns a crash into a wasted pixel.
fn clamp_size(width: u32, height: u32) -> (u32, u32) {
    (width.max(1), height.max(1))
}

/// The attachment view and the conversion-free view, in that order.
fn create_views(
    texture: &wgpu::Texture,
    format: wgpu::TextureFormat,
) -> (wgpu::TextureView, wgpu::TextureView) {
    (
        texture.create_view(&wgpu::TextureViewDescriptor::default()),
        texture.create_view(&wgpu::TextureViewDescriptor {
            format: Some(format.remove_srgb_suffix()),
            ..Default::default()
        }),
    )
}

fn create_texture(
    device: &wgpu::Device,
    label: &str,
    format: wgpu::TextureFormat,
    width: u32,
    height: u32,
) -> wgpu::Texture {
    // Reinterpreting an sRGB texture as its plain counterpart has to be
    // declared up front; a view format missing from this list is rejected.
    let raw = format.remove_srgb_suffix();
    let view_formats = if raw == format { Vec::new() } else { vec![raw] };

    device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        // TEXTURE_BINDING is what lets the viewport panel sample the result;
        // COPY_SRC is what lets a test read it back.
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC,
        view_formats: &view_formats,
    })
}

#[cfg(test)]
mod tests {
    use super::clamp_size;

    #[test]
    fn sizes_never_reach_zero() {
        assert_eq!(clamp_size(0, 0), (1, 1));
        assert_eq!(clamp_size(0, 24), (1, 24));
        assert_eq!(clamp_size(320, 0), (320, 1));
        assert_eq!(clamp_size(320, 160), (320, 160));
    }
}
