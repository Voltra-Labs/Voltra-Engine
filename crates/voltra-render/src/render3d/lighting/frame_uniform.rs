//! The frame's view, ambient, sky and fog, as `frame3d.wgsl`'s `Frame3D`
//! reads it: group 0, binding 0, shared by the surface and the sky.

use bytemuck::{Pod, Zeroable};
use glam::Mat4;

use super::environment::SkyGradient;
use super::Lighting3D;
use crate::render3d::view::View3D;

/// Group 0 binding 0 of the surface and sky pipelines: the view, its
/// inverse (where the sky's rays come from), the ambient and the time, the
/// fog and the sky. 256 bytes. The lights are their own block
/// ([`LightsUniform3D`](super::LightsUniform3D), binding 1).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct FrameUniform3D {
    view_proj: [[f32; 4]; 4],
    inverse_view_proj: [[f32; 4]; 4],
    eye: [f32; 4],
    forward: [f32; 4],
    ambient: [f32; 4],
    fog_color: [f32; 4],
    fog: [f32; 4],
    sky_top: [f32; 4],
    sky_horizon: [f32; 4],
    sky_bottom: [f32; 4],
}

impl FrameUniform3D {
    /// Where the time a surface reads wraps, in seconds: an hour, as
    /// Godot's `TIME` and Bevy's `globals.time` wrap. Under it an `f32`
    /// keeps a quarter of a millisecond.
    pub const TIME_WRAP: f32 = 3600.0;

    /// The uniform for `view` lit by `lighting` at `time` seconds (wrapped
    /// at [`TIME_WRAP`](Self::TIME_WRAP); not finite, 0), its target
    /// cleared to `clear` (linear, premultiplied).
    ///
    /// The sky the fog's tint reads is `lighting`'s, else the clear colour
    /// where it is opaque, else the fog's own colour — an overlay shows
    /// what drew before it, which the fog cannot know, so its tint does
    /// nothing. Broken numbers draw as nothing rather than as NaN.
    pub fn new(view: &View3D, lighting: &Lighting3D, clear: [f32; 4], time: f32) -> Self {
        let finite3 = |c: [f32; 3]| c.map(|c| if c.is_finite() { c.max(0.0) } else { 0.0 });
        let ambient = finite3(lighting.ambient);
        let perspective = if view.is_orthographic() { 0.0 } else { 1.0 };
        let fog = lighting.fog.map(|fog| fog.sanitized());
        let fog_color = fog.map_or([0.0; 3], |fog| fog.color);
        let sky = match lighting.sky {
            Some(sky) => sky.sanitized(),
            None if clear[3] > 0.0 && clear.iter().all(|c| c.is_finite()) => SkyGradient::flat(
                finite3([clear[0], clear[1], clear[2]].map(|c| c / clear[3])),
            ),
            None => SkyGradient::flat(fog_color),
        };
        let view_proj = view.view_projection();
        let inverse = view_proj.inverse();
        let inverse = if inverse.is_finite() {
            inverse
        } else {
            Mat4::IDENTITY
        };
        let rgb = |c: [f32; 3], w: f32| [c[0], c[1], c[2], w];
        Self {
            view_proj: view_proj.to_cols_array_2d(),
            inverse_view_proj: inverse.to_cols_array_2d(),
            eye: view.eye.extend(perspective).to_array(),
            forward: view.forward.extend(0.0).to_array(),
            ambient: rgb(
                ambient,
                if time.is_finite() {
                    time.rem_euclid(Self::TIME_WRAP)
                } else {
                    0.0
                },
            ),
            fog_color: rgb(fog_color, fog.map_or(0.0, |fog| fog.sky_tint)),
            fog: fog.map_or([0.0; 4], |fog| {
                [fog.density, fog.height, fog.height_falloff, 0.0]
            }),
            sky_top: rgb(sky.top, 0.0),
            sky_horizon: rgb(sky.horizon, 0.0),
            sky_bottom: rgb(sky.bottom, 0.0),
        }
    }
}

#[cfg(test)]
mod tests {
    use glam::{Affine3A, Vec3, Vec4};

    use super::*;
    use crate::render3d::Fog;

    #[test]
    fn the_uniform_holds_the_view_the_time_the_fog_and_the_sky() {
        let view = View3D::perspective(Affine3A::IDENTITY, 1.0, 1.0, 0.1, None);
        let fog = Fog {
            color: [0.5, 0.6, 0.7],
            density: 0.02,
            height: 1.0,
            height_falloff: f32::NAN,
            sky_tint: 2.0,
        };
        let lit = FrameUniform3D::new(
            &view,
            &Lighting3D {
                ambient: [0.1, f32::NAN, -1.0],
                fog: Some(fog),
                ..Lighting3D::DARK
            },
            [0.2, 0.4, 0.6, 1.0],
            3601.5,
        );
        assert_eq!(
            lit.ambient,
            [0.1, 0.0, 0.0, 1.5],
            "the time wrapped at an hour"
        );
        assert_eq!(lit.eye[3], 1.0, "a perspective view");
        assert_eq!(lit.fog_color, [0.5, 0.6, 0.7, 1.0], "the tint kept to 1");
        assert_eq!(lit.fog, [0.02, 1.0, 0.0, 0.0], "a NaN falloff is 0");
        assert_eq!(lit.sky_horizon, [0.2, 0.4, 0.6, 0.0], "no sky: the clear");
        // The inverse takes the near plane's centre back in front of the eye.
        let inverse = Mat4::from_cols_array_2d(&lit.inverse_view_proj);
        let near = inverse * Vec4::new(0.0, 0.0, 1.0, 1.0);
        let near = near.truncate() / near.w;
        assert!(near.abs_diff_eq(Vec3::new(0.0, 0.0, -0.1), 1e-5), "{near}");
        // An overlay's clear is transparent: the fog's own colour stands in.
        let overlay = FrameUniform3D::new(
            &view,
            &Lighting3D {
                fog: Some(fog),
                ..Lighting3D::DARK
            },
            [0.0; 4],
            f32::NAN,
        );
        assert_eq!(overlay.sky_top, [0.5, 0.6, 0.7, 0.0]);
        assert_eq!(overlay.ambient[3], 0.0, "no time, 0");
        let ortho = View3D::orthographic(Affine3A::IDENTITY, 1.0, 1.0, 0.1, 10.0);
        assert_eq!(
            FrameUniform3D::new(&ortho, &Lighting3D::DARK, [0.0; 4], 0.0).eye[3],
            0.0
        );
        assert_eq!(size_of::<FrameUniform3D>(), 256);
    }
}
