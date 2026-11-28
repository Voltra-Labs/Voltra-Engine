use voltra_render::glam::{Vec2, Vec3};

use super::*;
use crate::hierarchy::{can_parent, resolve_parents, set_parent, Parent};
use crate::scene_id::SceneId;

fn flat(world: &mut World) -> Entity {
    let entity = world.spawn();
    world.insert(entity, SceneId::new());
    world.insert(entity, Transform2D::from_translation(Vec2::new(1.0, 2.0)));
    entity
}

fn deep(world: &mut World) -> Entity {
    let entity = world.spawn();
    world.insert(entity, SceneId::new());
    world.insert(
        entity,
        Transform3D::from_translation(Vec3::new(1.0, 2.0, 3.0)),
    );
    entity
}

fn group(world: &mut World) -> Entity {
    let entity = world.spawn();
    world.insert(entity, SceneId::new());
    entity
}

/// A link written as a file would, past `set_parent`'s rules.
fn link(world: &mut World, child: Entity, parent: Entity) {
    let id = *world.get::<SceneId>(parent).expect("spawned with one");
    world.insert(child, Parent { id, entity: None });
}

#[test]
fn linking_across_dimensions_is_refused_both_ways() {
    let mut world = World::new();
    let (two, three) = (flat(&mut world), deep(&mut world));

    assert_eq!(
        set_parent(&mut world, three, two),
        Err(HierarchyError::MixedDimensions)
    );
    assert_eq!(
        set_parent(&mut world, two, three),
        Err(HierarchyError::MixedDimensions)
    );
    assert_eq!(parent_of(&world, three), None, "nothing was written");
    assert_eq!(parent_of(&world, two), None);
}

#[test]
fn a_grouping_node_passes_the_rule_through() {
    // No transform adds no frame, so it neither mixes nor shields a mix:
    // a 3D child under a group under a 2D root is still 3D under 2D.
    let mut world = World::new();
    let root = flat(&mut world);
    let holder = group(&mut world);
    let three = deep(&mut world);
    let two = flat(&mut world);

    set_parent(&mut world, holder, root).expect("a group may hang anywhere");
    set_parent(&mut world, two, holder).expect("2D under 2D, through the group");
    assert_eq!(
        set_parent(&mut world, three, holder),
        Err(HierarchyError::MixedDimensions)
    );

    // And from below: a group holding a 3D child cannot go under 2D.
    let other = group(&mut world);
    let three_again = deep(&mut world);
    set_parent(&mut world, three_again, other).expect("3D under a bare group");
    assert_eq!(
        set_parent(&mut world, other, root),
        Err(HierarchyError::MixedDimensions)
    );
    assert_eq!(can_parent(&world, other, three), Ok(()), "3D over 3D");
}

#[test]
fn same_dimension_links_are_untouched_by_the_rule() {
    let mut world = World::new();
    let (a, b) = (deep(&mut world), deep(&mut world));
    let (c, d) = (flat(&mut world), flat(&mut world));
    set_parent(&mut world, b, a).expect("3D under 3D");
    set_parent(&mut world, d, c).expect("2D under 2D");
}

#[test]
fn a_file_that_links_across_is_warned_about_and_left_alone() {
    let mut world = World::new();
    let two = flat(&mut world);
    let three = deep(&mut world);
    link(&mut world, three, two);

    resolve_parents(&mut world);

    assert_eq!(
        problems(&world),
        vec![Problem::MixedLink {
            child: three,
            above: two
        }]
    );
    assert_eq!(parent_of(&world, three), Some(two), "the link is kept");
    assert!(world.get::<Transform3D>(three).is_some());
}

#[test]
fn an_entity_with_both_transforms_is_warned_about_and_kept() {
    let mut world = World::new();
    let both = flat(&mut world);
    world.insert(both, Transform3D::default());

    resolve_parents(&mut world);

    assert_eq!(problems(&world), vec![Problem::BothTransforms(both)]);
    assert!(
        world.get::<Transform2D>(both).is_some(),
        "neither is dropped"
    );
    assert!(world.get::<Transform3D>(both).is_some());
}

#[test]
fn a_2d_world_has_nothing_to_find() {
    let mut world = World::new();
    let a = flat(&mut world);
    let b = flat(&mut world);
    set_parent(&mut world, b, a).expect("2D under 2D");
    assert!(problems(&world).is_empty());
}

#[test]
fn a_second_transform_is_refused_and_a_first_one_checked() {
    let mut world = World::new();
    let two = flat(&mut world);
    assert_eq!(
        can_take_transform(&world, two, Dimension::D3),
        Err(HierarchyError::SecondTransform)
    );
    assert_eq!(can_take_transform(&world, two, Dimension::D2), Ok(()));

    // A bare child of a 2D parent may become 2D, not 3D.
    let bare = group(&mut world);
    set_parent(&mut world, bare, two).expect("a group under 2D");
    assert_eq!(can_take_transform(&world, bare, Dimension::D2), Ok(()));
    assert_eq!(
        can_take_transform(&world, bare, Dimension::D3),
        Err(HierarchyError::MixedDimensions)
    );

    // A bare parent of a 3D child may become 3D, not 2D.
    let holder = group(&mut world);
    let three = deep(&mut world);
    set_parent(&mut world, three, holder).expect("3D under a group");
    assert_eq!(can_take_transform(&world, holder, Dimension::D3), Ok(()));
    assert_eq!(
        can_take_transform(&world, holder, Dimension::D2),
        Err(HierarchyError::MixedDimensions)
    );
}

#[test]
fn the_names_name_the_dimensions() {
    assert_eq!(
        Dimension::of_transform_name("Transform3D"),
        Some(Dimension::D3)
    );
    assert_eq!(
        Dimension::of_transform_name(Transform2D::NAME),
        Some(Dimension::D2)
    );
    assert_eq!(Dimension::of_transform_name("Sprite2D"), None);
}
