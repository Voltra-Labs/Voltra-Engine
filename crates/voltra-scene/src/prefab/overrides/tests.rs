//! Overrides kept up to date as an instance is edited, reverted, and an
//! instance applied back to its file — through a real store on disk.

use voltra_assets::AssetPath;
use voltra_render::glam::Vec2;

use super::*;
use crate::format::to_scene_file;
use crate::hierarchy::{children_of, set_parent};
use crate::name::Name;
use crate::prefab::{apply, link, spawn, sync, write};
use crate::scene_id::SceneId;
use crate::{Sprite2D, Transform2D};

fn registry() -> ComponentRegistry {
    ComponentRegistry::with_defaults()
}

fn source() -> AssetPath {
    AssetPath::new("coin.ron").expect("valid")
}

fn named(world: &mut World, name: &str, at: Vec2) -> Entity {
    let entity = world.spawn();
    world.insert(entity, SceneId::new());
    world.insert(entity, Name::new(name));
    world.insert(entity, Transform2D::from_translation(at));
    entity
}

/// A store holding `coin.ron`: a red coin with a glow under it.
fn store() -> (Prefabs, voltra_testkit::Scratch) {
    let root = voltra_testkit::scratch_root();
    let mut world = World::new();
    let coin = named(&mut world, "coin", Vec2::ZERO);
    world.insert(coin, Sprite2D::new([1.0, 0.0, 0.0, 1.0]));
    let glow = named(&mut world, "glow", Vec2::new(0.0, 0.2));
    world.insert(glow, Sprite2D::default());
    set_parent(&mut world, glow, coin).expect("a plain reparent");
    let file = to_scene_file(&world, &registry()).expect("saves");
    crate::format::write(&file, &root.join("coin.ron")).expect("writes");
    (Prefabs::new(&root), root)
}

fn instance(world: &mut World, prefabs: &mut Prefabs, at: Vec2) -> Entity {
    let handle = prefabs.load(&source());
    spawn(
        world,
        &registry(),
        prefabs.get(handle),
        Transform2D::from_translation(at),
    )
    .expect("spawns")
    .root()
    .expect("one root")
}

fn glow_of(world: &World, root: Entity) -> Entity {
    children_of(world, root)
        .into_iter()
        .find(|&c| world.get::<Name>(c).is_some_and(|n| n.as_str() == "glow"))
        .expect("a glow")
}

fn overridden(world: &World, entity: Entity) -> Vec<String> {
    world
        .get::<PrefabPart>(entity)
        .expect("a part")
        .overridden
        .iter()
        .cloned()
        .collect()
}

#[test]
fn an_edit_is_an_override_until_it_is_taken_back() {
    let (mut prefabs, _root) = store();
    let mut scene = World::new();
    let coin = instance(&mut scene, &mut prefabs, Vec2::new(4.0, 0.0));
    let refresh = |scene: &mut World, prefabs: &mut Prefabs| {
        refresh_overrides(scene, &registry(), prefabs, coin).expect("compares")
    };
    assert!(
        !refresh(&mut scene, &mut prefabs),
        "fresh, and placed: nothing"
    );

    scene.insert(coin, Sprite2D::new([0.0, 1.0, 0.0, 1.0]));
    assert!(refresh(&mut scene, &mut prefabs));
    assert_eq!(overridden(&scene, coin), ["Sprite2D"]);
    assert!(!refresh(&mut scene, &mut prefabs), "no change, no write");

    scene.insert(coin, Sprite2D::new([1.0, 0.0, 0.0, 1.0]));
    assert!(refresh(&mut scene, &mut prefabs));
    assert!(overridden(&scene, coin).is_empty(), "back to the prefab's");
}

