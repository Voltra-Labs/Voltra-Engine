//! Making and breaking the parent links, and the rules a link must keep.
//!
//! Every write to a [`Parent`] goes through [`set_parent`] or [`clear_parent`].
//! That is what keeps the two halves of the link — the stored [`SceneId`] and
//! the resolved [`Entity`] — from ever disagreeing, and it is where the rules a
//! hierarchy has to enforce live: no entity is its own ancestor, and a parent
//! has to have an identity a file can name. Walking the links is [`super::walk`].

use std::fmt;

use voltra_ecs::{Entity, World};

use super::parent::Parent;
use super::walk::is_ancestor;
use crate::scene_id::SceneId;
use crate::{Collider2D, RigidBody2D};

/// The walks, on the path 0.4 had them.
pub use super::walk::{
    ancestors, children_of, depth, descendants, despawn_recursive, parent_of, roots, MAX_DEPTH,
};

/// Why a reparent was refused.
///
/// Refused rather than clamped: every one of these means the caller asked for
/// something with no correct interpretation, and picking one silently is how an
/// editor loses a user's work while reporting success.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HierarchyError {
    /// The child and the parent are the same entity.
    Itself,
    /// The parent is already a descendant of the child.
    Cycle,
    /// The parent carries no [`SceneId`], so the link could not be saved.
    ///
    /// A link is stored by identity. A parent without one would make a child
    /// that loads as a root — the link would be lost at the first save, which
    /// is worse than never accepting it.
    ParentHasNoIdentity,
    /// The child, or the parent, takes part in the simulation.
    ///
    /// Physics reads and writes [`Transform2D`] as a world-space value, and knows
    /// nothing about the hierarchy. Parenting a body would make gravity pull
    /// along the parent's rotated axes and a contact resolve in the wrong
    /// frame. The rule goes when the solver gathers world transforms, not
    /// before.
    ///
    /// [`Transform2D`]: crate::Transform2D
    PhysicsBody,
}

impl fmt::Display for HierarchyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Itself => write!(f, "an entity cannot be its own parent"),
            Self::Cycle => write!(f, "that parent is already a child of this entity"),
            Self::ParentHasNoIdentity => {
                write!(f, "a parent must carry a scene identity to be saved")
            }
            Self::PhysicsBody => write!(f, "an entity with physics cannot be parented"),
        }
    }
}

impl std::error::Error for HierarchyError {}

/// Makes `child` a child of `parent`.
///
/// Replaces any link the child already had. The transform is **not** adjusted:
/// a `Transform2D` is local to its parent, so the child keeps its local values and
/// moves to where those values put it inside the new frame. Unity offers both
/// ("world position stays" is its default); the editor chooses per action, and
/// the primitive stays the one that does not silently rewrite data. See
/// [`set_world_transform_2d`] for the other half.
///
/// [`set_world_transform_2d`]: super::world_transform::set_world_transform_2d
pub fn set_parent(world: &mut World, child: Entity, parent: Entity) -> Result<(), HierarchyError> {
    can_parent(world, child, parent)?;
    let id = *world
        .get::<SceneId>(parent)
        .expect("can_parent rejects a parent without an identity");

    world.insert(child, Parent::resolved(id, parent));
    Ok(())
}

/// Whether [`set_parent`] would accept this pair, and why not if it would not.
///
/// Separate from the write so a UI can answer "may this be dropped here?" while
/// the pointer is still moving, without either duplicating the rules or
/// performing the reparent to find out. Every caller that asks and then writes
/// gets the same answer, because the write asks this too.
pub fn can_parent(world: &World, child: Entity, parent: Entity) -> Result<(), HierarchyError> {
    if child == parent {
        return Err(HierarchyError::Itself);
    }
    if takes_part_in_physics(world, child) || takes_part_in_physics(world, parent) {
        return Err(HierarchyError::PhysicsBody);
    }
    if world.get::<SceneId>(parent).is_none() {
        return Err(HierarchyError::ParentHasNoIdentity);
    }
    // The parent must not already hang off the child. Checked before the write,
    // because after it the walk itself would be the cycle.
    if is_ancestor(world, child, parent) {
        return Err(HierarchyError::Cycle);
    }

    Ok(())
}

/// Detaches `child` from its parent, reporting whether it had one.
///
/// Like [`set_parent`], this leaves the transform alone: the child keeps its
/// local values and they now mean world values.
pub fn clear_parent(world: &mut World, child: Entity) -> bool {
    world.remove::<Parent>(child).is_some()
}

/// Whether the simulation reads this entity's transform as a world value.
fn takes_part_in_physics(world: &World, entity: Entity) -> bool {
    world.get::<RigidBody2D>(entity).is_some() || world.get::<Collider2D>(entity).is_some()
}

#[cfg(test)]
mod tests {
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
}
