//! Which image files a scene file's records name, read without a world.
//!
//! What a loader needs to decode a scene's textures on a worker before the
//! entities naming them arrive: the decode is most of a texture's cost, and
//! the entity's own resolve then finds the texture already in the store.
//! The same four components the loop resolves textures for (`spawning.rs`
//! in `voltra-core`); one this list misses is still drawn — its texture is
//! decoded where it is resolved, on the frame, as before.

use std::collections::HashSet;

use voltra_assets::AssetPath;

use super::save::EntityRecord;
use crate::{ParticleEmitter, PointLight2D, Sprite, Tilemap};

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
                Sprite::NAME => {
                    if let Ok(sprite) = value.into_rust::<Sprite>() {
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
            let sprite = Sprite {
                texture: Some(path(texture)),
                ..Sprite::default()
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
}
