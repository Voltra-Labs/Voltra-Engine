//! Reading a scene file back into a world.

use std::path::Path;

use voltra_ecs::{Entity, World};

use super::error::SceneError;
use super::registry::ComponentRegistry;
#[cfg(test)]
use super::save::EntityRecord;
use super::save::{SceneFile, VERSION};
#[cfg(test)]
use crate::scene_id::SceneId;

/// Spawns every entity in `file` into `world`.
///
/// Adds rather than replaces. Whether to clear first is the caller's decision,
/// and the editor's menu is where that belongs.
///
/// All-or-nothing: if any entity fails to load, every entity this call spawned
/// is despawned before the error is returned, so a failed load leaves `world`
/// exactly as it found it. An editor that reports "could not open the scene"
/// and then shows half of it is worse than one that shows nothing.
///
/// The rollback is why the per-entity work lives in [`spawn_entities`] rather
/// than inline behind a `?`: `?` returns the instant something fails, which is
/// exactly the moment the list of entities to clean up would otherwise be
/// lost. `spawn_entities` instead returns the entities it spawned *alongside*
/// its `Result`, unconditionally, so there is a value to inspect and act on
/// before this function decides what to return.
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

    let (spawned, result) = spawn_entities(file, registry, world);
    if let Err(error) = result {
        for entity in spawned {
            world.despawn(entity);
        }
        return Err(error);
    }

    // Only now: a file is a list, and a child may name a parent that appears
    // after it. Resolving as each entity spawned would leave every forward
    // reference permanently unresolved.
    crate::hierarchy::resolve_parents(world);

    log::info!("loaded {} entities", spawned.len());
    Ok(())
}

