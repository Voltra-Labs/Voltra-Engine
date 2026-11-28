//! Every entity's 2D world matrix, composed in one pass.
//!
//! The memoised twin of [`super::world_transform`]'s single lookups: a frame
//! of sprites, or a pick, walks the whole world, and composing each chain
//! from the root would cost one multiply per entity per level. The pass
//! itself is `compose.rs`, shared with 3D's.

use voltra_ecs::{Entity, World};
use voltra_render::glam::{Mat3, Vec2};

use super::compose::Composed;
use crate::transform::Transform2D;

/// Every entity's world matrix, composed in one pass.
///
/// Built per frame by the sprite batch and by picking, which both walk the whole
/// world and would otherwise recompute a shared ancestor's matrix once per
/// descendant. Each entity is composed exactly once: the walk up stops at the
/// first ancestor already in the map.
///
/// Entities are keyed by handle, so a stale [`Entity`] from a previous frame
/// simply misses — [`matrix`](Self::matrix) answers identity for anything it
/// does not hold, which is the same answer a root with no transform gets.
#[derive(Debug, Default, Clone)]
pub struct WorldTransforms2D {
    composed: Composed<Mat3>,
}

impl WorldTransforms2D {
    /// Composes every entity with a [`Transform2D`] in one pass.
    ///
    /// Each chain is walked up only until it meets an ancestor already
    /// composed, so every matrix is multiplied once however deep the tree.
    /// A [`Parent`](super::Parent) naming a despawned entity, or one in a
    /// cycle, makes its child a root: drawn where its own transform says,
    /// and warned about.
    pub fn from_world(world: &World) -> Self {
        let mut this = Self::default();
        this.composed.compose::<Transform2D>(world);
        this
    }

    /// [`seen_from`](Self::seen_from) into this one, keeping its storage: a
    /// draw composing every frame reuses last frame's rather than
    /// allocating (and faulting in) megabytes for a large world afresh.
    pub fn recompose_seen_from(&mut self, world: &World, camera: Vec2) {
        self.composed.compose::<Transform2D>(world);
        crate::parallax::apply(self, world, camera);
    }

    /// [`from_world`](Self::from_world), with every [`Parallax`] layer
    /// displaced for a camera at `camera` — what the draw and the pick see.
    ///
    /// [`Parallax`]: crate::Parallax
    pub fn seen_from(world: &World, camera: Vec2) -> Self {
        let mut transforms = Self::from_world(world);
        crate::parallax::apply(&mut transforms, world, camera);
        transforms
    }

    /// Every entity composed.
    pub(crate) fn entities(&self) -> impl Iterator<Item = Entity> + '_ {
        self.composed.entities()
    }

    /// Moves `entity`'s composed matrix by `offset` in world space.
    pub(crate) fn translate(&mut self, entity: Entity, offset: Vec2) {
        if let Some(matrix) = self.composed.get_mut(entity) {
            *matrix = Mat3::from_translation(offset) * *matrix;
        }
    }

    /// `entity`'s world matrix, or [`Mat3::IDENTITY`] if it has none.
    pub fn matrix(&self, entity: Entity) -> Mat3 {
        self.get(entity).unwrap_or(Mat3::IDENTITY)
    }

    /// `entity`'s world matrix, or `None` if it was not composed.
    pub fn get(&self, entity: Entity) -> Option<Mat3> {
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
    use super::*;
    use crate::hierarchy::link::set_parent;
    use crate::hierarchy::world_transform::world_matrix_2d;
    use crate::hierarchy::Parent;
    use crate::scene_id::SceneId;
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

    #[test]
    fn the_cache_agrees_with_the_walk() {
        let mut world = World::new();
        let parent = spawn(
            &mut world,
            Transform2D::from_translation(Vec2::new(4.0, -1.0))
                .with_rotation(0.7)
                .with_scale(Vec2::new(2.0, 2.0)),
        );
        let child = spawn(&mut world, at(1.0, 3.0));
        let grandchild = spawn(&mut world, at(0.0, 1.0));
        set_parent(&mut world, child, parent).expect("a plain reparent");
        set_parent(&mut world, grandchild, child).expect("a plain reparent");

        let cache = WorldTransforms2D::from_world(&world);

        for entity in [parent, child, grandchild] {
            let walked = world_matrix_2d(&world, entity);
            let cached = cache.matrix(entity);
            assert!(
                (walked - cached).abs().to_cols_array().iter().sum::<f32>() < 1e-4,
                "{entity:?}: walk {walked:?} vs cache {cached:?}"
            );
        }
    }

    #[test]
    fn the_cache_holds_every_transform_and_nothing_else() {
        let mut world = World::new();
        spawn(&mut world, at(1.0, 0.0));
        spawn(&mut world, at(2.0, 0.0));
        let bare = world.spawn();
        world.insert(bare, SceneId::new());

        let cache = WorldTransforms2D::from_world(&world);

        assert_eq!(cache.len(), 2);
        assert_eq!(cache.get(bare), None);
        assert_eq!(
            cache.matrix(bare),
            Mat3::IDENTITY,
            "an entity with no transform adds no frame"
        );
    }

    #[test]
    fn the_cache_survives_a_cycle_in_the_data() {
        // Only a hand-edited file makes this; the editor still has to open it.
        let mut world = World::new();
        let a = spawn(&mut world, at(1.0, 0.0));
        let b = spawn(&mut world, at(0.0, 1.0));
        let a_id = *world.get::<SceneId>(a).expect("spawned with one");
        let b_id = *world.get::<SceneId>(b).expect("spawned with one");
        world.insert(a, Parent::resolved(b_id, b));
        world.insert(b, Parent::resolved(a_id, a));

        let cache = WorldTransforms2D::from_world(&world);

        assert_eq!(cache.len(), 2, "both are composed, neither hangs");
    }
}
