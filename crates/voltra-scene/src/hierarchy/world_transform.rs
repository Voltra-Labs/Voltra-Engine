//! Where a parented 2D entity actually is.
//!
//! A [`Transform2D`] is local to its parent. Everything that draws, picks or
//! points at an entity needs the composed matrix instead, and there are two
//! shapes of that need: one entity at a time (a gizmo, an inspector) and every
//! entity at once (a frame of sprites). [`world_matrix_2d`] serves the first by
//! walking up the chain; [`WorldTransforms2D`] serves the second by composing the
//! whole world once, so a deep tree costs one multiply per entity rather than
//! one per entity per level. Both are `compose.rs`'s arithmetic, which
//! 3D's twins ([`super::world_transform_3d`]) share.

use voltra_ecs::{Entity, World};
use voltra_render::glam::Mat3;

use super::compose;
use super::walk::parent_of;
use crate::transform::Transform2D;

/// The memoised pass, on the path 0.4 had it.
pub use super::world_transforms::WorldTransforms2D;

/// The 0.4 names, where 0.4 had them; see [`crate::renamed`].
#[allow(deprecated)]
pub use crate::renamed::{
    parent_matrix, set_world_transform, world_matrix, world_transform, WorldTransforms,
};

/// The frame `entity` sits in: every ancestor's matrix, composed.
///
/// [`Mat3::IDENTITY`] for a root, which is what makes a root's local transform
/// its world transform with no special case anywhere else. An ancestor with
/// no `Transform2D` — a grouping node, or a 3D one in a file that mixes the
/// two — adds no frame.
pub fn parent_matrix_2d(world: &World, entity: Entity) -> Mat3 {
    compose::parent_matrix::<Transform2D>(world, entity)
}

/// Where `entity` is in world space, as a matrix.
pub fn world_matrix_2d(world: &World, entity: Entity) -> Mat3 {
    compose::world_matrix::<Transform2D>(world, entity)
}

/// Where `entity` is in world space, as a transform.
///
/// Decomposed, so it is what a gizmo can drag and an overlay can draw. See
/// [`Transform2D::from_matrix`] for what a chain of parents can express that this
/// cannot.
pub fn world_transform_2d(world: &World, entity: Entity) -> Transform2D {
    Transform2D::from_matrix(world_matrix_2d(world, entity))
}

/// Puts `entity` at `target` in **world** space, writing its local transform.
///
/// The inverse of [`world_transform_2d`], and the way a drag that computed a
/// world-space answer lands on a child. Returns whether it was written: an
/// entity with no `Transform2D` has nowhere to put one, and a parent whose own
/// frame has collapsed to a point (a zero scale) gives no way back from world
/// space at all. Both leave the entity exactly as it was rather than writing a
/// NaN that would make it disappear.
pub fn set_world_transform_2d(world: &mut World, entity: Entity, target: &Transform2D) -> bool {
    compose::set_world(world, entity, target)
}

