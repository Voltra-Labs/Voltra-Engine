//! Prefabs spawned from a tick and before the first one.

use super::*;
use std::cell::RefCell;
use std::rc::Rc;
use voltra_render::glam::Vec2;
use voltra_scene::{ComponentRegistry, Name, SceneId};
use voltra_testkit::scratch_root;

/// An asset root holding `coin.ron`: one named, positioned entity.
fn root_with_a_coin() -> voltra_testkit::Scratch {
    let root = scratch_root();
    let mut world = voltra_ecs::World::new();
    let coin = world.spawn();
    world.insert(coin, SceneId::new());
    world.insert(coin, Name::new("coin"));
    world.insert(coin, Transform::default());
    voltra_scene::format::save(
        &world,
        &ComponentRegistry::with_defaults(),
        &root.join("coin.ron"),
    )
    .expect("the fixture writes");
    root
}

fn coin() -> AssetPath {
    AssetPath::new("coin.ron").expect("valid")
}

#[test]
fn a_tick_spawns_a_prefab_where_it_is_told() {
    let spawned = Rc::new(RefCell::new(Vec::new()));
    let seen = spawned.clone();
    let root = root_with_a_coin();
    let mut app = App::default()
        .with_asset_root(&root)
        .with_simulation()
        .with_update(move |tick| {
            let coin = tick.prefab(&coin());
            let instance = tick
                .spawn(coin, &Transform::from_translation(Vec2::new(3.0, 4.0)))
                .expect("the prefab reads");
            seen.borrow_mut().extend(instance.roots);
        });

    app.advance(1.0 / 60.0);

    let roots = spawned.borrow();
    assert_eq!(roots.len(), 1);
    assert_eq!(
        app.world.get::<Transform>(roots[0]).map(|t| t.translation),
        Some(Vec2::new(3.0, 4.0))
    );
    assert!(app.spawned.is_empty(), "the queue was resolved and emptied");
}

#[test]
fn a_tick_that_spawns_every_frame_makes_one_per_frame() {
    let root = root_with_a_coin();
    let mut app = App::default()
        .with_asset_root(&root)
        .with_simulation()
        .with_update(|tick| {
            let coin = tick.prefab(&coin());
            tick.spawn(coin, &Transform::default()).expect("reads");
        });

    for _ in 0..3 {
        app.advance(1.0 / 60.0);
    }

    assert_eq!(app.world.component_count::<Name>(), 3);
}

#[test]
fn a_forged_handle_spawns_nothing_rather_than_panicking() {
    let root = scratch_root();
    let mut app = App::default()
        .with_asset_root(&root)
        .with_simulation()
        .with_update(|tick| {
            let instance = tick
                .spawn(Handle::forge(40, 0), &Transform::default())
                .expect("not an error");
            assert!(instance.entities.is_empty());
        });

    app.advance(1.0 / 60.0);

    assert_eq!(app.world.entity_count(), 0);
}

#[test]
fn an_entity_despawned_by_the_tick_that_made_it_is_skipped() {
    let root = root_with_a_coin();
    let mut app = App::default()
        .with_asset_root(&root)
        .with_simulation()
        .with_update(|tick| {
            let coin = tick.prefab(&coin());
            let instance = tick.spawn(coin, &Transform::default()).expect("reads");
            for entity in instance.entities {
                tick.world.despawn(entity);
            }
        });

    app.advance(1.0 / 60.0);

    assert_eq!(app.world.entity_count(), 0);
}

#[test]
fn a_game_can_load_its_prefabs_before_the_first_tick() {
    let root = root_with_a_coin();
    let mut app = App::default().with_asset_root(&root);

    let handle = app.prefabs().load(&coin());

    assert_eq!(app.prefabs().get(handle).len(), 1);
}
