//! `builtin:capsule`: two hemispheres with a cylinder between.

use std::f32::consts::FRAC_PI_2;

use voltra_render::render3d::MeshData3D;

use super::revolve::{revolve, Row};

const SEGMENTS: u32 = 32;
/// Rings in each hemisphere.
const RINGS: u32 = 8;
const RADIUS: f32 = 0.5;
/// Half the straight part's height: 2 m tall in all.
const HALF_LENGTH: f32 = 0.5;

/// A capsule 2 m tall and 1 m across about the origin. V runs from the top
/// to the bottom by arc length, so a texture is not stretched on the
/// straight part.
pub(super) fn generate() -> MeshData3D {
    let arc = RADIUS * FRAC_PI_2;
    let total = 2.0 * arc + 2.0 * HALF_LENGTH;
    let mut profile = Vec::with_capacity(2 * (RINGS as usize + 1));
    for (centre, from, along) in [
        (HALF_LENGTH, 0.0, 0.0),
        (-HALF_LENGTH, FRAC_PI_2, arc + 2.0 * HALF_LENGTH),
    ] {
        for k in 0..=RINGS {
            let step = k as f32 / RINGS as f32 * FRAC_PI_2;
            let angle = from + step;
            let (sin, cos) = angle.sin_cos();
            let pole = (from == 0.0 && k == 0) || (from > 0.0 && k == RINGS);
            profile.push(Row {
                radius: if pole { 0.0 } else { RADIUS * sin },
                y: centre + RADIUS * cos,
                normal: (sin, cos),
                v: (along + RADIUS * step) / total,
            });
        }
    }
    let mut mesh = MeshData3D::default();
    revolve(&mut mesh, &profile, SEGMENTS);
    mesh
}
