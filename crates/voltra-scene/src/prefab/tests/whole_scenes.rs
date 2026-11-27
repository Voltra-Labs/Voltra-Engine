//! A whole scene spawned as a level: placed nowhere, linked to nothing.

use super::*;

fn level() -> Prefab {
    let mut world = World::new();
    let floor = named(&mut world, "floor", Vec2::new(5.0, -1.0));
    world.insert(floor, Sprite2D::default());
    let coin = named(&mut world, "coin", Vec2::new(2.0, 0.0));
    world.insert(
        coin,
        PrefabInstance::new(AssetPath::new("prefabs/coin.ron").expect("valid")),
    );
    prefab_of(&world).with_source(AssetPath::new("scenes/level.ron").expect("valid"))
}

#[test]
fn a_scene_keeps_its_authored_positions() {
    let mut world = World::new();
    let instance = spawn_scene(&mut world, &registry(), &level()).expect("reads");
    let floor = instance
        .roots
        .iter()
        .find(|e| name_of(&world, **e) == "floor")
        .copied()
        .expect("the floor");
    assert_eq!(
        world.get::<Transform2D>(floor).map(|t| t.translation),
        Some(Vec2::new(5.0, -1.0))
    );
}

#[test]
fn a_scene_is_not_an_instance_of_itself() {
    let mut world = World::new();
    let instance = spawn_scene(&mut world, &registry(), &level()).expect("reads");
    let floor = instance
        .roots
        .iter()
        .find(|e| name_of(&world, **e) == "floor")
        .copied()
        .expect("the floor");
    assert!(world.get::<PrefabInstance>(floor).is_none());
}

#[test]
fn a_prefab_placed_in_the_scene_keeps_its_own_link() {
    let mut world = World::new();
    let instance = spawn_scene(&mut world, &registry(), &level()).expect("reads");
    let coin = instance
        .roots
        .iter()
        .find(|e| name_of(&world, **e) == "coin")
        .copied()
        .expect("the coin");
    let link = world.get::<PrefabInstance>(coin).expect("still linked");
    assert_eq!(link.source.as_str(), "prefabs/coin.ron");
}
