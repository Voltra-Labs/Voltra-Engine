//! What lights a 3D frame: one directional light and the ambient, and the
//! uniform the surface shader reads them through with the view.

use bytemuck::{Pod, Zeroable};
use glam::Vec3;

use super::view::View3D;

/// A light from infinitely far away, all its rays parallel: the sun.
///
/// `color × energy` is what the shader lights with. `energy` is the light
/// units' stored value (ARCHITECTURE "Light units"): the illuminance in lux
/// over K = 1.2 · 2¹⁵, which an exposure of EV100 15 maps to 1 — so 1.0 is
/// a usable light in simple units, and physical units (stage 142) read the
/// same number as lux through K.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DirectionalLight {
    /// The way the light travels, in the world: its entity's −Z.
    pub direction: Vec3,
    /// Linear RGB.
    pub color: [f32; 3],
    /// The light units' stored value.
    pub energy: f32,
}

/// A 3D frame's light: what [`Scene3D`](super::Scene3D) is lit by.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Lighting3D {
    /// The one directional light, if there is one (stage 139 makes it a
    /// list).
    pub directional: Option<DirectionalLight>,
    /// A uniform sky's light, linear RGB in the light units' energy: the
    /// diffuse it gives a white surface.
    pub ambient: [f32; 3],
}

impl Lighting3D {
    /// No light at all: an unlit surface still draws as it is.
    pub const DARK: Self = Self {
        directional: None,
        ambient: [0.0; 3],
    };
}

/// Group 0 of the surface pipelines, as `surface3d.wgsl`'s `Frame3D` reads
/// it: the view and the light. 144 bytes.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct FrameUniform3D {
    view_proj: [[f32; 4]; 4],
    eye: [f32; 4],
    forward: [f32; 4],
    light_direction: [f32; 4],
    light_color: [f32; 4],
    ambient: [f32; 4],
}

impl FrameUniform3D {
    /// The uniform for `view` lit by `lighting`. A light with no finite
    /// direction, or a non-finite colour or energy, lights nothing: the
    /// scene side warns about it.
    pub fn new(view: &View3D, lighting: &Lighting3D) -> Self {
        let finite3 = |c: [f32; 3]| c.map(|c| if c.is_finite() { c.max(0.0) } else { 0.0 });
        let light = lighting.directional.and_then(|light| {
            let toward = -light.direction.normalize_or_zero();
            let color = finite3(light.color);
            let energy = if light.energy.is_finite() {
                light.energy.max(0.0)
            } else {
                0.0
            };
            (toward != Vec3::ZERO).then_some((toward, color.map(|c| c * energy)))
        });
        let (toward, color, on) = match light {
            Some((toward, color)) => (toward, color, 1.0),
            None => (Vec3::ZERO, [0.0; 3], 0.0),
        };
        let ambient = finite3(lighting.ambient);
        let perspective = if view.is_orthographic() { 0.0 } else { 1.0 };
        Self {
            view_proj: view.view_projection().to_cols_array_2d(),
            eye: view.eye.extend(perspective).to_array(),
            forward: view.forward.extend(0.0).to_array(),
            light_direction: toward.extend(on).to_array(),
            light_color: [color[0], color[1], color[2], 0.0],
            ambient: [ambient[0], ambient[1], ambient[2], 0.0],
        }
    }
}

#[cfg(test)]
mod tests {
    use glam::Affine3A;

    use super::*;

    #[test]
    fn the_uniform_points_toward_the_light_and_scales_its_colour() {
        let view = View3D::perspective(Affine3A::IDENTITY, 1.0, 1.0, 0.1, None);
        let lit = FrameUniform3D::new(
            &view,
            &Lighting3D {
                directional: Some(DirectionalLight {
                    direction: Vec3::new(0.0, -2.0, 0.0),
                    color: [1.0, 0.5, 0.25],
                    energy: 2.0,
                }),
                ambient: [0.1, f32::NAN, -1.0],
            },
        );
        assert_eq!(lit.light_direction, [0.0, 1.0, 0.0, 1.0]);
        assert_eq!(lit.light_color, [2.0, 1.0, 0.5, 0.0]);
        assert_eq!(lit.ambient, [0.1, 0.0, 0.0, 0.0]);
        assert_eq!(lit.eye[3], 1.0, "a perspective view");
        let broken = FrameUniform3D::new(
            &view,
            &Lighting3D {
                directional: Some(DirectionalLight {
                    direction: Vec3::ZERO,
                    color: [1.0; 3],
                    energy: 1.0,
                }),
                ambient: [0.0; 3],
            },
        );
        assert_eq!(broken.light_direction[3], 0.0, "no direction, no light");
        let ortho = View3D::orthographic(Affine3A::IDENTITY, 1.0, 1.0, 0.1, 10.0);
        assert_eq!(FrameUniform3D::new(&ortho, &Lighting3D::DARK).eye[3], 0.0);
        assert_eq!(size_of::<FrameUniform3D>(), 144);
    }
}
