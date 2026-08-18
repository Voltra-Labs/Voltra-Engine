//! Where a parented entity actually is.
//!
//! A [`Transform`] is local to its parent. Everything that draws, picks or
//! points at an entity needs the composed matrix instead, and there are two
//! shapes of that need: one entity at a time (a gizmo, an inspector) and every
//! entity at once (a frame of sprites). [`world_matrix`] serves the first by
//! walking up the chain; [`WorldTransforms`] serves the second by composing the
//! whole world once, so a deep tree costs one multiply per entity rather than
//! one per entity per level.

use std::collections::HashMap;

use voltra_ecs::{Entity, World};
use voltra_render::glam::Mat3;

use super::link::{ancestors, parent_of};
use super::parent::Parent;
use crate::transform::Transform;

/// Below this a parent's frame has collapsed and cannot be inverted.
///
/// Same threshold and same reasoning as picking: `Mat3::inverse` on a singular
/// matrix does not panic, it returns infinities and NaN, and a NaN transform
/// removes an entity from the screen with no error anywhere.
const MIN_DETERMINANT: f32 = 1e-12;

/// The frame `entity` sits in: every ancestor's matrix, composed.
///
/// [`Mat3::IDENTITY`] for a root, which is what makes a root's local transform
/// its world transform with no special case anywhere else.
pub fn parent_matrix(world: &World, entity: Entity) -> Mat3 {
    ancestors(world, entity)
        .iter()
        .rev()
        .fold(Mat3::IDENTITY, |above, ancestor| {
            above * local_matrix(world, *ancestor)
        })
}

/// Where `entity` is in world space, as a matrix.
pub fn world_matrix(world: &World, entity: Entity) -> Mat3 {
    parent_matrix(world, entity) * local_matrix(world, entity)
}

/// Where `entity` is in world space, as a transform.
///
/// Decomposed, so it is what a gizmo can drag and an overlay can draw. See
/// [`Transform::from_matrix`] for what a chain of parents can express that this
/// cannot.
pub fn world_transform(world: &World, entity: Entity) -> Transform {
    Transform::from_matrix(world_matrix(world, entity))
}

/// Puts `entity` at `target` in **world** space, writing its local transform.
///
/// The inverse of [`world_transform`], and the way a drag that computed a
/// world-space answer lands on a child. Returns whether it was written: an
/// entity with no `Transform` has nowhere to put one, and a parent whose own
/// frame has collapsed to a point (a zero scale) gives no way back from world
/// space at all. Both leave the entity exactly as it was rather than writing a
/// NaN that would make it disappear.
pub fn set_world_transform(world: &mut World, entity: Entity, target: &Transform) -> bool {
    let parent = parent_matrix(world, entity);
    if parent.determinant().abs() < MIN_DETERMINANT {
        log::warn!("{entity:?} sits under a collapsed parent; leaving its transform alone");
        return false;
    }

    let local = Transform::from_matrix(parent.inverse() * target.matrix());
    match world.get_mut::<Transform>(entity) {
        Some(transform) => {
            *transform = local;
            true
        }
        None => false,
    }
}

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
pub struct WorldTransforms {
    matrices: HashMap<Entity, Mat3>,
}

impl WorldTransforms {
    /// Composes every entity that has a [`Transform`].
    pub fn from_world(world: &World) -> Self {
        let locals: HashMap<Entity, Mat3> = world
            .query::<Transform>()
            .map(|(entity, transform)| (entity, transform.matrix()))
            .collect();
        let parents: HashMap<Entity, Entity> = world
            .query::<Parent>()
            .filter_map(|(entity, link)| link.entity.map(|parent| (entity, parent)))
            .filter(|(_, parent)| world.is_alive(*parent))
            .collect();

        let mut matrices: HashMap<Entity, Mat3> = HashMap::with_capacity(locals.len());
        // Reused across entities: one allocation for the whole pass, not one
        // per chain walked.
        let mut chain: Vec<Entity> = Vec::new();

        for entity in locals.keys().copied() {
            if matrices.contains_key(&entity) {
                continue;
            }

            // Up to the first ancestor already composed, or to the root.
            chain.clear();
            let mut cursor = entity;
            let mut above = Mat3::IDENTITY;
            loop {
                chain.push(cursor);
                let Some(&parent) = parents.get(&cursor) else {
                    break;
                };
                if let Some(known) = matrices.get(&parent) {
                    above = *known;
                    break;
                }
                // A cycle, which only a hand-edited file can produce. The
                // entity at the top of the walk is treated as a root: the
                // alternative is looping forever on a file the editor opened.
                if chain.contains(&parent) {
                    log::warn!("{parent:?} is in a parent cycle; composing it as a root");
                    break;
                }
                cursor = parent;
            }

            // Top-down, so each entity multiplies the frame already composed.
            for entity in chain.iter().rev().copied() {
                above *= locals.get(&entity).copied().unwrap_or(Mat3::IDENTITY);
                matrices.insert(entity, above);
            }
        }

        Self { matrices }
    }

    /// `entity`'s world matrix, or [`Mat3::IDENTITY`] if it has none.
    pub fn matrix(&self, entity: Entity) -> Mat3 {
        self.get(entity).unwrap_or(Mat3::IDENTITY)
    }

    /// `entity`'s world matrix, or `None` if it was not composed.
    pub fn get(&self, entity: Entity) -> Option<Mat3> {
        self.matrices.get(&entity).copied()
    }

    pub fn len(&self) -> usize {
        self.matrices.len()
    }

