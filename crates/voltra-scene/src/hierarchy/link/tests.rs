use super::*;
use crate::Transform2D;

/// An entity that can be a parent: it carries an identity.
fn spawn(world: &mut World) -> Entity {
    let entity = world.spawn();
    world.insert(entity, SceneId::new());
    world.insert(entity, Transform2D::default());
    entity
}

#[test]
fn a_child_points_at_its_parent() {
    let mut world = World::new();
    let parent = spawn(&mut world);
    let child = spawn(&mut world);

    set_parent(&mut world, child, parent).expect("a plain reparent");

    assert_eq!(parent_of(&world, child), Some(parent));
    assert_eq!(children_of(&world, parent), vec![child]);
}

#[test]
fn the_link_carries_the_parents_identity() {
    // Not the `Entity`: that is what makes the link survive a save.
    let mut world = World::new();
    let parent = spawn(&mut world);
    let child = spawn(&mut world);
    let id = *world.get::<SceneId>(parent).expect("spawned with one");

    set_parent(&mut world, child, parent).expect("a plain reparent");

    assert_eq!(world.get::<Parent>(child).map(|link| link.id), Some(id));
}

#[test]
fn reparenting_replaces_the_previous_link() {
    let mut world = World::new();
    let first = spawn(&mut world);
    let second = spawn(&mut world);
    let child = spawn(&mut world);

    set_parent(&mut world, child, first).expect("a plain reparent");
    set_parent(&mut world, child, second).expect("and another");

    assert_eq!(parent_of(&world, child), Some(second));
    assert!(children_of(&world, first).is_empty());
}

#[test]
fn an_entity_cannot_be_its_own_parent() {
    let mut world = World::new();
    let entity = spawn(&mut world);

    assert_eq!(
        set_parent(&mut world, entity, entity),
        Err(HierarchyError::Itself)
    );
    assert_eq!(parent_of(&world, entity), None);
}

#[test]
fn a_cycle_is_refused() {
    // The one that matters: dragging a parent onto its own grandchild. The
    // tree would have no root, and every walk up it would run forever.
    let mut world = World::new();
    let grandparent = spawn(&mut world);
    let parent = spawn(&mut world);
    let child = spawn(&mut world);

    set_parent(&mut world, parent, grandparent).expect("a plain reparent");
    set_parent(&mut world, child, parent).expect("a plain reparent");

    assert_eq!(
        set_parent(&mut world, grandparent, child),
        Err(HierarchyError::Cycle)
    );
    assert_eq!(parent_of(&world, grandparent), None);
}

#[test]
fn a_parent_without_an_identity_is_refused() {
    let mut world = World::new();
    let child = spawn(&mut world);
    let transient = world.spawn();
    world.insert(transient, Transform2D::default());

    assert_eq!(
        set_parent(&mut world, child, transient),
        Err(HierarchyError::ParentHasNoIdentity)
    );
}

#[test]
fn a_physics_body_cannot_be_parented() {
    // Physics reads `Transform2D` as world space. Until the solver gathers
    // world transforms, a parented body simulates in the wrong frame.
    let mut world = World::new();
    let parent = spawn(&mut world);
    let body = spawn(&mut world);
    world.insert(body, RigidBody2D::new_dynamic(1.0));

    assert_eq!(
        set_parent(&mut world, body, parent),
        Err(HierarchyError::PhysicsBody)
    );
}

#[test]
fn nothing_can_be_parented_to_a_physics_body_either() {
    // The parent moves under the solver every step, and the child's local
    // transform would be measured against a frame nothing else knows about.
    let mut world = World::new();
    let body = spawn(&mut world);
    world.insert(body, Collider2D::Circle { radius: 1.0 });
    let child = spawn(&mut world);

    assert_eq!(
        set_parent(&mut world, child, body),
        Err(HierarchyError::PhysicsBody)
    );
}

#[test]
fn can_parent_answers_without_writing_anything() {
    // What the hierarchy panel asks on every frame of a drag. It must agree
    // with `set_parent` and must not change the scene to find out.
    let mut world = World::new();
    let parent = spawn(&mut world);
    let child = spawn(&mut world);
    set_parent(&mut world, child, parent).expect("a plain reparent");

    assert_eq!(
        can_parent(&world, parent, child),
        Err(HierarchyError::Cycle)
    );
    assert_eq!(parent_of(&world, parent), None, "asking wrote nothing");

    let other = spawn(&mut world);
    assert_eq!(can_parent(&world, other, parent), Ok(()));
    assert_eq!(parent_of(&world, other), None, "still only an answer");
}

#[test]
fn clearing_a_link_makes_a_root_again() {
    let mut world = World::new();
    let parent = spawn(&mut world);
    let child = spawn(&mut world);
    set_parent(&mut world, child, parent).expect("a plain reparent");

    assert!(clear_parent(&mut world, child));
    assert_eq!(parent_of(&world, child), None);
    assert!(roots(&world).contains(&child));
}

#[test]
fn clearing_a_link_that_is_not_there_reports_it() {
    let mut world = World::new();
    let entity = spawn(&mut world);
    assert!(!clear_parent(&mut world, entity));
}
