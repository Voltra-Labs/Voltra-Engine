//! A mesh drawn in the 3D world.

use voltra_assets::{AssetPath, MeshPath};

/// Draws a mesh at its entity's [`Transform3D`](crate::Transform3D):
/// Godot's `MeshInstance3D`, Unity's `MeshFilter` + `MeshRenderer`.
///
/// Every instance of one mesh and one material in view is one instanced
/// draw. Drawn through its `material` — a `.material.ron` of `domain:
/// Surface` — or, with none, the default white standard (a dielectric of
/// roughness 0.5), lit by the world's `DirectionalLight3D`s and
/// `Environment3D`, the eight point and spot lights that matter most to
/// it, on its `light_layers`. A mesh that will not load draws a magenta cube and is
/// warned about once; a material that will not, the default, its error
/// logged once. With no `Transform3D` it sits in its parent's frame — the
/// world origin at the root.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MeshInstance3D {
    /// The mesh: `"builtin:cube"` and its siblings, or a mesh file.
    pub mesh: MeshPath,
    /// Its colour, linear RGBA, as a [`Sprite2D`](crate::Sprite2D)'s:
    /// multiplies the material's base colour. White, the default, is not
    /// written.
    #[serde(default = "white", skip_serializing_if = "is_white")]
    pub color: [f32; 4],
    /// The surface material, a `.material.ron` from the asset root; `None`,
    /// the default and not written, is the default white standard.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub material: Option<AssetPath>,
    /// The light layers it is lit on, as a
    /// [`Sprite2D`](crate::Sprite2D)'s `light_layers`: a light lights it
    /// only if its `layers` share a bit with these. The first layer — the
    /// default, and not written — is lit by every light that has not been
    /// told otherwise.
    #[serde(
        default = "crate::light::default_light_layers",
        skip_serializing_if = "crate::light::is_default_light_layers"
    )]
    pub light_layers: u32,
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
            material: None,
            light_layers: crate::light::default_light_layers(),
        }
    }

    /// Draws it through the surface material at `path`.
    pub fn with_material(mut self, path: AssetPath) -> Self {
        self.material = Some(path);
        self
    }

    /// Sets its colour.
    pub fn with_color(mut self, color: [f32; 4]) -> Self {
        self.color = color;
        self
    }
}

/// Loads the material every mesh in `world` names, and the images each
/// surface material names — before a frame draws them, so the draw list
/// and the frame read the stores without changing them. A hash lookup for
/// what is loaded already.
///
/// A render texture named as a surface's image is not drawn yet (its slot
/// stays empty): said once per path in `warned`.
pub fn resolve_world(
    world: &voltra_ecs::World,
    materials: &mut voltra_assets::Materials,
    textures: &mut voltra_assets::Textures,
    device: &voltra_render::wgpu::Device,
    queue: &voltra_render::wgpu::Queue,
    warned: &mut std::collections::HashSet<AssetPath>,
) {
    for (_, instance) in world.query::<MeshInstance3D>() {
        let Some(path) = &instance.material else {
            continue;
        };
        let handle = materials.load(path);
        let Some(surface) = materials.get(handle).and_then(|m| m.surface()) else {
            continue;
        };
        for image in surface.textures.iter() {
            if voltra_assets::textures::is_render_texture(image.as_str()) {
                if warned.insert(image.clone()) {
                    log::warn!(
                        "the material {} names the render texture {} as an image: a 3D surface \
                         showing a camera's picture is not drawn yet, so the slot stays empty",
                        path.as_str(),
                        image.as_str()
                    );
                }
                continue;
            }
            textures.load(device, queue, image);
        }
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
        let gold =
            red.with_material(AssetPath::new("materials/gold.material.ron").expect("a path"));
        let text = ron::to_string(&gold).expect("serialises");
        assert!(
            text.contains("material:Some(Path(\"materials/gold.material.ron\"))"),
            "{text}"
        );
        assert_eq!(
            ron::from_str::<MeshInstance3D>(&text).expect("parses"),
            gold
        );
    }
}