    pub fn is_empty(&self) -> bool {
        self.matrices.is_empty()
    }
}

/// `entity`'s own transform as a matrix, or the identity if it has none.
///
/// An entity with children but no transform of its own is a grouping node, and
/// the identity is exactly what "adds no frame of its own" means.
fn local_matrix(world: &World, entity: Entity) -> Mat3 {
    world
        .get::<Transform>(entity)
        .map(Transform::matrix)
        .unwrap_or(Mat3::IDENTITY)
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

    fn spawn(world: &mut World, transform: Transform) -> Entity {
        let entity = world.spawn();
        world.insert(entity, SceneId::new());
        world.insert(entity, transform);
        entity
    }

    fn at(x: f32, y: f32) -> Transform {
        Transform::from_translation(Vec2::new(x, y))
    }

    fn position(world: &World, entity: Entity) -> Vec2 {
        world_matrix(world, entity).transform_point2(Vec2::ZERO)
    }

    #[test]
    fn a_root_is_where_its_own_transform_says() {
        let mut world = World::new();
        let entity = spawn(&mut world, at(3.0, 4.0));

        assert_eq!(parent_matrix(&world, entity), Mat3::IDENTITY);
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
            Transform::default()
                .with_rotation(FRAC_PI_2)
                .with_scale(Vec2::splat(2.0)),
        );
        let child = spawn(&mut world, at(1.0, 0.0));
        set_parent(&mut world, child, parent).expect("a plain reparent");

        // Offset one to the right, scaled by two and turned a quarter turn:
        // two units up.
        assert!((position(&world, child) - Vec2::new(0.0, 2.0)).length() < 1e-5);

        let composed = world_transform(&world, child);
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
    fn the_cache_agrees_with_the_walk() {
        let mut world = World::new();
        let parent = spawn(
            &mut world,
            Transform::from_translation(Vec2::new(4.0, -1.0))
                .with_rotation(0.7)
                .with_scale(Vec2::new(2.0, 2.0)),
        );
        let child = spawn(&mut world, at(1.0, 3.0));
        let grandchild = spawn(&mut world, at(0.0, 1.0));
        set_parent(&mut world, child, parent).expect("a plain reparent");
        set_parent(&mut world, grandchild, child).expect("a plain reparent");

        let cache = WorldTransforms::from_world(&world);

        for entity in [parent, child, grandchild] {
            let walked = world_matrix(&world, entity);
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

        let cache = WorldTransforms::from_world(&world);

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

        let cache = WorldTransforms::from_world(&world);

        assert_eq!(cache.len(), 2, "both are composed, neither hangs");
    }

    #[test]
    fn setting_a_world_transform_writes_the_local_one() {
        let mut world = World::new();
        let parent = spawn(&mut world, at(10.0, 0.0));
        let child = spawn(&mut world, at(0.0, 0.0));
        set_parent(&mut world, child, parent).expect("a plain reparent");

        assert!(set_world_transform(&mut world, child, &at(12.0, 3.0)));

        let local = world.get::<Transform>(child).expect("still has one");
        assert!((local.translation - Vec2::new(2.0, 3.0)).length() < 1e-5);
        assert!((position(&world, child) - Vec2::new(12.0, 3.0)).length() < 1e-5);
    }

    #[test]
    fn setting_a_world_transform_on_a_root_writes_it_unchanged() {
        let mut world = World::new();
        let entity = spawn(&mut world, at(0.0, 0.0));

        assert!(set_world_transform(&mut world, entity, &at(5.0, 5.0)));

        let local = world.get::<Transform>(entity).expect("still has one");
        assert_eq!(local.translation, Vec2::new(5.0, 5.0));
    }

    #[test]
    fn a_world_transform_round_trips_through_a_turned_parent() {
        let mut world = World::new();
        let parent = spawn(
            &mut world,
            Transform::from_translation(Vec2::new(3.0, -2.0))
                .with_rotation(0.9)
                .with_scale(Vec2::splat(1.5)),
        );
        let child = spawn(&mut world, at(0.0, 0.0));
        set_parent(&mut world, child, parent).expect("a plain reparent");

        let target = Transform::from_translation(Vec2::new(-4.0, 6.0))
            .with_rotation(0.2)
            .with_scale(Vec2::splat(3.0));
        assert!(set_world_transform(&mut world, child, &target));

        let back = world_transform(&world, child);
        assert!((back.translation - target.translation).length() < 1e-4);
        assert!((back.rotation - target.rotation).abs() < 1e-4);
        assert!((back.scale - target.scale).length() < 1e-4);
    }

    #[test]
    fn a_collapsed_parent_leaves_the_child_alone() {
        // A zero-scaled parent maps every point to its origin, so no world
        // position has a local answer. Writing one anyway means NaN.
        let mut world = World::new();
        let parent = spawn(&mut world, Transform::default().with_scale(Vec2::ZERO));
        let child = spawn(&mut world, at(1.0, 1.0));
        set_parent(&mut world, child, parent).expect("a plain reparent");

        assert!(!set_world_transform(&mut world, child, &at(5.0, 5.0)));

        let local = world.get::<Transform>(child).expect("still has one");
        assert_eq!(local.translation, Vec2::new(1.0, 1.0));
    }

    #[test]
    fn an_entity_without_a_transform_cannot_be_placed() {
        let mut world = World::new();
        let bare = world.spawn();
        assert!(!set_world_transform(&mut world, bare, &at(1.0, 1.0)));
    }
}
