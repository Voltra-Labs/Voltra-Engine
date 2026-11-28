//! Where a parented 3D entity actually is: the single lookups, twins of
//! [`super::world_transform`]'s, over the same arithmetic
//! (`compose.rs`).
//!
//! A chain composes only [`Transform3D`]s: an ancestor without one — a
//! grouping node, or a 2D one in a file that mixes the two — adds no frame.

use voltra_ecs::{Entity, World};
use voltra_render::glam::Affine3A;

use super::compose;
use crate::transform3d::Transform3D;

/// The frame `entity` sits in: every ancestor's matrix, composed.
/// [`Affine3A::IDENTITY`] for a root.
pub fn parent_matrix_3d(world: &World, entity: Entity) -> Affine3A {
    compose::parent_matrix::<Transform3D>(world, entity)
}

/// Where `entity` is in world space, as a matrix.
pub fn world_matrix_3d(world: &World, entity: Entity) -> Affine3A {
    compose::world_matrix::<Transform3D>(world, entity)
}

/// Where `entity` is in world space, as a transform. Lossy for the shear a
/// chain can express; see [`Transform3D::from_matrix`].
pub fn world_transform_3d(world: &World, entity: Entity) -> Transform3D {
    Transform3D::from_matrix(world_matrix_3d(world, entity))
}

/// Puts `entity` at `target` in **world** space, writing its local
/// transform; whether it was written. An entity with no `Transform3D`, or
/// one under a collapsed parent, is left exactly as it was.
pub fn set_world_transform_3d(world: &mut World, entity: Entity, target: &Transform3D) -> bool {
    compose::set_world(world, entity, target)
}

#[cfg(test)]
mod tests {
    use std::f32::consts::FRAC_PI_2;

    use voltra_render::glam::{Quat, Vec3};

    use super::*;
    use crate::hierarchy::link::set_parent;
    use crate::scene_id::SceneId;

    fn spawn(world: &mut World, transform: Transform3D) -> Entity {
        let entity = world.spawn();
        world.insert(entity, SceneId::new());
        world.insert(entity, transform);
        entity
    }

    fn at(x: f32, y: f32, z: f32) -> Transform3D {
        Transform3D::from_translation(Vec3::new(x, y, z))
    }

    fn position(world: &World, entity: Entity) -> Vec3 {
        world_matrix_3d(world, entity).transform_point3(Vec3::ZERO)
    }

    #[test]
    fn a_root_is_where_its_own_transform_says() {
        let mut world = World::new();
        let entity = spawn(&mut world, at(1.0, 2.0, 3.0));
        assert_eq!(parent_matrix_3d(&world, entity), Affine3A::IDENTITY);
        assert!((position(&world, entity) - Vec3::new(1.0, 2.0, 3.0)).length() < 1e-6);
    }

    #[test]
    fn a_chain_rotates_the_offset_then_translates_it() {
        // The order that matters: the parent's quarter turn about Y carries
        // the child's offset of one along X to −Z, and only then is the
        // parent's translation added. The other order would put the child
        // at (11, 0, 0) turned, or at (0, 0, −11).
        let mut world = World::new();
        let grandparent = spawn(&mut world, at(0.0, 5.0, 0.0));
        let parent = spawn(
            &mut world,
            at(10.0, 0.0, 0.0).with_rotation(Quat::from_rotation_y(FRAC_PI_2)),
        );
        let child = spawn(&mut world, at(1.0, 0.0, 0.0));
        set_parent(&mut world, parent, grandparent).expect("a plain reparent");
        set_parent(&mut world, child, parent).expect("a plain reparent");

        let p = position(&world, child);
        assert!((p - Vec3::new(10.0, 5.0, -1.0)).length() < 1e-5, "{p:?}");
        let composed = world_transform_3d(&world, child);
        assert!(composed
            .rotation
            .abs_diff_eq(Quat::from_rotation_y(FRAC_PI_2), 1e-6));
    }

    #[test]
    fn a_grouping_node_without_a_transform_passes_its_children_through() {
        let mut world = World::new();
        let root = spawn(&mut world, at(0.0, 0.0, 4.0));
        let group = world.spawn();
        world.insert(group, SceneId::new());
        let child = spawn(&mut world, at(1.0, 0.0, 0.0));
        set_parent(&mut world, group, root).expect("a plain reparent");
        set_parent(&mut world, child, group).expect("a plain reparent");

        assert!((position(&world, child) - Vec3::new(1.0, 0.0, 4.0)).length() < 1e-6);
    }

    #[test]
    fn a_world_transform_round_trips_through_a_turned_parent() {
        let mut world = World::new();
        let parent = spawn(
            &mut world,
            at(3.0, -2.0, 1.0)
                .with_rotation(Quat::from_euler(
                    voltra_render::glam::EulerRot::YXZ,
                    0.9,
                    0.3,
                    -0.4,
                ))
                .with_scale(Vec3::splat(1.5)),
        );
        let child = spawn(&mut world, Transform3D::default());
        set_parent(&mut world, child, parent).expect("a plain reparent");

        let target = at(-4.0, 6.0, 2.0)
            .with_rotation(Quat::from_rotation_x(0.2))
            .with_scale(Vec3::splat(3.0));
        assert!(set_world_transform_3d(&mut world, child, &target));

        let back = world_transform_3d(&world, child);
        assert!((back.translation - target.translation).length() < 1e-4);
        assert!(back.rotation.abs_diff_eq(target.rotation, 1e-4));
        assert!((back.scale - target.scale).length() < 1e-4);
    }

    #[test]
    fn a_collapsed_parent_leaves_the_child_alone() {
        let mut world = World::new();
        let parent = spawn(&mut world, Transform3D::default().with_scale(Vec3::ZERO));
        let child = spawn(&mut world, at(1.0, 1.0, 1.0));
        set_parent(&mut world, child, parent).expect("a plain reparent");

        assert!(!set_world_transform_3d(
            &mut world,
            child,
            &at(5.0, 5.0, 5.0)
        ));
        let local = world.get::<Transform3D>(child).expect("still has one");
        assert_eq!(local.translation, Vec3::ONE);
    }

    #[test]
    fn an_entity_without_a_transform_cannot_be_placed() {
        let mut world = World::new();
        let bare = world.spawn();
        assert!(!set_world_transform_3d(
            &mut world,
            bare,
            &at(1.0, 1.0, 1.0)
        ));
    }
}
