use std::collections::BTreeMap;

use voltra_assets::AssetPath;
use voltra_render::glam::Vec2;

use super::*;
use crate::format::ComponentRegistry;
use crate::format::{to_scene_file, EntityRecord};
use crate::hierarchy::Parent;
use crate::hierarchy::{children_of, parent_of, set_parent};
use crate::scene_id::SceneId;
use crate::transform::Transform;
use crate::{Name, Sprite};
use voltra_ecs::{Entity, World};

fn registry() -> ComponentRegistry {
    ComponentRegistry::with_defaults()
}

/// A prefab made the way an author makes one: build it in a world, save it.
fn prefab_of(world: &World) -> Prefab {
    let file = to_scene_file(world, &registry()).expect("a valid world saves");
    Prefab::new(file).expect("this build's own version")
}

fn named(world: &mut World, name: &str, at: Vec2) -> Entity {
    let entity = world.spawn();
    world.insert(entity, SceneId::new());
    world.insert(entity, Name::new(name));
    world.insert(entity, Transform::from_translation(at));
    entity
}

/// A ship at the origin with a gun one unit to its right.
fn ship() -> Prefab {
    let mut world = World::new();
    let hull = named(&mut world, "hull", Vec2::ZERO);
    world.insert(hull, Sprite::default());
    let gun = named(&mut world, "gun", Vec2::new(1.0, 0.0));
    set_parent(&mut world, gun, hull).expect("a plain reparent");
    prefab_of(&world)
}

fn name_of(world: &World, entity: Entity) -> &str {
    world.get::<Name>(entity).map(Name::as_str).unwrap_or("")
}

#[test]
fn a_spawn_makes_every_entity_with_its_components() {
    let mut world = World::new();

    let instance =
        spawn(&mut world, &registry(), &ship(), &Transform::default()).expect("the prefab reads");

    assert_eq!(instance.entities.len(), 2);
    assert_eq!(world.entity_count(), 2);
    let hull = instance.root().expect("one root");
    assert_eq!(name_of(&world, hull), "hull");
    assert!(world.get::<Sprite>(hull).is_some());
}

#[test]
fn two_spawns_never_share_an_id() {
    // Two entities under one id is a save that writes one of them twice and
    // a load that puts one of them back.
    let mut world = World::new();
    let prefab = ship();

    spawn(&mut world, &registry(), &prefab, &Transform::default()).expect("reads");
    spawn(&mut world, &registry(), &prefab, &Transform::default()).expect("reads");

    let mut ids: Vec<SceneId> = world.query::<SceneId>().map(|(_, id)| *id).collect();
    let before = ids.len();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), before);
}

#[test]
fn the_ids_are_not_the_prefab_s_own() {
    // An instance spawned into the scene the prefab was cut from must not
    // collide with the entities it was cut from.
    let mut source = World::new();
    let only = named(&mut source, "only", Vec2::ZERO);
    let original = *source.get::<SceneId>(only).expect("spawned with one");
    let prefab = prefab_of(&source);

    let instance = spawn(&mut source, &registry(), &prefab, &Transform::default()).expect("reads");

    let copy = instance.root().expect("one root");
    assert_ne!(source.get::<SceneId>(copy), Some(&original));
}

#[test]
fn a_child_hangs_off_its_own_instance_s_parent() {
    // The remap: with two instances in the world, each gun must be under the
    // hull spawned with it rather than under whichever hull resolved first.
    let mut world = World::new();
    let prefab = ship();

    let first = spawn(&mut world, &registry(), &prefab, &Transform::default()).expect("reads");
    let second = spawn(&mut world, &registry(), &prefab, &Transform::default()).expect("reads");

    for instance in [first, second] {
        let hull = instance.root().expect("one root");
        let guns = children_of(&world, hull);
        assert_eq!(guns.len(), 1);
        assert!(instance.entities.contains(&guns[0]));
        assert_eq!(
            parent_of(&world, guns[0]),
            Some(hull),
            "resolved, not just named"
        );
    }
}

