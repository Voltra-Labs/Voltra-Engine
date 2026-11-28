//! The meshes every project has without a file: `builtin:cube`, `sphere`,
//! `plane`, `capsule` and `cylinder`, generated in code.
//!
//! One metre across, as Unity's are, so a scale is a size in metres: the
//! cube, sphere, plane and cylinder fit the unit box about their origin,
//! and the capsule is a character's 2 m tall and 1 m wide (Unity's and
//! Godot's default; Bevy's `Capsule3d`). Every one carries outward
//! normals, UVs with `(0, 0)` at the image's top-left, and analytic
//! tangents along increasing U with `w = 1`: the bitangent
//! `cross(normal, tangent)` points up the image (glTF's convention), so a
//! normal map authored for glTF reads the same on these.

mod capsule;
mod cube;
mod cylinder;
mod plane;
mod revolve;
mod sphere;
#[cfg(test)]
mod tests;

use voltra_render::render3d::MeshData3D;

/// A mesh the engine generates rather than reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum BuiltinMesh {
    /// A 1 m cube about its origin.
    Cube,
    /// A sphere 1 m across: 32 segments around, 16 rings.
    Sphere,
    /// A 1 m square in the XZ plane, facing +Y: a floor.
    Plane,
    /// A capsule 2 m tall and 1 m across, upright.
    Capsule,
    /// A cylinder 1 m tall and 1 m across, upright, capped.
    Cylinder,
}

impl BuiltinMesh {
    /// Every built-in mesh, in the order a menu lists them.
    pub const ALL: [Self; 5] = [
        Self::Cube,
        Self::Sphere,
        Self::Plane,
        Self::Capsule,
        Self::Cylinder,
    ];

    /// The name after `builtin:` in a file.
    pub fn name(self) -> &'static str {
        match self {
            Self::Cube => "cube",
            Self::Sphere => "sphere",
            Self::Plane => "plane",
            Self::Capsule => "capsule",
            Self::Cylinder => "cylinder",
        }
    }

    /// The mesh named `name`, if one is.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|mesh| mesh.name() == name)
    }

    /// Its vertices and triangles.
    pub fn generate(self) -> MeshData3D {
        match self {
            Self::Cube => cube::generate(),
            Self::Sphere => sphere::generate(),
            Self::Plane => plane::generate(),
            Self::Capsule => capsule::generate(),
            Self::Cylinder => cylinder::generate(),
        }
    }
}
