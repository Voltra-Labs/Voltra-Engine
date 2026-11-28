//! The textures one 3D shot is drawn into: colour (multisampled or not),
//! its resolve, and depth — all one size.

use super::depth::DepthTarget;
use super::hdr::Hdr3D;

/// A 3D shot's targets, kept the same size and remade together.
///
/// With one sample the colour texture is the picture the post chain reads;
/// with more, the pass draws into a multisampled colour texture that is
/// resolved into a second, single-sampled one at the end of the pass — the
/// multisampled contents are then discarded, as nothing reads them.
#[derive(Debug)]
pub struct Targets3D {
    hdr: Hdr3D,
    width: u32,
    height: u32,
    /// What the pass draws into: `hdr.samples` a pixel.
    color: wgpu::TextureView,
    /// The single-sampled picture, when `color` is multisampled.
    resolved: Option<wgpu::TextureView>,
    depth: DepthTarget,
    /// Every texture, destroyed when remade or dropped.
    textures: Vec<wgpu::Texture>,
}

impl Targets3D {
    /// Targets of `hdr`'s format and samples, `width` × `height`, each at
    /// least 1.
    pub fn new(device: &wgpu::Device, hdr: Hdr3D, width: u32, height: u32) -> Self {
        let (width, height) = (width.max(1), height.max(1));
        let make = |label, samples, usage| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: samples,
                dimension: wgpu::TextureDimension::D2,
                format: hdr.format,
                usage,
                view_formats: &[],
            })
        };
        let picture = wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING;
        let mut textures = Vec::new();
        let (color, resolved) = if hdr.samples > 1 {
            let msaa = make(
                "color-3d-msaa",
                hdr.samples,
                wgpu::TextureUsages::RENDER_ATTACHMENT,
            );
            let resolved = make("color-3d", 1, picture);
            let views = (
                msaa.create_view(&wgpu::TextureViewDescriptor::default()),
                Some(resolved.create_view(&wgpu::TextureViewDescriptor::default())),
            );
            textures.extend([msaa, resolved]);
            views
        } else {
            let color = make("color-3d", 1, picture);
            let view = color.create_view(&wgpu::TextureViewDescriptor::default());
            textures.push(color);
            (view, None)
        };
        Self {
            hdr,
            width,
            height,
            color,
            resolved,
            depth: DepthTarget::with_samples(device, width, height, hdr.samples),
            textures,
        }
    }

    /// Remakes them at `width` × `height` if they are not that size; whether
    /// they were remade.
    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) -> bool {
        if self.size() == (width.max(1), height.max(1)) {
            return false;
        }
        *self = Self::new(device, self.hdr, width, height);
        true
    }

    /// The format and samples they were made with.
    pub fn hdr(&self) -> Hdr3D {
        self.hdr
    }

    /// Their size in pixels.
    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// The depth target's size, the colour's by construction.
    pub fn depth_size(&self) -> (u32, u32) {
        self.depth.size()
    }

    /// What the pass draws colour into.
    pub fn color(&self) -> &wgpu::TextureView {
        &self.color
    }

    /// Where the pass resolves colour to, when it is multisampled.
    pub fn resolve(&self) -> Option<&wgpu::TextureView> {
        self.resolved.as_ref()
    }

    /// The depth attachment.
    pub fn depth(&self) -> &wgpu::TextureView {
        self.depth.view()
    }

    /// The finished, single-sampled linear picture the post chain reads.
    pub fn picture(&self) -> &wgpu::TextureView {
        self.resolved.as_ref().unwrap_or(&self.color)
    }
}

/// Destroyed rather than left to the drop, as a
/// [`RenderTarget`](crate::RenderTarget) is: freed at once on the web too.
impl Drop for Targets3D {
    fn drop(&mut self) {
        for texture in &self.textures {
            texture.destroy();
        }
    }
}
