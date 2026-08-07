//! Reading a scene file back into a world.

use std::path::Path;

use voltra_ecs::World;

use super::error::SceneError;
use super::registry::ComponentRegistry;
use super::save::{SceneFile, VERSION};
#[cfg(test)]
use crate::scene_id::SceneId;
use crate::scene_id::UnknownComponents;

/// Spawns every entity in `file` into `world`.
///
/// Adds rather than replaces. Whether to clear first is the caller's decision,
/// and the editor's menu is where that belongs.
pub fn from_scene_file(
    file: &SceneFile,
    registry: &ComponentRegistry,
    world: &mut World,
) -> Result<(), SceneError> {
    if file.version != VERSION {
        return Err(SceneError::UnsupportedVersion {
            found: file.version,
            supported: VERSION,
        });
    }

    for record in &file.entities {
        let entity = world.spawn();
        world.insert(entity, record.id);

        let mut unknown = UnknownComponents::default();

        for (name, value) in &record.components {
            match registry.load_one(world, entity, name, value) {
                // Registered: a failure here is ours, and it propagates. The
                // name says this build understands the component, so data it
                // cannot read is broken rather than foreign.
                Some(result) => result?,
                // Not registered: keep it verbatim so saving writes it back.
                // Dropping it is how a build silently deletes work done by a
                // build that knew more.
                None => {
                    log::warn!("scene contains unknown component `{name}`; keeping it unread");
                    unknown.0.insert(name.clone(), value.clone());
                }
            }
        }

        if !unknown.0.is_empty() {
            world.insert(entity, unknown);
        }
    }

    log::info!("loaded {} entities", file.entities.len());
    Ok(())
}

/// Reads `path` and spawns its entities into `world`.
pub fn load(
    path: &Path,
    registry: &ComponentRegistry,
    world: &mut World,
) -> Result<(), SceneError> {
    let text = std::fs::read_to_string(path)?;
    let file: SceneFile = ron::from_str(&text).map_err(SceneError::Parse)?;
    from_scene_file(&file, registry, world)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::save::{save, to_scene_file};
    use crate::{Sprite, Transform};
    use voltra_render::glam::Vec2;

    fn source_world() -> World {
        let mut world = World::new();
        for i in 0..3 {
            let e = world.spawn();
            world.insert(e, SceneId::new());
            world.insert(
                e,
                Transform::from_translation(Vec2::new(i as f32, -1.0)).with_scale(Vec2::splat(0.5)),
            );
            world.insert(e, Sprite::new([1.0, 0.0, 0.0, 1.0]).with_sort_order(i));
        }
        world
    }

    #[test]
    fn a_world_survives_a_round_trip() {
        let registry = ComponentRegistry::with_defaults();
        let original = source_world();
        let file = to_scene_file(&original, &registry).expect("saving cannot fail here");

        let mut loaded = World::new();
        from_scene_file(&file, &registry, &mut loaded).expect("loading cannot fail here");

        let mut before: Vec<_> = original
            .query::<SceneId>()
            .map(|(e, id)| {
                (
                    *id,
                    original.get::<Transform>(e).copied(),
                    original.get::<Sprite>(e).copied(),
                )
            })
            .collect();
        let mut after: Vec<_> = loaded
            .query::<SceneId>()
            .map(|(e, id)| {
                (
                    *id,
                    loaded.get::<Transform>(e).copied(),
                    loaded.get::<Sprite>(e).copied(),
                )
            })
            .collect();
        before.sort_by_key(|(id, _, _)| *id);
        after.sort_by_key(|(id, _, _)| *id);

        assert_eq!(before, after);
    }

    #[test]
    fn an_unknown_component_survives_a_round_trip() {
        // The test the preservation promise rests on. Compared as parsed values
        // rather than as text, because formatting is the serializer's choice and
        // byte equality against a hand-written file is not achievable.
        let registry = ComponentRegistry::with_defaults();
        let text = r#"(
            version: 1,
            entities: [
                (
                    id: "018f3a2b-7c41-7000-8000-2b1d4e5f6a70",
                    components: {
                        "Physics": (mass: 5.0, friction: 0.25),
                        "Sprite": (color: (1.0, 1.0, 1.0, 1.0), sort_order: 0),
                    },
                ),
            ],
        )"#;

        let file: SceneFile = ron::from_str(text).expect("the fixture is valid RON");
        let expected = file.entities[0]
            .components
            .get("Physics")
            .expect("the fixture has a Physics")
            .clone();

        let mut world = World::new();
        from_scene_file(&file, &registry, &mut world).expect("loading cannot fail here");

        let saved = to_scene_file(&world, &registry).expect("saving cannot fail here");
        assert_eq!(saved.entities[0].components.get("Physics"), Some(&expected));
    }

    #[test]
    fn a_wrong_version_is_refused() {
        let registry = ComponentRegistry::with_defaults();
        let file = SceneFile {
            version: VERSION + 1,
            entities: Vec::new(),
        };

        let mut world = World::new();
        let result = from_scene_file(&file, &registry, &mut world);
        assert!(
            matches!(result, Err(SceneError::UnsupportedVersion { .. })),
            "got {result:?}"
        );
    }

    #[test]
    fn a_registered_component_with_wrong_data_fails_the_load() {
        // Not preserved as unknown: the name is registered, so this build claims
        // to understand it, and data it cannot read is a real error.
        let registry = ComponentRegistry::with_defaults();
        let text = r#"(
            version: 1,
            entities: [
                (
                    id: "018f3a2b-7c41-7000-8000-2b1d4e5f6a70",
                    components: { "Sprite": "not a sprite" },
                ),
            ],
        )"#;
        let file: SceneFile = ron::from_str(text).expect("the fixture is valid RON");

        let mut world = World::new();
        let result = from_scene_file(&file, &registry, &mut world);
        assert!(
            matches!(result, Err(SceneError::Component { .. })),
            "got {result:?}"
        );
    }

    #[test]
    fn a_missing_file_is_an_io_error() {
        let registry = ComponentRegistry::with_defaults();
        let mut world = World::new();
        let missing = std::env::temp_dir().join("voltra-no-such-scene.ron");
        let _ = std::fs::remove_file(&missing);

        let result = load(&missing, &registry, &mut world);
        assert!(matches!(result, Err(SceneError::Io(_))), "got {result:?}");
    }

    #[test]
    fn malformed_ron_is_a_parse_error() {
        let registry = ComponentRegistry::with_defaults();
        let mut world = World::new();
        let dir = std::env::temp_dir().join("voltra-scene-load-test");
        std::fs::create_dir_all(&dir).expect("temp dir");
        let path = dir.join("broken.ron");
        std::fs::write(&path, "this is not ron at all {{{").expect("write");

        let result = load(&path, &registry, &mut world);
        assert!(
            matches!(result, Err(SceneError::Parse(_))),
            "got {result:?}"
        );
    }

    #[test]
    fn saving_is_idempotent_through_a_load() {
        let registry = ComponentRegistry::with_defaults();
        let dir = std::env::temp_dir().join("voltra-scene-idempotent-test");
        std::fs::create_dir_all(&dir).expect("temp dir");
        let first = dir.join("first.ron");
        let second = dir.join("second.ron");

        save(&source_world(), &registry, &first).expect("saving cannot fail here");

        let mut world = World::new();
        load(&first, &registry, &mut world).expect("loading cannot fail here");
        save(&world, &registry, &second).expect("saving cannot fail here");

        assert_eq!(
            std::fs::read_to_string(&first).expect("written"),
            std::fs::read_to_string(&second).expect("written"),
            "save -> load -> save must be a fixed point"
        );
    }
}
