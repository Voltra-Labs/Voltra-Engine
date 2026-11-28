//! What the device can draw into, per format: asked once, when the context
//! is made, because only the adapter can say.

use crate::render3d::DEPTH_FORMAT;

/// The target formats a 3D shot may use, as the device will validate them.
///
/// Where the adapter meets WebGPU's format rules
/// (`DownlevelFlags::WEBGPU_TEXTURE_FORMAT_SUPPORT`) wgpu checks every
/// texture against the formats' *guaranteed* features, so those are what
/// is recorded; where it does not — WebGL2 — wgpu asks the backend, and so
/// does this (`wgpu-core`'s `describe_format_features`). On WebGL2
/// `Rgba16Float` is a render target only with `EXT_color_buffer_float` or
/// `EXT_color_buffer_half_float`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TargetSupport {
    /// `Rgba16Float`, the HDR target's format.
    pub float: wgpu::TextureFormatFeatures,
    /// The frame's own sRGB format, the HDR target's stand-in.
    pub srgb: wgpu::TextureFormatFeatures,
    /// The depth format, [`DEPTH_FORMAT`].
    pub depth: wgpu::TextureFormatFeatures,
}

impl TargetSupport {
    /// What `adapter`, opened with `features`, supports, `srgb` being the
    /// frame's format.
    pub(super) fn of(
        adapter: &wgpu::Adapter,
        features: wgpu::Features,
        srgb: wgpu::TextureFormat,
    ) -> Self {
        let downlevel = !adapter
            .get_downlevel_capabilities()
            .flags
            .contains(wgpu::DownlevelFlags::WEBGPU_TEXTURE_FORMAT_SUPPORT);
        let specific = features.contains(wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES);
        let ask = |format: wgpu::TextureFormat| {
            if downlevel || specific {
                adapter.get_texture_format_features(format)
            } else {
                format.guaranteed_format_features(features)
            }
        };
        Self {
            float: ask(wgpu::TextureFormat::Rgba16Float),
            srgb: ask(srgb),
            depth: ask(DEPTH_FORMAT),
        }
    }

    /// Whether `features` lets a format be drawn into and sampled after.
    pub fn renders(features: &wgpu::TextureFormatFeatures) -> bool {
        features
            .allowed_usages
            .contains(wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING)
            && features
                .flags
                .contains(wgpu::TextureFormatFeatureFlags::FILTERABLE)
    }

    /// Whether `features` lets a format be drawn `samples` times a pixel
    /// and, for a colour format, resolved.
    pub fn multisamples(
        features: &wgpu::TextureFormatFeatures,
        samples: u32,
        colour: bool,
    ) -> bool {
        features.flags.sample_count_supported(samples)
            && (!colour
                || samples == 1
                || features
                    .flags
                    .contains(wgpu::TextureFormatFeatureFlags::MULTISAMPLE_RESOLVE))
    }
}
