//! What a 3D shot is drawn into: the format and the samples a pixel,
//! chosen against what the device can do.

use crate::context::TargetSupport;

/// The colour format and the samples a pixel of a 3D shot's target.
///
/// `Rgba16Float`, linear and with room above one for light that adds up,
/// [`MSAA`](Self::DEFAULT_SAMPLES) 4× by default, resolved before the post
/// chain. **Degrades rather than fails**: where the device cannot draw
/// into `Rgba16Float` (WebGL2 without `EXT_color_buffer_float` or
/// `_half_float`) the frame's own sRGB format stands in — light past one
/// clips before the tone curve, and the curve still shapes what is under
/// it; where a format cannot be multisampled and resolved, one sample.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hdr3D {
    /// The colour target's format.
    pub format: wgpu::TextureFormat,
    /// Samples a pixel: 1 or 4.
    pub samples: u32,
}

/// Why a 3D target is not what was asked for: said once a session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HdrDegraded {
    /// The device cannot draw into `Rgba16Float`: the sRGB format instead.
    NoFloatTarget,
    /// The format or the depth format cannot be multisampled this many
    /// times, or resolved: one sample instead.
    NoMultisample(u32),
}

impl std::fmt::Display for HdrDegraded {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoFloatTarget => write!(
                f,
                "this device cannot draw into Rgba16Float; 3D is drawn in 8 bits, light past \
                 1 clipped before the tone curve"
            ),
            Self::NoMultisample(n) => {
                write!(
                    f,
                    "this device cannot draw {n}× MSAA; 3D edges are not smoothed"
                )
            }
        }
    }
}

impl Hdr3D {
    /// The HDR format, when the device can draw it.
    pub const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
    /// The samples a pixel unless `default.render.ron` says otherwise.
    pub const DEFAULT_SAMPLES: u32 = 4;

    /// The best of `Rgba16Float` with `samples` that `support` allows,
    /// `fallback` (the frame's sRGB format) standing in for the float
    /// format; and what had to give, in order.
    pub fn choose(
        support: &TargetSupport,
        fallback: wgpu::TextureFormat,
        samples: u32,
    ) -> (Self, Vec<HdrDegraded>) {
        let mut degraded = Vec::new();
        let (format, features) = if TargetSupport::renders(&support.float) {
            (Self::FORMAT, &support.float)
        } else {
            degraded.push(HdrDegraded::NoFloatTarget);
            (fallback, &support.srgb)
        };
        let samples = samples.max(1);
        let samples = if TargetSupport::multisamples(features, samples, true)
            && TargetSupport::multisamples(&support.depth, samples, false)
        {
            samples
        } else {
            degraded.push(HdrDegraded::NoMultisample(samples));
            1
        };
        (Self { format, samples }, degraded)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn features(
        usages: wgpu::TextureUsages,
        flags: wgpu::TextureFormatFeatureFlags,
    ) -> wgpu::TextureFormatFeatures {
        wgpu::TextureFormatFeatures {
            allowed_usages: usages,
            flags,
        }
    }

    fn full(format: wgpu::TextureFormat) -> wgpu::TextureFormatFeatures {
        format.guaranteed_format_features(wgpu::Features::empty())
    }

    const SRGB: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

    #[test]
    fn webgpu_rules_give_float_and_four_samples() {
        let support = TargetSupport {
            float: full(Hdr3D::FORMAT),
            srgb: full(SRGB),
            depth: full(super::super::DEPTH_FORMAT),
        };
        let (hdr, degraded) = Hdr3D::choose(&support, SRGB, 4);
        assert_eq!(
            hdr,
            Hdr3D {
                format: Hdr3D::FORMAT,
                samples: 4
            }
        );
        assert!(degraded.is_empty());
        let (one, _) = Hdr3D::choose(&support, SRGB, 1);
        assert_eq!(one.samples, 1);
    }

    #[test]
    fn webgl2_without_float_targets_draws_srgb_and_without_resolve_one_sample() {
        // WebGL2 with no EXT_color_buffer_float: Rgba16Float only sampled.
        let sampled = features(
            wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            wgpu::TextureFormatFeatureFlags::FILTERABLE,
        );
        let support = TargetSupport {
            float: sampled,
            srgb: full(SRGB),
            depth: full(super::super::DEPTH_FORMAT),
        };
        let (hdr, degraded) = Hdr3D::choose(&support, SRGB, 4);
        assert_eq!(
            hdr,
            Hdr3D {
                format: SRGB,
                samples: 4
            }
        );
        assert_eq!(degraded, [HdrDegraded::NoFloatTarget]);
        // A colour format that multisamples but cannot resolve.
        let mut unresolved = full(SRGB);
        unresolved
            .flags
            .remove(wgpu::TextureFormatFeatureFlags::MULTISAMPLE_RESOLVE);
        let support = TargetSupport {
            srgb: unresolved,
            ..support
        };
        let (hdr, degraded) = Hdr3D::choose(&support, SRGB, 4);
        assert_eq!(
            hdr,
            Hdr3D {
                format: SRGB,
                samples: 1
            }
        );
        assert_eq!(
            degraded,
            [HdrDegraded::NoFloatTarget, HdrDegraded::NoMultisample(4)]
        );
    }
}