#[test]
fn a_parent_outside_the_prefab_is_cut_and_its_child_is_a_root() {
    // A fragment cut out of a bigger scene still names the parent it had
    // there. Following that id would hang every instance off one entity in
    // whatever world happens to hold it.
    let mut world = World::new();
    let outside = named(&mut world, "outside", Vec2::ZERO);
    let outside_id = *world.get::<SceneId>(outside).expect("spawned with one");
    let mut source = World::new();
    let orphan = named(&mut source, "orphan", Vec2::ZERO);
    source.insert(orphan, Parent::new(outside_id));

    let instance = spawn(
        &mut world,
        &registry(),
        &prefab_of(&source),
        &Transform::default(),
    )
    .expect("reads");

    let root = instance.root().expect("the orphan is a root");
    assert_eq!(parent_of(&world, root), None);
    assert!(
        world.get::<Parent>(root).is_none(),
        "the link is gone, not just unresolved"
    );
}

#[test]
fn a_component_that_will_not_read_spawns_nothing_at_all() {
    let mut world = World::new();
    let mut components = BTreeMap::new();
    components.insert(
        "Transform".to_owned(),
        ron::value::RawValue::from_boxed_ron("\"not a transform\"".into()).expect("valid ron"),
    );
    let file = SceneFile {
        version: VERSION,
        entities: vec![
            EntityRecord {
                id: SceneId::new(),
                components: BTreeMap::new(),
            },
            EntityRecord {
                id: SceneId::new(),
                components,
            },
        ],
    };
    let prefab = Prefab::new(file).expect("the version is fine");

    let result = spawn(&mut world, &registry(), &prefab, &Transform::default());

    assert!(result.is_err());
    assert_eq!(
        world.entity_count(),
        0,
        "the first entity was taken back out"
    );
}

#[test]
fn an_unknown_component_travels_with_the_copy() {
    // A build that knows fewer components must not strip the ones it does
    // not know from every instance it spawns.
    let mut components = BTreeMap::new();
    components.insert(
        "FutureThing".to_owned(),
        ron::value::RawValue::from_boxed_ron("(answer: 42)".into()).expect("valid ron"),
    );
    let file = SceneFile {
        version: VERSION,
        entities: vec![EntityRecord {
            id: SceneId::new(),
            components,
        }],
    };
    let mut world = World::new();

    let instance = spawn(
        &mut world,
        &registry(),
        &Prefab::new(file).expect("fine"),
        &Transform::default(),
    )
    .expect("reads");

    let kept = world
        .get::<crate::scene_id::UnknownComponents>(instance.root().expect("one"))
        .expect("kept verbatim");
    assert!(kept.0.contains_key("FutureThing"));
}

#[test]
fn a_file_from_another_version_is_refused_once_rather_than_at_every_spawn() {
    let file = SceneFile {
        version: VERSION + 1,
        entities: Vec::new(),
    };
    assert!(matches!(
        Prefab::new(file),
        Err(SceneError::UnsupportedVersion { .. })
    ));
}

#[test]
fn the_empty_prefab_spawns_nothing() {
    let mut world = World::new();
    let instance = spawn(
        &mut world,
        &registry(),
        &Prefab::empty(),
        &Transform::default(),
    )
    .expect("empty");
    assert_eq!(instance, Instance::default());
    assert_eq!(world.entity_count(), 0);
}

fn path(raw: &str) -> AssetPath {
    AssetPath::new(raw).expect("a valid asset path")
}

fn write_ship(root: &std::path::Path, name: &str) {
    let mut world = World::new();
    named(&mut world, "hull", Vec2::ZERO);
    crate::format::save(&world, &registry(), &root.join(name)).expect("the fixture writes");
}

mod added;
mod link;
mod loops;
mod nested;
mod own;
mod placement;
mod store;
mod variant;
mod whole_scenes;
mod write;
