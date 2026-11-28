//! `builtin:cylinder`: a side of revolution and two flat caps.

use std::f32::consts::TAU;

use voltra_render::glam::Vec3;
use voltra_render::render3d::{MeshData3D, Vertex3D};

use super::revolve::{revolve, Row};

const SEGMENTS: u32 = 32;
const RADIUS: f32 = 0.5;
const HALF_HEIGHT: f32 = 0.5;

/// A cylinder 1 m tall and 1 m across about the origin. The side takes the
/// whole texture; each cap maps it flat, seen from outside, as the cube's
/// top and bottom do.
pub(super) fn generate() -> MeshData3D {
    let mut mesh = MeshData3D::default();
    let side = [HALF_HEIGHT, -HALF_HEIGHT].map(|y| Row {
        radius: RADIUS,
        y,
        normal: (1.0, 0.0),
        v: if y > 0.0 { 0.0 } else { 1.0 },
    });
    revolve(&mut mesh, &side, SEGMENTS);
    cap(&mut mesh, HALF_HEIGHT, Vec3::Y);
    cap(&mut mesh, -HALF_HEIGHT, Vec3::NEG_Y);
    mesh
}

/// A disc at height `y` facing `normal` (±Y): U along +X, the image's up
/// along −Z on the top and +Z underneath.
fn cap(mesh: &mut MeshData3D, y: f32, normal: Vec3) {
    let up_z = -normal.y;
    let vertex = |x: f32, z: f32| Vertex3D {
        position: [x, y, z],
        normal: normal.to_array(),
        uv: [0.5 + x / (2.0 * RADIUS), 0.5 - up_z * z / (2.0 * RADIUS)],
        tangent: [1.0, 0.0, 0.0, 1.0],
    };
    let centre = mesh.vertices.len() as u32;
    mesh.vertices.push(vertex(0.0, 0.0));
    for j in 0..=SEGMENTS {
        let (sin, cos) = (j as f32 / SEGMENTS as f32 * TAU).sin_cos();
        mesh.vertices.push(vertex(RADIUS * sin, RADIUS * cos));
    }
    for j in 0..SEGMENTS {
        let (a, b) = (centre + 1 + j, centre + 2 + j);
        // Going round from +Z towards +X is counter-clockwise from above.
        if normal.y > 0.0 {
            mesh.indices.extend([centre, a, b]);
        } else {
            mesh.indices.extend([centre, b, a]);
        }
    }
}
