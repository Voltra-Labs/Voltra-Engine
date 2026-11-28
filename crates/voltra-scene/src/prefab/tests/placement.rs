//! Where a spawn puts the roots it makes, and what it leaves alone.

use voltra_render::glam::Vec2;

use super::*;

#[test]
fn the_roots_are_placed_and_the_children_keep_their_offsets() {
    let mut world = World::new();
    let at = Transform2D::from_translation(Vec2::new(10.0, 5.0));

    let instance = spawn(&mut world, &registry(), &ship(), at).expect("reads");

    let hull = instance.root().expect("one root");
    let gun = children_of(&world, hull)[0];
    assert_eq!(
        world.get::<Transform2D>(hull).map(|t| t.translation),
        Some(Vec2::new(10.0, 5.0))
    );
    assert_eq!(
        world.get::<Transform2D>(gun).map(|t| t.translation),
        Some(Vec2::new(1.0, 0.0)),
        "local to the hull, which moved it"
    );
}

#[test]
fn a_rotated_placement_turns_the_root_about_its_own_origin() {
    let mut world = World::new();
    let at = Transform2D::from_translation(Vec2::new(3.0, 0.0))
        .with_rotation(std::f32::consts::FRAC_PI_2);

    let instance = spawn(&mut world, &registry(), &ship(), at).expect("reads");

    let hull = world
        .get::<Transform2D>(instance.root().expect("one root"))
        .copied()
        .expect("authored with one");
    assert!((hull.translation - Vec2::new(3.0, 0.0)).length() < 1e-5);
    assert!((hull.rotation - std::f32::consts::FRAC_PI_2).abs() < 1e-5);
}

#[test]
fn a_root_authored_off_the_origin_keeps_its_offset_from_the_placement() {
    let mut source = World::new();
    named(&mut source, "off", Vec2::new(2.0, 0.0));
    let mut world = World::new();

    let instance = spawn(
        &mut world,
        &registry(),
        &prefab_of(&source),
        Transform2D::from_translation(Vec2::new(0.0, 7.0)),
    )
    .expect("reads");

    assert_eq!(
        world
            .get::<Transform2D>(instance.root().expect("one root"))
            .map(|t| t.translation),
        Some(Vec2::new(2.0, 7.0))
    );
}

#[test]
fn a_root_with_no_transform_is_not_given_one() {
    let mut source = World::new();
    let entity = source.spawn();
    source.insert(entity, SceneId::new());
    source.insert(entity, Name::new("listener rig"));
    let mut world = World::new();

    let instance = spawn(
        &mut world,
        &registry(),
        &prefab_of(&source),
        Transform2D::from_translation(Vec2::ONE),
    )
    .expect("reads");

    assert!(world
        .get::<Transform2D>(instance.root().expect("one root"))
        .is_none());
}
