//! The standard surface's numbers: glTF 2.0's metallic-roughness inputs.

use glam::{Affine2, Vec2};
use serde::{Deserialize, Serialize};

/// How a surface's alpha is used: glTF's `alphaMode`.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub enum AlphaMode {
    /// Alpha is ignored: the surface is opaque.
    #[default]
    Opaque,
    /// Cut out where alpha is under the cutoff, opaque elsewhere; under
    /// MSAA the edge is smoothed by alpha to coverage.
    Mask(f32),
    /// Blended over what is behind, drawn after the opaque, back to front,
    /// depth-tested and not written.
    Blend,
}

impl AlphaMode {
    /// glTF's default cutoff, 0.5.
    pub const DEFAULT_CUTOFF: f32 = 0.5;

    /// The cutoff a mask cuts at, kept within `0..=1`; `None` for the
    /// other modes.
    pub fn cutoff(self) -> Option<f32> {
        match self {
            Self::Mask(cutoff) if cutoff.is_finite() => Some(cutoff.clamp(0.0, 1.0)),
            Self::Mask(_) => Some(Self::DEFAULT_CUTOFF),
            _ => None,
        }
    }
}

/// Moves a surface's texture coordinates before every texture is sampled:
/// `KHR_texture_transform` — scaled, then rotated `rotation` radians
/// counter-clockwise, then offset. The identity changes nothing.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct UvTransform {
    /// Added last, in texture coordinates (1 is the whole image).
    #[serde(default)]
    pub offset: [f32; 2],
    /// Multiplies first: 2 repeats the image twice across.
    #[serde(default = "one")]
    pub scale: [f32; 2],
    /// Radians, counter-clockwise, about the coordinates' origin.
    #[serde(default)]
    pub rotation: f32,
}

fn one() -> [f32; 2] {
    [1.0, 1.0]
}

impl UvTransform {
    /// No change.
    pub const IDENTITY: Self = Self {
        offset: [0.0, 0.0],
        scale: [1.0, 1.0],
        rotation: 0.0,
    };

    /// The matrix the shader applies, as `KHR_texture_transform` writes it
    /// (translation · rotation · scale, its rotation turning the other way
    /// from glam's: `u' = cos·u + sin·v`, `v' = −sin·u + cos·v`). A
    /// number that is not finite counts as the identity's.
    pub fn matrix(&self) -> Affine2 {
        let finite = |v: [f32; 2], or: [f32; 2]| {
            let v = Vec2::from(v);
            if v.is_finite() {
                v
            } else {
                Vec2::from(or)
            }
        };
        let rotation = if self.rotation.is_finite() {
            self.rotation
        } else {
            0.0
        };
        Affine2::from_scale_angle_translation(
            finite(self.scale, [1.0, 1.0]),
            -rotation,
            finite(self.offset, [0.0, 0.0]),
        )
    }

    /// Whether it changes nothing.
    pub fn is_identity(&self) -> bool {
        *self == Self::IDENTITY
    }
}

impl Default for UvTransform {
    fn default() -> Self {
        Self::IDENTITY
    }
}

/// A standard surface's numbers: what glTF 2.0's metallic-roughness
/// material holds besides its images, shaded by Filament's BRDF. Each
/// factor multiplies its texture where the material has one (see
/// [`SurfaceTextures`](super::SurfaceTextures)).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StandardSurface {
    /// Linear RGBA; multiplies the base-colour texture and the instance's
    /// tint.
    pub base_color: [f32; 4],
    /// 0 a dielectric, 1 a metal; multiplies the metallic-roughness
    /// texture's blue channel.
    pub metallic: f32,
    /// Perceptual roughness, 0 a mirror, 1 chalk; multiplies the
    /// metallic-roughness texture's green channel.
    pub roughness: f32,
    /// How far the normal map bends the normal: its X and Y scaled.
    pub normal_scale: f32,
    /// How much the occlusion texture's red channel darkens the ambient.
    pub occlusion_strength: f32,
    /// Linear RGB light the surface gives off; multiplies the emissive
    /// texture.
    pub emissive: [f32; 3],
    /// Multiplies `emissive`: `KHR_materials_emissive_strength`, in the
    /// light units' energy.
    pub emissive_strength: f32,
    /// What its alpha does.
    pub alpha: AlphaMode,
    /// Both faces drawn, the back lit with its normal turned round.
    pub double_sided: bool,
    /// Its base colour drawn as it is, no light: `KHR_materials_unlit`.
    pub unlit: bool,
    /// Moves the texture coordinates of every texture.
    pub uv_transform: UvTransform,
}

impl StandardSurface {
    /// White, a dielectric of roughness 0.5 (Bevy's default; Godot's is 1,
    /// Unity's smoothness 0.5): what a mesh with no material draws as.
    pub const DEFAULT: Self = Self {
        base_color: [1.0; 4],
        metallic: 0.0,
        roughness: 0.5,
        normal_scale: 1.0,
        occlusion_strength: 1.0,
        emissive: [0.0; 3],
        emissive_strength: 1.0,
        alpha: AlphaMode::Opaque,
        double_sided: false,
        unlit: false,
        uv_transform: UvTransform::IDENTITY,
    };

    /// White and unlit: an instance's tint drawn as it is.
    pub const UNLIT: Self = Self {
        unlit: true,
        ..Self::DEFAULT
    };
}

impl Default for StandardSurface {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_uv_transform_offsets_scales_and_turns_as_khr_texture_transform() {
        let moved = UvTransform {
            offset: [0.25, 0.0],
            ..UvTransform::IDENTITY
        };
        let p = moved.matrix().transform_point2(Vec2::new(0.5, 0.5));
        assert!(p.abs_diff_eq(Vec2::new(0.75, 0.5), 1e-6), "{p}");
        // A quarter turn: u' = cos·u + sin·v = v, v' = −sin·u + cos·v = −u.
        let turned = UvTransform {
            rotation: std::f32::consts::FRAC_PI_2,
            ..UvTransform::IDENTITY
        };
        let p = turned.matrix().transform_point2(Vec2::new(1.0, 0.0));
        assert!(p.abs_diff_eq(Vec2::new(0.0, -1.0), 1e-6), "{p}");
        let scaled = UvTransform {
            scale: [2.0, 3.0],
            offset: [0.5, 0.5],
            rotation: 0.0,
        };
        let p = scaled.matrix().transform_point2(Vec2::new(1.0, 1.0));
        assert!(p.abs_diff_eq(Vec2::new(2.5, 3.5), 1e-6), "{p}");
        let broken = UvTransform {
            offset: [f32::NAN, 0.0],
            scale: [f32::INFINITY, 1.0],
            rotation: f32::NAN,
        };
        assert_eq!(broken.matrix(), Affine2::IDENTITY);
    }

    #[test]
    fn a_mask_cutoff_stays_within_zero_and_one() {
        assert_eq!(AlphaMode::Mask(0.3).cutoff(), Some(0.3));
        assert_eq!(AlphaMode::Mask(2.0).cutoff(), Some(1.0));
        assert_eq!(AlphaMode::Mask(f32::NAN).cutoff(), Some(0.5));
        assert_eq!(AlphaMode::Blend.cutoff(), None);
    }
}
