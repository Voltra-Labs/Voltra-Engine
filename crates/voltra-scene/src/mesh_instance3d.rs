//! A mesh drawn in the 3D world.

use voltra_assets::MeshPath;

/// Draws a mesh at its entity's [`Transform3D`](crate::Transform3D):
/// Godot's `MeshInstance3D`, Unity's `MeshFilter` + `MeshRenderer`.
///
/// Every instance of one mesh in view is one instanced draw. Unlit and
/// flat-tinted until surface materials (stage 137). A mesh that will not
/// load draws a magenta cube and is warned about once. With no
/// `Transform3D` it sits in its parent's frame — the world origin at the
/// root.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MeshInstance3D {
    /// The mesh: `"builtin:cube"` and its siblings, or a mesh file.
    pub mesh: MeshPath,
    /// Its colour, linear RGBA, as a [`Sprite2D`](crate::Sprite2D)'s.
    /// White, the default, is not written.
    #[serde(default = "white", skip_serializing_if = "is_white")]
    pub color: [f32; 4],
}

fn white() -> [f32; 4] {
    [1.0; 4]
}

fn is_white(color: &[f32; 4]) -> bool {
    *color == white()
}

impl Default for MeshInstance3D {
    fn default() -> Self {
        Self::new(MeshPath::default())
    }
}

impl MeshInstance3D {
    /// The registered name a scene file stores it under.
    pub const NAME: &'static str = "MeshInstance3D";

    /// `mesh`, white.
    pub fn new(mesh: impl Into<MeshPath>) -> Self {
        Self {
            mesh: mesh.into(),
            color: white(),
        }
    }

    /// Sets its colour.
    pub fn with_color(mut self, color: [f32; 4]) -> Self {
        self.color = color;
        self
    }
}

#[cfg(test)]
mod tests {
    use voltra_assets::BuiltinMesh;

    use super::*;

    #[test]
    fn a_mesh_instance_is_written_as_its_mesh_name() {
        let sphere = MeshInstance3D::new(BuiltinMesh::Sphere);
        let text = ron::to_string(&sphere).expect("serialises");
        assert_eq!(text, "(mesh:\"builtin:sphere\")");
        assert_eq!(
            ron::from_str::<MeshInstance3D>(&text).expect("parses"),
            sphere
        );
        let red = sphere.with_color([1.0, 0.0, 0.0, 1.0]);
        let text = ron::to_string(&red).expect("serialises");
        assert_eq!(ron::from_str::<MeshInstance3D>(&text).expect("parses"), red);
    }
}
