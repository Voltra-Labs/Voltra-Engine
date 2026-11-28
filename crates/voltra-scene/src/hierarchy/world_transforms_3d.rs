//! Every entity's 3D world matrix, composed in one pass: the twin of
//! [`super::WorldTransforms2D`], over the same memoised pass
//! (`compose.rs`).

use voltra_ecs::{Entity, World};
use voltra_render::glam::Affine3A;

use super::compose::Composed;
use crate::transform3d::Transform3D;

/// Every entity's 3D world matrix, composed in one pass.
///
/// For the callers that walk the whole world — a 3D view's draw list, its
/// picking — which would otherwise compose a shared ancestor once per
/// descendant. Keyed by handle, so a stale [`Entity`] misses and
/// [`matrix`](Self::matrix) answers identity for it, as for a root with no
/// transform. No parallax: that is a 2D draw-time displacement.
#[derive(Debug, Default, Clone)]
pub struct WorldTransforms3D {
    composed: Composed<Affine3A>,
}

impl WorldTransforms3D {
    /// Composes every entity with a [`Transform3D`] in one pass.
    ///
    /// Each chain is walked up only until it meets an ancestor already
    /// composed. A [`Parent`](super::Parent) naming a despawned entity, or
    /// one in a cycle, makes its child a root, warned about.
    pub fn from_world(world: &World) -> Self {
        let mut this = Self::default();
        this.composed.compose::<Transform3D>(world);
        this
    }

    /// `entity`'s world matrix, or [`Affine3A::IDENTITY`] if it has none.
    pub fn matrix(&self, entity: Entity) -> Affine3A {
        self.get(entity).unwrap_or(Affine3A::IDENTITY)
    }

    /// `entity`'s world matrix, or `None` if it was not composed.
    pub fn get(&self, entity: Entity) -> Option<Affine3A> {
        self.composed.get(entity)
    }

    /// How many entities have a composed matrix.
    pub fn len(&self) -> usize {
        self.composed.len()
    }

    /// Whether no entity has a composed matrix.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use voltra_render::glam::{EulerRot, Quat, Vec3};

    use super::*;
    use crate::hierarchy::{set_parent, world_matrix_3d, Parent};
    use crate::scene_id::SceneId;
    use crate::Transform2D;

    fn spawn(world: &mut World, transform: Transform3D) -> Entity {
        let entity = world.spawn();
        world.insert(entity, SceneId::new());
        world.insert(entity, transform);
        entity
    }

    fn turned(seed: f32) -> Transform3D {
        Transform3D::from_translation(Vec3::new(seed, -2.0 * seed, 0.5))
            .with_rotation(Quat::from_euler(
                EulerRot::YXZ,
                0.7 * seed,
                -0.3,
                0.2 * seed,
            ))
            .with_scale(Vec3::new(1.0 + 0.1 * seed, 0.9, 1.2))
    }

    #[test]
    fn the_pass_equals_the_walk_to_a_millionth() {
        // A chain six deep and a fork, every link turned, moved and scaled.
        let mut world = World::new();
        let mut chain = vec![spawn(&mut world, turned(0.0))];
        for i in 1..6 {
            let next = spawn(&mut world, turned(i as f32 * 0.37));
            set_parent(&mut world, next, chain[i - 1]).expect("a plain reparent");
            chain.push(next);
        }
        let fork = spawn(&mut world, turned(2.5));
        set_parent(&mut world, fork, chain[2]).expect("a plain reparent");
        chain.push(fork);

        let pass = WorldTransforms3D::from_world(&world);

        assert_eq!(pass.len(), chain.len());
        for entity in chain {
            let walked = world_matrix_3d(&world, entity);
            let composed = pass.matrix(entity);
            assert!(
                walked.abs_diff_eq(composed, 1e-6),
                "{entity:?}: walk {walked:?} vs pass {composed:?}"
            );
        }
    }

    #[test]
    fn a_cycle_in_the_data_terminates() {
        let mut world = World::new();
        let a = spawn(&mut world, turned(1.0));
        let b = spawn(&mut world, turned(2.0));
        let a_id = *world.get::<SceneId>(a).expect("spawned with one");
        let b_id = *world.get::<SceneId>(b).expect("spawned with one");
        world.insert(a, Parent::resolved(b_id, b));
        world.insert(b, Parent::resolved(a_id, a));

        let pass = WorldTransforms3D::from_world(&world);

        assert_eq!(pass.len(), 2, "both composed, neither hangs");
        let _ = world_matrix_3d(&world, a);
    }

    #[test]
    fn a_2d_entity_is_not_composed_and_adds_no_frame() {
        // A file that mixes the two, left alone: the 2D link above a 3D
        // child is not a frame of the child's.
        let mut world = World::new();
        let flat = world.spawn();
        world.insert(flat, SceneId::new());
        world.insert(
            flat,
            Transform2D::from_translation(voltra_render::glam::Vec2::new(100.0, 0.0)),
        );
        let deep = spawn(&mut world, Transform3D::from_translation(Vec3::X));
        let id = *world.get::<SceneId>(flat).expect("spawned with one");
        world.insert(deep, Parent::resolved(id, flat));

        let pass = WorldTransforms3D::from_world(&world);

        assert_eq!(
            pass.get(flat).map(|m| m.translation),
            Some(Vec3::ZERO.into())
        );
        assert!(pass
            .matrix(deep)
            .translation
            .abs_diff_eq(Vec3::X.into(), 1e-6));
        assert_eq!(WorldTransforms3D::from_world(&World::new()).len(), 0);
    }
}
