//! The tests of `stash`.

use super::*;
use voltra_scene::{Name, Sprite, Transform};

#[test]
fn an_entity_comes_back_as_it_was_but_for_what_the_wire_brings() {
    let registry = ComponentRegistry::with_defaults();
    let mut world = World::new();
    let ship = world.spawn();
    let ship_id = SceneId::new();
    world.insert(ship, ship_id);
    world.insert(ship, Replicated::default());
    world.insert(ship, Transform::default());
    world.insert(ship, Sprite::default().with_sort_order(4));
    let flame = world.spawn();
    world.insert(flame, SceneId::new());
    world.insert(flame, Name::new("flame"));
    hierarchy::set_parent(&mut world, flame, ship).expect("parents");
    let turret = world.spawn();
    let turret_id = SceneId::new();
    world.insert(turret, turret_id);
    world.insert(turret, Replicated::default());
    world.insert(turret, Transform::default());
    hierarchy::set_parent(&mut world, turret, ship).expect("parents");

    let mut stash = Stash::default();
    let net = |e: Entity| (e == turret).then_some(9);
    let ids = KeepIds {
        net_id: &net,
        stays: &|_| false,
    };
    stash.keep(&mut world, &registry, &["Transform"], ship, &ids);
    assert_eq!(world.entity_count(), 0, "despawned, all of it");
    assert_eq!(stash.len(), 2, "the ship and its turret, each its own");

    let back = stash.restore(&mut world, &registry, ship_id).expect("kept");
    assert_eq!(back.len(), 2, "the ship and its flame");
    assert_eq!(
        world.get::<Sprite>(back[0]).map(|s| s.sort_order),
        Some(4),
        "its own state kept"
    );
    assert!(
        world.get::<Transform>(back[0]).is_none(),
        "the replicated one comes with the spawn"
    );
    hierarchy::resolve_parents(&mut world);
    assert_eq!(hierarchy::parent_of(&world, back[1]), Some(back[0]));
    assert!(
        stash.restore(&mut world, &registry, ship_id).is_none(),
        "once"
    );

    stash.forget(9);
    assert_eq!(stash.len(), 0, "the turret despawned for real");
}

#[test]
fn a_replicated_child_the_server_still_sends_stays_in_the_world() {
    let registry = ComponentRegistry::with_defaults();
    let mut world = World::new();
    let tank = world.spawn();
    world.insert(tank, SceneId::new());
    world.insert(tank, Replicated::default());
    let turret = world.spawn();
    world.insert(turret, SceneId::new());
    world.insert(turret, Replicated::default());
    hierarchy::set_parent(&mut world, turret, tank).expect("parents");
    let barrel = world.spawn();
    world.insert(barrel, SceneId::new());
    hierarchy::set_parent(&mut world, barrel, turret).expect("parents");

    let mut stash = Stash::default();
    let ids = KeepIds {
        net_id: &|_| None,
        stays: &|e| e == turret,
    };
    stash.keep(&mut world, &registry, &[], tank, &ids);
    assert!(!world.is_alive(tank));
    assert!(world.is_alive(turret) && world.is_alive(barrel), "left be");
    assert_eq!(hierarchy::parent_of(&world, turret), None, "unparented");
    assert_eq!(hierarchy::parent_of(&world, barrel), Some(turret));
    assert_eq!(stash.len(), 1, "only the tank set aside");
    stash.forget_unnumbered();
    assert_eq!(
        stash.len(),
        0,
        "swept, never sent: let go of with the level"
    );
}