/// Spawns every entity in `file` into `world`, stopping at the first error.
///
/// Always returns every entity spawned before it stopped, paired with the
/// outcome, rather than folding the two into a `Result<Vec<Entity>, (Vec<
/// Entity>, SceneError)>`: `clippy::result_large_err` correctly rejects that
/// shape (the tuple makes `SceneError`'s size infectious), and the caller
/// needs the spawned list in both the success and the failure case anyway, so
/// a plain pair is both the smaller type and the fit.
fn spawn_entities(
    file: &SceneFile,
    registry: &ComponentRegistry,
    world: &mut World,
) -> (Vec<Entity>, Result<(), SceneError>) {
    let mut spawned = Vec::with_capacity(file.entities.len());

    for record in &file.entities {
        let entity = world.spawn();
        spawned.push(entity);
        world.insert(entity, record.id);

        // Shared with `apply_record`, which reads the same components onto an
        // entity that may already exist. A load always spawns, so it keeps the
        // spawn and hands over only the reading.
        let unknown =
            match super::record::insert_components(world, registry, entity, &record.components) {
                Ok(unknown) => unknown,
                // The whole load goes down rather than leaving this entity
                // half-built; the caller despawns everything in `spawned`.
                Err(error) => return (spawned, Err(error)),
            };

        if !unknown.0.is_empty() {
            world.insert(entity, unknown);
        }
    }

    (spawned, Ok(()))
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
    use crate::{CollisionLayers, Sensor, Sprite, Transform};
    use voltra_render::glam::Vec2;

    use crate::hierarchy::{children_of, parent_of, set_parent};

    #[test]
    fn a_parent_link_survives_a_save_and_a_load() {
        // The point of storing the link by `SceneId`: the entities the second
        // world hands out are different handles entirely.
        let mut source = World::new();
        let parent = source.spawn();
        source.insert(parent, SceneId::new());
        source.insert(parent, Transform::from_translation(Vec2::new(10.0, 0.0)));
        let child = source.spawn();
        source.insert(child, SceneId::new());
        source.insert(child, Transform::from_translation(Vec2::new(1.0, 0.0)));
        set_parent(&mut source, child, parent).expect("a plain reparent");

        let registry = ComponentRegistry::with_defaults();
        let file = to_scene_file(&source, &registry).expect("a valid world saves");

        let mut target = World::new();
        from_scene_file(&file, &registry, &mut target).expect("what we just wrote");

        let loaded_child = crate::format::entity_with_id(
            &target,
            *source.get::<SceneId>(child).expect("spawned with one"),
        )
        .expect("the child is in the file");
        let loaded_parent = crate::format::entity_with_id(
            &target,
            *source.get::<SceneId>(parent).expect("spawned with one"),
        )
        .expect("the parent is in the file");

        assert_eq!(parent_of(&target, loaded_child), Some(loaded_parent));
        assert_eq!(children_of(&target, loaded_parent), vec![loaded_child]);
    }

    #[test]
    fn a_child_listed_before_its_parent_still_resolves() {
        // A file is sorted by id, and an entity made before its parent sorts
        // first. Resolving per entity as it spawned would leave this dangling.
        let mut source = World::new();
        let child = source.spawn();
        source.insert(child, SceneId::new());
        source.insert(child, Transform::default());
        let parent = source.spawn();
        source.insert(parent, SceneId::new());
        source.insert(parent, Transform::default());
        set_parent(&mut source, child, parent).expect("a plain reparent");

        let registry = ComponentRegistry::with_defaults();
        let file = to_scene_file(&source, &registry).expect("a valid world saves");
        assert!(
            file.entities[0].id < file.entities[1].id,
            "the child was created first, so it is written first"
        );

        let mut target = World::new();
        from_scene_file(&file, &registry, &mut target).expect("what we just wrote");

        let loaded_child = crate::format::entity_with_id(
            &target,
            *source.get::<SceneId>(child).expect("spawned with one"),
        )
        .expect("the child is in the file");
        assert!(parent_of(&target, loaded_child).is_some());
    }

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
                    original.get::<Sprite>(e).cloned(),
                )
            })
            .collect();
        let mut after: Vec<_> = loaded
            .query::<SceneId>()
            .map(|(e, id)| {
                (
                    *id,
                    loaded.get::<Transform>(e).copied(),
                    loaded.get::<Sprite>(e).cloned(),
                )
            })
            .collect();
        before.sort_by_key(|(id, _, _)| *id);
        after.sort_by_key(|(id, _, _)| *id);

        assert_eq!(before, after);
    }

    #[test]
    fn a_filter_and_a_sensor_survive_a_round_trip() {
        let registry = ComponentRegistry::with_defaults();
        let mut original = World::new();
        let entity = original.spawn();
        original.insert(entity, SceneId::new());
        original.insert(entity, Transform::default());
        original.insert(entity, CollisionLayers::on(3).looking_at(1));
        original.insert(entity, Sensor);

        let file = to_scene_file(&original, &registry).expect("saving cannot fail here");
        let mut loaded = World::new();
        from_scene_file(&file, &registry, &mut loaded).expect("loading cannot fail here");

        let (loaded_entity, _) = loaded
            .query::<SceneId>()
            .next()
            .expect("the one entity is in the file");
        assert_eq!(
            loaded.get::<CollisionLayers>(loaded_entity),
            Some(&CollisionLayers::on(3).looking_at(1))
        );
        assert_eq!(loaded.get::<Sensor>(loaded_entity), Some(&Sensor));
    }

    #[test]
    fn a_scene_written_before_filters_existed_still_loads() {
        // Which is what "absent means everything, and solid" is for: a file
        // with no filter is not a file with an empty one.
        let registry = ComponentRegistry::with_defaults();
        let text = r#"(
            version: 1,
            entities: [
                (
                    id: "018f3a2b-7c41-7000-8000-2b1d4e5f6a70",
                    components: {
                        "Collider": Box(half_extents: (0.5, 0.5)),
                    },
                ),
            ],
        )"#;
        let file: SceneFile = ron::from_str(text).expect("the fixture is valid RON");

        let mut world = World::new();
        from_scene_file(&file, &registry, &mut world).expect("loading cannot fail here");

        let (entity, _) = world.query::<SceneId>().next().expect("one entity");
        assert_eq!(world.get::<CollisionLayers>(entity), None);
        assert_eq!(world.get::<Sensor>(entity), None);
        assert!(CollisionLayers::interact(None, None));
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
        // Non-empty on purpose: with `entities: Vec::new()` a version check
        // placed *after* the spawn loop would pass just as happily, because
        // there is nothing to spawn either way. This proves the ordering.
        let registry = ComponentRegistry::with_defaults();
        let mut components = std::collections::BTreeMap::new();
        components.insert(
            "Sprite".to_owned(),
            ron::value::RawValue::from_ron("(color: (1.0, 1.0, 1.0, 1.0), sort_order: 0)")
                .expect("valid RON")
                .to_owned(),
        );
        let file = SceneFile {
            version: VERSION + 1,
            entities: vec![EntityRecord {
                id: SceneId::new(),
                components,
            }],
        };

        let mut world = World::new();
        let result = from_scene_file(&file, &registry, &mut world);
        assert!(
            matches!(result, Err(SceneError::UnsupportedVersion { .. })),
            "got {result:?}"
        );
        assert_eq!(
            world.entity_count(),
            0,
            "a version check that runs after the spawn loop would leave an entity behind"
        );
    }

    #[test]
    fn a_failed_load_rolls_back_every_entity_it_spawned() {
        // A load is all-or-nothing: reporting an error and leaving half the
        // file's entities in the world is worse than reporting nothing, since
        // nothing then signals how far the load got.
        let registry = ComponentRegistry::with_defaults();
        let text = r#"(
            version: 1,
            entities: [
                (
                    id: "018f3a2b-7c41-7000-8000-2b1d4e5f6a70",
                    components: { "Sprite": (color: (1.0, 1.0, 1.0, 1.0), sort_order: 0) },
                ),
                (
                    id: "018f3a2b-7c41-7000-8000-2b1d4e5f6a71",
                    components: { "Sprite": "not a sprite" },
                ),
            ],
        )"#;
        let file: SceneFile = ron::from_str(text).expect("the fixture is valid RON");

        let mut world = World::new();
        let pre_existing = world.spawn();
        world.insert(pre_existing, SceneId::new());
        world.insert(pre_existing, Sprite::default());

        let result = from_scene_file(&file, &registry, &mut world);
        assert!(
            matches!(result, Err(SceneError::Component { .. })),
            "got {result:?}"
        );

        assert!(
            world.is_alive(pre_existing),
            "a failed load must not touch an entity that was already in the world"
        );
        assert_eq!(
            world.get::<Sprite>(pre_existing),
            Some(&Sprite::default()),
            "a failed load must not touch components on a pre-existing entity"
        );
        assert_eq!(
            world.query::<SceneId>().count(),
            1,
            "no entity from the failing file may remain after a failed load"
        );
        assert_eq!(
            world.entity_count(),
            1,
            "every entity the failed load spawned must be rolled back"
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
