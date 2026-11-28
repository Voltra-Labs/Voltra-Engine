//! `builtin:plane`, and the square face the cube is made of.

use voltra_render::glam::Vec3;
use voltra_render::render3d::{MeshData3D, Vertex3D};

/// A 1 m square in the XZ plane facing +Y, U along +X and the image's up
/// along −Z: a floor seen from above reads as its picture does.
pub(super) fn generate() -> MeshData3D {
    let mut mesh = MeshData3D::default();
    face(&mut mesh, Vec3::ZERO, Vec3::Y, Vec3::X, Vec3::NEG_Z, 0.5);
    mesh
}

/// Appends a square about `center` facing `normal`, `half` metres to each
/// side along `u` (U's direction) and `up` (the image's up): two
/// triangles, counter-clockwise seen from where `normal` points.
pub(super) fn face(
    mesh: &mut MeshData3D,
    center: Vec3,
    normal: Vec3,
    u: Vec3,
    up: Vec3,
    half: f32,
) {
    let base = mesh.vertices.len() as u32;
    for (su, sv) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
        mesh.vertices.push(Vertex3D {
            position: (center + (u * su + up * sv) * half).to_array(),
            normal: normal.to_array(),
            uv: [(su + 1.0) * 0.5, (1.0 - sv) * 0.5],
            tangent: u.extend(1.0).to_array(),
        });
    }
    mesh.indices
        .extend([base, base + 1, base + 2, base, base + 2, base + 3]);
}
