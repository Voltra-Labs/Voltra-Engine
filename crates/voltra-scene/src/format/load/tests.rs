//! The tests of `load`.

use super::*;
use crate::format::save::{save, to_scene_file};
use crate::{CollisionLayers, Sensor, Sprite2D, Transform2D};
use voltra_render::glam::Vec2;

use crate::hierarchy::{children_of, parent_of, set_parent};

#[test]
fn a_parent_link_survives_a_save_and_a_load() {
    // The point of storing the link by `SceneId`: the entities the second
    // world hands out are different handles entirely.
    let mut source = World::new();
    let parent = source.spawn();
    source.insert(parent, SceneId::new());
    source.insert(parent, Transform2D::from_translation(Vec2::new(10.0, 0.0)));
    let child = source.spawn();
    source.insert(child, SceneId::new());
    source.insert(child, Transform2D::from_translation(Vec2::new(1.0, 0.0)));
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
    source.insert(child, Transform2D::default());
    let parent = source.spawn();
    source.insert(parent, SceneId::new());
    source.insert(parent, Transform2D::default());
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
            Transform2D::from_translation(Vec2::new(i as f32, -1.0)).with_scale(Vec2::splat(0.5)),
        );
        world.insert(e, Sprite2D::new([1.0, 0.0, 0.0, 1.0]).with_sort_order(i));
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
                original.get::<Transform2D>(e).copied(),
                original.get::<Sprite2D>(e).cloned(),
            )
        })
        .collect();
    let mut after: Vec<_> = loaded
        .query::<SceneId>()
        .map(|(e, id)| {
            (
                *id,
                loaded.get::<Transform2D>(e).copied(),
                loaded.get::<Sprite2D>(e).cloned(),
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
    original.insert(entity, Transform2D::default());
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
    world.insert(pre_existing, Sprite2D::default());

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
        world.get::<Sprite2D>(pre_existing),
        Some(&Sprite2D::default()),
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

#[test]
fn records_spawn_under_their_own_ids_and_resolve_once_asked() {
    let mut source = World::new();
    let parent = source.spawn();
    let parent_id = SceneId::new();
    source.insert(parent, parent_id);
    source.insert(parent, Transform2D::default());
    let child = source.spawn();
    source.insert(child, SceneId::new());
    source.insert(child, Sprite2D::default());
    set_parent(&mut source, child, parent).expect("a plain reparent");
    let registry = ComponentRegistry::with_defaults();
    let records: Vec<EntityRecord> = [child, parent]
        .iter()
        .map(|&e| {
            crate::format::record_entity(&source, &registry, e)
                .expect("has an id")
                .expect("serializes")
        })
        .collect();

    let mut target = World::new();
    let spawned = spawn_records(&records, &registry, &mut target).expect("spawns");
    assert_eq!(spawned.len(), 2);
    assert_eq!(target.get::<SceneId>(spawned[1]), Some(&parent_id));
    assert_eq!(parent_of(&target, spawned[0]), None, "not resolved yet");
    crate::hierarchy::resolve_parents(&mut target);
    assert_eq!(parent_of(&target, spawned[0]), Some(spawned[1]));

    // All or nothing.
    let mut broken = records.clone();
    broken[1].components.insert(
        "Transform".to_owned(),
        ron::value::RawValue::from_ron("7").expect("ron").to_owned(),
    );
    let mut empty = World::new();
    assert!(spawn_records(&broken, &registry, &mut empty).is_err());
    assert_eq!(empty.entity_count(), 0);
}
