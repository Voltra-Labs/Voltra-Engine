//! Which image files a scene file's records name, read without a world.
//!
//! What a loader needs to decode a scene's textures on a worker before the
//! entities naming them arrive: the decode is most of a texture's cost, and
//! the entity's own resolve then finds the texture already in the store.
//! The same four components the loop resolves textures for (`spawning.rs`
//! in `voltra-core`); one this list misses is still drawn — its texture is
//! decoded where it is resolved, on the frame, as before.

use std::collections::HashSet;
use std::path::Path;

use voltra_assets::{AssetPath, MaterialDomain};

use super::save::EntityRecord;
use crate::{MeshInstance3D, ParticleEmitter, PointLight2D, Sprite2D, Tilemap};

/// Every texture `records` name, each once, in the order first named.
/// A component that will not read names nothing here; spawning it is what
/// reports the error.
pub fn texture_paths<'a>(records: impl IntoIterator<Item = &'a EntityRecord>) -> Vec<AssetPath> {
    let mut seen = HashSet::new();
    let mut paths = Vec::new();
    let mut add = |path: Option<AssetPath>| {
        if let Some(path) = path {
            if seen.insert(path.clone()) {
                paths.push(path);
            }
        }
    };
    for record in records {
        for (name, value) in &record.components {
            match name.as_str() {
                Sprite2D::NAME => {
                    if let Ok(sprite) = value.into_rust::<Sprite2D>() {
                        add(sprite.texture);
                        add(sprite.normal_map);
                    }
                }
                Tilemap::NAME => {
                    if let Ok(map) = value.into_rust::<Tilemap>() {
                        add(map.texture);
                        add(map.normal_map);
                    }
                }
                ParticleEmitter::NAME => {
                    if let Ok(emitter) = value.into_rust::<ParticleEmitter>() {
                        add(emitter.texture);
                    }
                }
                PointLight2D::NAME => {
                    if let Ok(light) = value.into_rust::<PointLight2D>() {
                        add(light.cookie);
                    }
                }
                _ => {}
            }
        }
    }
    paths
}

/// Every image the surface materials of `records`' meshes name, each
/// once: each material file read from `root` (once), its `standard`
/// block's texture paths listed, colour and data alike. A material that
/// will not read, or is a sprite's, names nothing here; the frame says why
/// when it draws the mesh.
pub fn surface_texture_paths<'a>(
    records: impl IntoIterator<Item = &'a EntityRecord>,
    root: &Path,
) -> Vec<AssetPath> {
    let mut materials = HashSet::new();
    let mut seen = HashSet::new();
    let mut paths = Vec::new();
    for record in records {
        let Some(value) = record.components.get(MeshInstance3D::NAME) else {
            continue;
        };
        let Some(material) = value
            .into_rust::<MeshInstance3D>()
            .ok()
            .and_then(|mesh| mesh.material)
        else {
            continue;
        };
        if !materials.insert(material.clone()) {
            continue;
        }
        let Ok(file) = voltra_assets::read_material_file(root, &material) else {
            continue;
        };
        if file.domain != MaterialDomain::Surface {
            continue;
        }
        let Ok(textures) = file.surface_texture_paths() else {
            continue;
        };
        for image in textures.iter() {
            if seen.insert(image.clone()) {
                paths.push(image.clone());
            }
        }
    }
    paths
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::{to_scene_file, ComponentRegistry};
    use crate::{Name, SceneId};
    use voltra_ecs::World;

    fn path(raw: &str) -> AssetPath {
        AssetPath::new(raw).expect("valid")
    }

    #[test]
    fn every_texture_named_is_listed_once() {
        let mut world = World::new();
        for texture in ["a.png", "b.png", "a.png"] {
            let entity = world.spawn();
            world.insert(entity, SceneId::new());
            let sprite = Sprite2D {
                texture: Some(path(texture)),
                ..Sprite2D::default()
            };
            world.insert(entity, sprite);
        }
        let plain = world.spawn();
        world.insert(plain, SceneId::new());
        world.insert(plain, Name::new("no texture"));
        let file = to_scene_file(&world, &ComponentRegistry::with_defaults()).expect("saves");

        let mut found = texture_paths(&file.entities);
        found.sort_by(|a, b| a.as_str().cmp(b.as_str()));

        assert_eq!(found, vec![path("a.png"), path("b.png")]);
    }

    #[test]
    fn a_mesh_s_surface_material_names_its_images_once() {
        let root = voltra_testkit::scratch_root();
        std::fs::create_dir_all(root.join("m")).expect("a folder");
        std::fs::write(
            root.join("m/brick.material.ron"),
            r#"(version: 1, domain: Surface, standard: (
                base_color_texture: "t/brick.png", normal_texture: "t/brick_n.png"))"#,
        )
        .expect("written");
        let mut world = World::new();
        for _ in 0..2 {
            let entity = world.spawn();
            world.insert(entity, SceneId::new());
            world.insert(
                entity,
                MeshInstance3D::new(voltra_assets::BuiltinMesh::Cube)
                    .with_material(path("m/brick.material.ron")),
            );
        }
        let missing = world.spawn();
        world.insert(missing, SceneId::new());
        world.insert(
            missing,
            MeshInstance3D::new(voltra_assets::BuiltinMesh::Cube)
                .with_material(path("m/none.material.ron")),
        );
        let registry = ComponentRegistry::with_defaults();
        let file = to_scene_file(&world, &registry).expect("saved");
        assert_eq!(
            surface_texture_paths(&file.entities, &root),
            [path("t/brick.png"), path("t/brick_n.png")]
        );
    }
}