#[test]
fn a_revert_puts_one_component_or_all_of_them_back() {
    let (mut prefabs, _root) = store();
    let mut scene = World::new();
    let coin = instance(&mut scene, &mut prefabs, Vec2::ZERO);
    scene.insert(coin, Sprite2D::new([0.0, 1.0, 0.0, 1.0]));
    scene.insert(coin, Name::new("mine"));
    refresh_overrides(&mut scene, &registry(), &mut prefabs, coin).expect("compares");
    assert_eq!(overridden(&scene, coin), ["Name", "Sprite2D"]);

    revert(
        &mut scene,
        &registry(),
        &mut prefabs,
        coin,
        Some("Sprite2D"),
    )
    .expect("reverts");
    assert_eq!(
        scene.get::<Sprite2D>(coin).expect("s").color,
        [1.0, 0.0, 0.0, 1.0]
    );
    assert_eq!(scene.get::<Name>(coin).expect("n").as_str(), "mine");
    revert(&mut scene, &registry(), &mut prefabs, coin, None).expect("reverts");
    assert_eq!(scene.get::<Name>(coin).expect("n").as_str(), "coin");
}

#[test]
fn a_deleted_part_is_remembered_and_a_whole_revert_brings_it_back() {
    let (mut prefabs, _root) = store();
    let mut scene = World::new();
    let coin = instance(&mut scene, &mut prefabs, Vec2::ZERO);
    let glow = glow_of(&scene, coin);
    assert_eq!(note_removed(&mut scene, glow), Some(coin));
    assert_eq!(
        note_removed(&mut scene, coin),
        None,
        "the root takes the whole instance"
    );
    hierarchy::despawn_recursive(&mut scene, glow);

    let handle = prefabs.load(&source());
    let report = sync(&mut scene, &registry(), prefabs.get(handle), coin).expect("syncs");
    assert_eq!(report.spawned, 0, "deleted stays deleted");

    revert_instance(&mut scene, &registry(), &mut prefabs, coin).expect("reverts");
    assert_eq!(children_of(&scene, coin).len(), 1, "the glow is back");
}

#[test]
fn applying_an_instance_writes_it_over_the_prefab_and_the_others_follow() {
    let (mut prefabs, root) = store();
    let mut scene = World::new();
    let edited = instance(&mut scene, &mut prefabs, Vec2::new(5.0, 0.0));
    let other = instance(&mut scene, &mut prefabs, Vec2::new(-5.0, 0.0));
    // The edit: blue, and a shine of its own under it.
    scene.insert(edited, Sprite2D::new([0.0, 0.0, 1.0, 1.0]));
    let shine = named(&mut scene, "shine", Vec2::new(0.1, 0.0));
    set_parent(&mut scene, shine, edited).expect("a plain reparent");
    refresh_overrides(&mut scene, &registry(), &mut prefabs, edited).expect("compares");

    let report = apply(&mut scene, &registry(), &mut prefabs, edited).expect("applies");
    assert_eq!(report.spawned, 1, "the other instance gets the shine");
    assert_eq!(
        scene.get::<Sprite2D>(other).expect("s").color,
        [0.0, 0.0, 1.0, 1.0]
    );
    assert_eq!(children_of(&scene, other).len(), 2);
    assert!(
        overridden(&scene, edited).is_empty(),
        "it is the prefab now"
    );
    assert_eq!(
        scene.get::<Transform2D>(other).expect("t").translation,
        Vec2::new(-5.0, 0.0),
        "placements stay their own"
    );

    // On disk, the root is where the prefab had it, not where the instance is.
    let mut fresh = Prefabs::new(&root);
    let mut again = World::new();
    let copy = instance(&mut again, &mut fresh, Vec2::ZERO);
    assert_eq!(
        again.get::<Transform2D>(copy).expect("t").translation,
        Vec2::ZERO
    );
    assert_eq!(
        again.get::<Sprite2D>(copy).expect("s").color,
        [0.0, 0.0, 1.0, 1.0]
    );
}

#[test]
fn a_selection_written_and_linked_is_already_in_step() {
    let (mut prefabs, _root) = store();
    let mut scene = World::new();
    let lamp = named(&mut scene, "lamp", Vec2::new(3.0, 3.0));
    scene.insert(lamp, Sprite2D::default());
    let target = AssetPath::new("lamp.ron").expect("valid");
    write(&scene, &registry(), &mut prefabs, lamp, &target).expect("writes");
    link(&mut scene, lamp, target.clone());

    let handle = prefabs.load(&target);
    let report = sync(&mut scene, &registry(), prefabs.get(handle), lamp).expect("syncs");
    assert!(report.is_empty(), "{report:?}");
    assert_eq!(
        scene.get::<Transform2D>(lamp).expect("t").translation,
        Vec2::new(3.0, 3.0)
    );
}
