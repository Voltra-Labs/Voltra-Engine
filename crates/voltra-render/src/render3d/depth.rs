//! The 3D pass's depth target: reversed Z in a float.

/// The depth format: a 32-bit float, core on every backend, WebGL2
/// included. With reversed Z it spreads precision evenly over distance.
pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

/// What depth is cleared to: the far plane, under reversed Z.
pub const DEPTH_CLEAR: f32 = 0.0;

/// The depth test: nearer is greater, under reversed Z.
pub const DEPTH_COMPARE: wgpu::CompareFunction = wgpu::CompareFunction::GreaterEqual;

/// A depth texture kept the size of the colour target it is drawn with.
#[derive(Debug)]
pub struct DepthTarget {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
}

impl DepthTarget {
    /// A `width` × `height` depth target, each at least 1.
    pub fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("depth-3d"),
            size: wgpu::Extent3d {
                width: width.max(1),
                height: height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Self { texture, view }
    }

    /// Remakes it at `width` × `height` if it is not that size already;
    /// whether it did.
    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) -> bool {
        if self.size() == (width.max(1), height.max(1)) {
            return false;
        }
        *self = Self::new(device, width, height);
        true
    }

    /// Its size in pixels.
    pub fn size(&self) -> (u32, u32) {
        (self.texture.width(), self.texture.height())
    }

    /// The view the pass attaches.
    pub fn view(&self) -> &wgpu::TextureView {
        &self.view
    }
}
