//! A surface of revolution about Y from a profile: the sphere, the
//! capsule and the cylinder's side.

use std::f32::consts::TAU;

use voltra_render::glam::Vec3;
use voltra_render::render3d::{MeshData3D, Vertex3D};

/// One row of a profile, top to bottom.
#[derive(Debug, Clone, Copy)]
pub(super) struct Row {
    /// Distance from the axis.
    pub radius: f32,
    /// Height.
    pub y: f32,
    /// The normal in the profile's plane: out from the axis, and up.
    pub normal: (f32, f32),
    /// The texture's V at this row, 0 at the top.
    pub v: f32,
}

/// Appends `profile` turned once about Y in `segments` steps. U runs once
/// around, from +Z towards +X (the front's U rightwards seen from +Z),
/// with a seam of doubled vertices; a row on the axis (a pole) gets no
/// degenerate triangle.
pub(super) fn revolve(mesh: &mut MeshData3D, profile: &[Row], segments: u32) {
    let base = mesh.vertices.len() as u32;
    let columns = segments + 1;
    for row in profile {
        for j in 0..columns {
            let u = j as f32 / segments as f32;
            let (sin, cos) = (u * TAU).sin_cos();
            let (out, up) = row.normal;
            mesh.vertices.push(Vertex3D {
                position: [row.radius * sin, row.y, row.radius * cos],
                normal: Vec3::new(out * sin, up, out * cos).normalize().to_array(),
                uv: [u, row.v],
                // d(position)/du, normalised: defined at the poles too.
                tangent: [cos, 0.0, -sin, 1.0],
            });
        }
    }
    for (i, pair) in profile.windows(2).enumerate() {
        let i = i as u32;
        for j in 0..segments {
            let a = base + i * columns + j;
            let b = a + columns;
            let (c, d) = (b + 1, a + 1);
            if pair[1].radius > 0.0 {
                mesh.indices.extend([a, b, c]);
            }
            if pair[0].radius > 0.0 {
                mesh.indices.extend([a, c, d]);
            }
        }
    }
}
