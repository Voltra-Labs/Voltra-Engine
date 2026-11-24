use voltra_render::glam::Vec2;

use super::*;
use crate::format::to_scene_file;
use crate::hierarchy::{parent_of, set_parent};
use crate::{Name, Replicated, Transform};

fn registry() -> ComponentRegistry {
    ComponentRegistry::with_defaults()
}

fn named(world: &mut World, name: &str) -> Entity {
    let entity = world.spawn();
    world.insert(entity, SceneId::new());
    world.insert(entity, Name::new(name));
    world.insert(entity, Transform::from_translation(Vec2::ZERO));
    entity
}

fn name_of(world: &World, entity: Entity) -> String {
    world
        .get::<Name>(entity)
        .map(|name| name.as_str().to_owned())
        .unwrap_or_default()
}

/// A tree saved children first: `leaf` under `branch` under `trunk`, and a
/// lone `rock` — the order a load would trip over if it went by the file.
fn tree() -> SceneFile {
    let mut world = World::new();
    let leaf = named(&mut world, "leaf");
    let branch = named(&mut world, "branch");
    let trunk = named(&mut world, "trunk");
    named(&mut world, "rock");
    set_parent(&mut world, leaf, branch).expect("parents");
    set_parent(&mut world, branch, trunk).expect("parents");
    to_scene_file(&world, &registry()).expect("saves")
}

fn spawn_all(staged: &mut StagedScene, world: &mut World) {
    while staged
        .spawn_next(world, &registry())
        .expect("the records read")
        .is_some()
    {}
}

#[test]
fn one_entity_per_step_each_complete_when_it_arrives() {
    let mut staged = StagedScene::new(tree()).expect("this version");
    let mut world = World::new();
    assert_eq!(staged.len(), 4);

    while let Some(entity) = staged.spawn_next(&mut world, &registry()).expect("reads") {
        assert_eq!(world.entity_count(), staged.spawned());
        // Its parent, if it has one, is already in and linked.
        if let Some(link) = world.get::<Parent>(entity) {
            let parent = link.entity.expect("resolved on arrival");
            assert!(world.is_alive(parent));
        }
    }

    assert!(staged.is_done());
    let leaf = staged
        .entities()
        .iter()
        .copied()
        .find(|e| name_of(&world, *e) == "leaf")
        .expect("spawned");
    let branch = parent_of(&world, leaf).expect("under its branch");
    assert_eq!(name_of(&world, branch), "branch");
    let trunk = parent_of(&world, branch).expect("under the trunk");
    assert_eq!(name_of(&world, trunk), "trunk");
    let mut roots: Vec<String> = staged.roots().iter().map(|e| name_of(&world, *e)).collect();
    roots.sort();
    assert_eq!(roots, vec!["rock", "trunk"]);
}

#[test]
fn ids_are_fresh_as_a_spawn_s_are() {
    let file = tree();
    let original: Vec<SceneId> = file.entities.iter().map(|r| r.id).collect();
    let mut staged = StagedScene::new(file).expect("this version");
    let mut world = World::new();
    spawn_all(&mut staged, &mut world);
    for (_, id) in world.query::<SceneId>() {
        assert!(!original.contains(id));
    }
}

#[test]
fn a_skipped_record_takes_its_subtree_with_it() {
    let mut world = World::new();
    let player = named(&mut world, "player");
    world.insert(player, Replicated::default());
    let hat = named(&mut world, "hat");
    set_parent(&mut world, hat, player).expect("parents");
    named(&mut world, "wall");
    let file = to_scene_file(&world, &registry()).expect("saves");

    let mut staged = StagedScene::new(file).expect("this version");
    staged.skip(|record| record.components.contains_key(Replicated::NAME));
    let mut loaded = World::new();
    spawn_all(&mut staged, &mut loaded);

    let names: Vec<String> = staged
        .entities()
        .iter()
        .map(|e| name_of(&loaded, *e))
        .collect();
    assert_eq!(names, vec!["wall"]);
}

#[test]
fn despawn_takes_back_what_a_partial_load_made() {
    let mut staged = StagedScene::new(tree()).expect("this version");
    let mut world = World::new();
    staged.spawn_next(&mut world, &registry()).expect("reads");
    staged.spawn_next(&mut world, &registry()).expect("reads");
    assert_eq!(world.entity_count(), 2);

    staged.despawn(&mut world);

    assert_eq!(world.entity_count(), 0);
    assert!(staged.entities().is_empty());
}

#[test]
fn a_component_that_will_not_read_stops_the_spawn_with_its_error() {
    let mut file = tree();
    let bad = ron::value::RawValue::from_ron("(translation: \"left\")").expect("valid ron");
    file.entities[3]
        .components
        .insert("Transform".to_owned(), bad.to_owned());
    let mut staged = StagedScene::new(file).expect("this version");
    let mut world = World::new();

    let mut failed = false;
    for _ in 0..4 {
        if staged.spawn_next(&mut world, &registry()).is_err() {
            failed = true;
            break;
        }
    }

    assert!(failed);
    // The one that failed is not left half made.
    assert_eq!(world.entity_count(), staged.spawned());
}

#[test]
fn check_finds_the_broken_component_without_a_world() {
    assert!(StagedScene::new(tree())
        .expect("this version")
        .check(&registry())
        .is_ok());
    let mut file = tree();
    let bad = ron::value::RawValue::from_ron("(translation: \"left\")").expect("valid ron");
    file.entities[1]
        .components
        .insert("Transform".to_owned(), bad.to_owned());
    let staged = StagedScene::new(file).expect("this version");
    assert!(matches!(
        staged.check(&registry()),
        Err(SceneError::Component { .. })
    ));
}

#[test]
fn another_version_is_refused() {
    let mut file = tree();
    file.version += 1;
    assert!(matches!(
        StagedScene::new(file),
        Err(SceneError::UnsupportedVersion { .. })
    ));
}

#[test]
fn a_loop_of_links_still_spawns_every_entity() {
    let mut a = EntityRecord {
        id: SceneId::new(),
        components: Default::default(),
    };
    let mut b = EntityRecord {
        id: SceneId::new(),
        components: Default::default(),
    };
    let link = |id: SceneId| ron::value::RawValue::from_rust(&Parent::new(id)).expect("serialises");
    a.components.insert(PARENT.to_owned(), link(b.id));
    b.components.insert(PARENT.to_owned(), link(a.id));
    let file = SceneFile {
        version: VERSION,
        entities: vec![a, b],
    };
    let mut staged = StagedScene::new(file).expect("this version");
    let mut world = World::new();
    spawn_all(&mut staged, &mut world);
    assert_eq!(world.entity_count(), 2);
}
