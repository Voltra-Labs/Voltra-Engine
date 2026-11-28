//! `builtin:cube`: six faces of four vertices, each with its face's normal.

use voltra_render::glam::Vec3;
use voltra_render::render3d::MeshData3D;

use super::plane::face;

/// A 1 m cube about the origin.
pub(super) fn generate() -> MeshData3D {
    let mut mesh = MeshData3D::default();
    // Each face's normal, then the way U runs and the way the image's up
    // runs on it; `normal × u = up` keeps every face counter-clockwise
    // from outside and its bitangent up the image.
    for (normal, u, up) in [
        (Vec3::X, Vec3::NEG_Z, Vec3::Y),
        (Vec3::NEG_X, Vec3::Z, Vec3::Y),
        (Vec3::Y, Vec3::X, Vec3::NEG_Z),
        (Vec3::NEG_Y, Vec3::X, Vec3::Z),
        (Vec3::Z, Vec3::X, Vec3::Y),
        (Vec3::NEG_Z, Vec3::NEG_X, Vec3::Y),
    ] {
        face(&mut mesh, normal * 0.5, normal, u, up, 0.5);
    }
    mesh
}