/// Whether `entity` is drawn where its own transform says it is.
///
/// The cheap test for the common case: most entities are roots, and a caller
/// that knows one is can skip composing anything.
pub fn is_root(world: &World, entity: Entity) -> bool {
    parent_of(world, entity).is_none()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hierarchy::link::set_parent;
    use crate::scene_id::SceneId;
    use std::f32::consts::FRAC_PI_2;
    use voltra_render::glam::Vec2;

    fn spawn(world: &mut World, transform: Transform2D) -> Entity {
        let entity = world.spawn();
        world.insert(entity, SceneId::new());
        world.insert(entity, transform);
        entity
    }

    fn at(x: f32, y: f32) -> Transform2D {
        Transform2D::from_translation(Vec2::new(x, y))
    }

    fn position(world: &World, entity: Entity) -> Vec2 {
        world_matrix_2d(world, entity).transform_point2(Vec2::ZERO)
    }

    #[test]
    fn a_root_is_where_its_own_transform_says() {
        let mut world = World::new();
        let entity = spawn(&mut world, at(3.0, 4.0));

        assert_eq!(parent_matrix_2d(&world, entity), Mat3::IDENTITY);
        assert!((position(&world, entity) - Vec2::new(3.0, 4.0)).length() < 1e-5);
        assert!(is_root(&world, entity));
    }

    #[test]
    fn a_child_is_offset_by_its_parent() {
        let mut world = World::new();
        let parent = spawn(&mut world, at(10.0, 0.0));
        let child = spawn(&mut world, at(1.0, 2.0));
        set_parent(&mut world, child, parent).expect("a plain reparent");

        assert!((position(&world, child) - Vec2::new(11.0, 2.0)).length() < 1e-5);
    }

    #[test]
    fn a_child_turns_and_grows_with_its_parent() {
        // The property that makes a hierarchy worth having: the parent's
        // rotation and scale carry the child's offset with them.
        let mut world = World::new();
        let parent = spawn(
            &mut world,
            Transform2D::default()
                .with_rotation(FRAC_PI_2)
                .with_scale(Vec2::splat(2.0)),
        );
        let child = spawn(&mut world, at(1.0, 0.0));
        set_parent(&mut world, child, parent).expect("a plain reparent");

        // Offset one to the right, scaled by two and turned a quarter turn:
        // two units up.
        assert!((position(&world, child) - Vec2::new(0.0, 2.0)).length() < 1e-5);

        let composed = world_transform_2d(&world, child);
        assert!((composed.rotation - FRAC_PI_2).abs() < 1e-5);
        assert!((composed.scale - Vec2::splat(2.0)).length() < 1e-5);
    }

    #[test]
    fn three_levels_compose_in_order() {
        let mut world = World::new();
        let grandparent = spawn(&mut world, at(100.0, 0.0));
        let parent = spawn(&mut world, at(10.0, 0.0));
        let child = spawn(&mut world, at(1.0, 0.0));
        set_parent(&mut world, parent, grandparent).expect("a plain reparent");
        set_parent(&mut world, child, parent).expect("a plain reparent");

        assert!((position(&world, child) - Vec2::new(111.0, 0.0)).length() < 1e-5);
    }

    #[test]
    fn setting_a_world_transform_writes_the_local_one() {
        let mut world = World::new();
        let parent = spawn(&mut world, at(10.0, 0.0));
        let child = spawn(&mut world, at(0.0, 0.0));
        set_parent(&mut world, child, parent).expect("a plain reparent");

        assert!(set_world_transform_2d(&mut world, child, &at(12.0, 3.0)));

        let local = world.get::<Transform2D>(child).expect("still has one");
        assert!((local.translation - Vec2::new(2.0, 3.0)).length() < 1e-5);
        assert!((position(&world, child) - Vec2::new(12.0, 3.0)).length() < 1e-5);
    }

    #[test]
    fn setting_a_world_transform_on_a_root_writes_it_unchanged() {
        let mut world = World::new();
        let entity = spawn(&mut world, at(0.0, 0.0));

        assert!(set_world_transform_2d(&mut world, entity, &at(5.0, 5.0)));

        let local = world.get::<Transform2D>(entity).expect("still has one");
        assert_eq!(local.translation, Vec2::new(5.0, 5.0));
    }

    #[test]
    fn a_world_transform_round_trips_through_a_turned_parent() {
        let mut world = World::new();
        let parent = spawn(
            &mut world,
            Transform2D::from_translation(Vec2::new(3.0, -2.0))
                .with_rotation(0.9)
                .with_scale(Vec2::splat(1.5)),
        );
        let child = spawn(&mut world, at(0.0, 0.0));
        set_parent(&mut world, child, parent).expect("a plain reparent");

        let target = Transform2D::from_translation(Vec2::new(-4.0, 6.0))
            .with_rotation(0.2)
            .with_scale(Vec2::splat(3.0));
        assert!(set_world_transform_2d(&mut world, child, &target));

        let back = world_transform_2d(&world, child);
        assert!((back.translation - target.translation).length() < 1e-4);
        assert!((back.rotation - target.rotation).abs() < 1e-4);
        assert!((back.scale - target.scale).length() < 1e-4);
    }

    #[test]
    fn a_collapsed_parent_leaves_the_child_alone() {
        // A zero-scaled parent maps every point to its origin, so no world
        // position has a local answer. Writing one anyway means NaN.
        let mut world = World::new();
        let parent = spawn(&mut world, Transform2D::default().with_scale(Vec2::ZERO));
        let child = spawn(&mut world, at(1.0, 1.0));
        set_parent(&mut world, child, parent).expect("a plain reparent");

        assert!(!set_world_transform_2d(&mut world, child, &at(5.0, 5.0)));

        let local = world.get::<Transform2D>(child).expect("still has one");
        assert_eq!(local.translation, Vec2::new(1.0, 1.0));
    }

    #[test]
    fn an_entity_without_a_transform_cannot_be_placed() {
        let mut world = World::new();
        let bare = world.spawn();
        assert!(!set_world_transform_2d(&mut world, bare, &at(1.0, 1.0)));
    }
}
