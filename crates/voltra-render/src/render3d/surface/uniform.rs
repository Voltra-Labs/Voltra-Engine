//! A standard surface's numbers as `surface3d.wgsl`'s `Surface3D` reads
//! them.

use bytemuck::{Pod, Zeroable};

use super::standard::StandardSurface;

/// One material's uniform block: 96 bytes.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct SurfaceUniform {
    base_color: [f32; 4],
    emissive: [f32; 4],
    factors: [f32; 4],
    flags: [f32; 4],
    uv_x: [f32; 4],
    uv_y: [f32; 4],
}

impl SurfaceUniform {
    /// The block for `surface`, `normal_map` saying whether a normal map is
    /// bound. A number that is not finite is its default's, so a broken
    /// file cannot write NaN into every pixel it covers.
    pub fn new(surface: &StandardSurface, normal_map: bool) -> Self {
        let default = StandardSurface::DEFAULT;
        let or = |v: f32, d: f32| if v.is_finite() { v } else { d };
        let base_color: [f32; 4] =
            std::array::from_fn(|i| or(surface.base_color[i], default.base_color[i]));
        let strength = or(surface.emissive_strength, 1.0).max(0.0);
        let emissive: [f32; 3] =
            std::array::from_fn(|i| or(surface.emissive[i], 0.0).max(0.0) * strength);
        let uv = surface.uv_transform.matrix();
        let (x, y) = (uv.matrix2.x_axis, uv.matrix2.y_axis);
        let t = uv.translation;
        Self {
            base_color,
            emissive: [emissive[0], emissive[1], emissive[2], 0.0],
            factors: [
                or(surface.metallic, default.metallic).clamp(0.0, 1.0),
                or(surface.roughness, default.roughness).clamp(0.0, 1.0),
                or(surface.normal_scale, 1.0),
                or(surface.occlusion_strength, 1.0).clamp(0.0, 1.0),
            ],
            flags: [
                surface.alpha.cutoff().unwrap_or(0.0),
                f32::from(u8::from(surface.unlit)),
                f32::from(u8::from(normal_map)),
                0.0,
            ],
            // Rows of the 2 × 3 affine: u' = x.x·u + y.x·v + t.x.
            uv_x: [x.x, y.x, t.x, 0.0],
            uv_y: [x.y, y.y, t.y, 0.0],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render3d::{AlphaMode, UvTransform};

    #[test]
    fn the_block_holds_the_numbers_and_refuses_nan() {
        let surface = StandardSurface {
            base_color: [0.5, f32::NAN, 1.0, 1.0],
            metallic: 2.0,
            roughness: f32::NAN,
            emissive: [1.0, 0.5, 0.0],
            emissive_strength: 4.0,
            alpha: AlphaMode::Mask(0.25),
            uv_transform: UvTransform {
                offset: [0.25, 0.5],
                ..UvTransform::IDENTITY
            },
            ..StandardSurface::DEFAULT
        };
        let block = SurfaceUniform::new(&surface, true);
        assert_eq!(block.base_color, [0.5, 1.0, 1.0, 1.0]);
        assert_eq!(block.factors[..2], [1.0, 0.5]);
        assert_eq!(block.emissive, [4.0, 2.0, 0.0, 0.0]);
        assert_eq!(block.flags, [0.25, 0.0, 1.0, 0.0]);
        assert_eq!(block.uv_x, [1.0, 0.0, 0.25, 0.0]);
        assert_eq!(block.uv_y, [0.0, 1.0, 0.5, 0.0]);
        assert_eq!(size_of::<SurfaceUniform>(), 96);
    }
}
