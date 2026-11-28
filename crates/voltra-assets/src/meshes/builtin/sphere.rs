//! `builtin:sphere`: a UV sphere.

use std::f32::consts::PI;

use voltra_render::render3d::MeshData3D;

use super::revolve::{revolve, Row};

const SEGMENTS: u32 = 32;
const RINGS: u32 = 16;
const RADIUS: f32 = 0.5;

/// A sphere 1 m across about the origin, V from the north pole down.
pub(super) fn generate() -> MeshData3D {
    let profile: Vec<Row> = (0..=RINGS)
        .map(|i| {
            let v = i as f32 / RINGS as f32;
            let (sin, cos) = (v * PI).sin_cos();
            Row {
                // Exactly zero at the poles, where `sin` is a rounding off.
                radius: if i == 0 || i == RINGS {
                    0.0
                } else {
                    RADIUS * sin
                },
                y: RADIUS * cos,
                normal: (sin, cos),
                v,
            }
        })
        .collect();
    let mut mesh = MeshData3D::default();
    revolve(&mut mesh, &profile, SEGMENTS);
    mesh
}
