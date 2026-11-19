//! The link from an instance root back to the file it was spawned from.

use voltra_render::glam::Vec2;
use voltra_testkit::scratch_root;

use super::*;

fn link_of(world: &World, entity: Entity) -> Option<&str> {
    world
        .get::<PrefabInstance>(entity)
        .map(|link| link.source.as_str())
}

#[test]
fn a_prefab_read_from_a_file_links_its_root_to_that_file() {
    let root = scratch_root();
    write_ship(&root, "ship.ron");
    let mut prefabs = Prefabs::new(&root);
    let handle = prefabs.load(&path("ship.ron"));
    let mut world = World::new();

    let instance = spawn(
        &mut world,
        &registry(),
        prefabs.get(handle),
        &Transform::default(),
    )
    .expect("reads");

    let hull = instance.root().expect("one root");
    assert_eq!(link_of(&world, hull), Some("ship.ron"));
}

#[test]
fn only_roots_are_linked_not_their_children() {
    let ship = ship().with_source(path("ship.ron"));
    let mut world = World::new();

    let instance = spawn(&mut world, &registry(), &ship, &Transform::default()).expect("reads");

    let hull = instance.root().expect("one root");
    let gun = children_of(&world, hull)[0];
    assert_eq!(link_of(&world, hull), Some("ship.ron"));
    assert_eq!(link_of(&world, gun), None);
}

#[test]
fn a_prefab_built_in_memory_links_nothing() {
    let mut world = World::new();

    let instance = spawn(&mut world, &registry(), &ship(), &Transform::default()).expect("reads");

    assert_eq!(link_of(&world, instance.root().expect("one root")), None);
}

#[test]
fn a_root_is_an_instance_of_what_it_was_spawned_from_not_what_it_was_saved_as() {
    // A scene saved from an instance of `coin.ron` and then spawned as a
    // prefab of its own: the root's record still says `coin.ron`, and the
    // spawn is of `pile.ron`.
    let mut source = World::new();
    let coin = named(&mut source, "coin", Vec2::ZERO);
    source.insert(coin, PrefabInstance::new(path("coin.ron")));
    let pile = prefab_of(&source).with_source(path("pile.ron"));
    let mut world = World::new();

    let instance = spawn(&mut world, &registry(), &pile, &Transform::default()).expect("reads");

    assert_eq!(
        link_of(&world, instance.root().expect("one root")),
        Some("pile.ron")
    );
}

#[test]
fn a_child_that_is_an_instance_of_its_own_keeps_its_link() {
    // A chest prefab with a coin instance inside it: the coin is a nested
    // instance and has to go on saying so.
    let mut source = World::new();
    let chest = named(&mut source, "chest", Vec2::ZERO);
    let coin = named(&mut source, "coin", Vec2::new(0.0, 1.0));
    source.insert(coin, PrefabInstance::new(path("coin.ron")));
    set_parent(&mut source, coin, chest).expect("a plain reparent");
    let chest = prefab_of(&source).with_source(path("chest.ron"));
    let mut world = World::new();

    let instance = spawn(&mut world, &registry(), &chest, &Transform::default()).expect("reads");

    let root = instance.root().expect("one root");
    let child = children_of(&world, root)[0];
    assert_eq!(link_of(&world, root), Some("chest.ron"));
    assert_eq!(link_of(&world, child), Some("coin.ron"));
}

#[test]
fn the_link_survives_a_save_and_a_load() {
    let ship = ship().with_source(path("ship.ron"));
    let mut world = World::new();
    spawn(&mut world, &registry(), &ship, &Transform::default()).expect("reads");
    let file = to_scene_file(&world, &registry()).expect("saves");

    let mut loaded = World::new();
    crate::format::from_scene_file(&file, &registry(), &mut loaded).expect("loads");

    let linked: Vec<_> = loaded
        .query::<PrefabInstance>()
        .map(|(_, link)| link.source.as_str().to_owned())
        .collect();
    assert_eq!(linked, ["ship.ron"]);
}
