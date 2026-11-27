//! Cutting a selection out as a prefab, and refusing one that holds itself.

use voltra_render::glam::Vec2;
use voltra_testkit::scratch_root;

use super::*;

/// A world with a level, a chest under it and a coin under the chest.
fn level() -> (World, Entity, Entity, Entity) {
    let mut world = World::new();
    let level = named(&mut world, "level", Vec2::ZERO);
    let chest = named(&mut world, "chest", Vec2::new(5.0, 2.0));
    if let Some(transform) = world.get_mut::<Transform>(chest) {
        transform.rotation = 0.5;
    }
    set_parent(&mut world, chest, level).expect("a plain reparent");
    let coin = named(&mut world, "coin", Vec2::new(0.0, 1.0));
    set_parent(&mut world, coin, chest).expect("a plain reparent");
    (world, level, chest, coin)
}

#[test]
fn a_fragment_holds_the_root_and_what_hangs_off_it_and_nothing_above() {
    let (world, _, chest, _) = level();

    let file = fragment(&world, &registry(), chest).expect("records");

    assert_eq!(file.entities.len(), 2);
    assert_eq!(
        file.entities[0].id,
        *world.get::<SceneId>(chest).expect("an id")
    );
}

#[test]
fn the_root_loses_its_parent_and_its_position_but_keeps_its_shape() {
    let (world, _, chest, _) = level();

    let prefab = Prefab::new(fragment(&world, &registry(), chest).expect("records")).expect("v1");
    let mut out = World::new();
    let at = Transform::from_translation(Vec2::new(-3.0, 0.0));
    let instance = spawn(&mut out, &registry(), &prefab, &at).expect("reads");

    let root = instance.root().expect("one root");
    let transform = out.get::<Transform>(root).expect("positional");
    assert_eq!(transform.translation, Vec2::new(-3.0, 0.0), "lands on `at`");
    assert_eq!(transform.rotation, 0.5);
    assert_eq!(instance.entities.len(), 2);
    assert_eq!(parent_of(&out, instance.entities[1]), Some(root));
}

#[test]
fn the_root_forgets_what_it_was_an_instance_of_and_a_child_does_not() {
    let (mut world, _, chest, coin) = level();
    world.insert(chest, PrefabInstance::new(path("old_chest.ron")));
    world.insert(coin, PrefabInstance::new(path("coin.ron")));

    let file = fragment(&world, &registry(), chest).expect("records");

    assert!(!file.entities[0]
        .components
        .contains_key(PrefabInstance::NAME));
    assert_eq!(
        Prefab::new(file).expect("v1").nested_sources(),
        [path("coin.ron")]
    );
}

#[test]
fn a_written_prefab_spawns_linked_to_where_it_was_written() {
    let root = scratch_root();
    let (world, _, chest, _) = level();
    let mut prefabs = Prefabs::new(&root);

    write(
        &world,
        &registry(),
        &mut prefabs,
        chest,
        &path("props/chest.ron"),
    )
    .expect("writes");

    let handle = prefabs.load(&path("props/chest.ron"));
    let mut out = World::new();
    let instance = spawn(
        &mut out,
        &registry(),
        prefabs.get(handle),
        &Transform::default(),
    )
    .expect("reads");
    assert_eq!(instance.entities.len(), 2);
    assert_eq!(
        out.get::<PrefabInstance>(instance.root().expect("one root"))
            .map(|link| link.source.as_str()),
        Some("props/chest.ron")
    );
}

#[test]
fn writing_over_a_loaded_prefab_refreshes_it_straight_away() {
    let root = scratch_root();
    write_ship(&root, "chest.ron");
    let mut prefabs = Prefabs::new(&root);
    let handle = prefabs.load(&path("chest.ron"));
    assert_eq!(prefabs.get(handle).len(), 1);
    let (world, _, chest, _) = level();

    write(&world, &registry(), &mut prefabs, chest, &path("chest.ron")).expect("writes");

    assert_eq!(prefabs.get(handle).len(), 2);
}

#[test]
fn a_prefab_that_would_hold_an_instance_of_itself_is_refused_and_not_written() {
    let root = scratch_root();
    let (mut world, _, chest, coin) = level();
    world.insert(coin, PrefabInstance::new(path("chest.ron")));
    let mut prefabs = Prefabs::new(&root);

    let error = write(&world, &registry(), &mut prefabs, chest, &path("chest.ron"))
        .expect_err("a prefab inside itself");

    assert!(
        matches!(error, WriteError::Cycle { ref through, .. } if through.as_str() == "chest.ron")
    );
    assert!(!root.join("chest.ron").exists());
}

#[test]
fn a_cycle_through_another_prefab_is_refused_too() {
    // `bag.ron` holds a chest; writing a chest that holds a bag would make
    // each contain the other.
    let root = scratch_root();
    let mut bag = World::new();
    let sack = named(&mut bag, "bag", Vec2::ZERO);
    let inner = named(&mut bag, "chest", Vec2::ZERO);
    bag.insert(inner, PrefabInstance::new(path("chest.ron")));
    set_parent(&mut bag, inner, sack).expect("a plain reparent");
    crate::format::save(&bag, &registry(), &root.join("bag.ron")).expect("the fixture writes");

    let (mut world, _, chest, coin) = level();
    world.insert(coin, PrefabInstance::new(path("bag.ron")));
    let mut prefabs = Prefabs::new(&root);

    let error = write(&world, &registry(), &mut prefabs, chest, &path("chest.ron"))
        .expect_err("chest → bag → chest");

    assert!(
        matches!(error, WriteError::Cycle { ref through, .. } if through.as_str() == "bag.ron")
    );
}

#[test]
fn a_loop_already_on_disk_that_misses_the_target_ends_rather_than_spinning() {
    // `a.ron` and `b.ron` already hold each other, hand-written. Writing a
    // third prefab that nests `a` must answer, not walk the loop forever.
    let root = scratch_root();
    for (name, nests) in [("a.ron", "b.ron"), ("b.ron", "a.ron")] {
        let mut file = World::new();
        let top = named(&mut file, "top", Vec2::ZERO);
        let inner = named(&mut file, "inner", Vec2::ZERO);
        file.insert(inner, PrefabInstance::new(path(nests)));
        set_parent(&mut file, inner, top).expect("a plain reparent");
        crate::format::save(&file, &registry(), &root.join(name)).expect("the fixture writes");
    }
    let (mut world, _, chest, coin) = level();
    world.insert(coin, PrefabInstance::new(path("a.ron")));
    let mut prefabs = Prefabs::new(&root);

    write(&world, &registry(), &mut prefabs, chest, &path("c.ron")).expect("no cycle through c");
}
