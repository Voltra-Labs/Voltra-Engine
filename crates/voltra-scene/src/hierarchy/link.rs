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
mod tests;
