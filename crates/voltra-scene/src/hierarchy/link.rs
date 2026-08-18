//! Making, breaking and walking the parent links.
//!
//! Every write to a [`Parent`] goes through [`set_parent`] or [`clear_parent`].
//! That is what keeps the two halves of the link — the stored [`SceneId`] and
//! the resolved [`Entity`] — from ever disagreeing, and it is where the rules a
//! hierarchy has to enforce live: no entity is its own ancestor, and a parent
//! has to have an identity a file can name.

use std::collections::HashSet;
use std::fmt;

use voltra_ecs::{Entity, World};

use super::parent::Parent;
use crate::scene_id::SceneId;
use crate::{Collider, RigidBody};

/// How deep a chain [`ancestors`] will walk before it gives up.
///
/// A guard against a cycle, not a design limit: cycles cannot be made through
/// [`set_parent`], but a hand-edited scene file can name one, and every walk up
/// the tree has to terminate on one anyway. Deeper than any hierarchy a person
/// builds, shallow enough that a cycle is caught in the same frame it appears.
pub const MAX_DEPTH: usize = 256;

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
    /// Physics reads and writes [`Transform`] as a world-space value, and knows
    /// nothing about the hierarchy. Parenting a body would make gravity pull
    /// along the parent's rotated axes and a contact resolve in the wrong
    /// frame. The rule goes when the solver gathers world transforms, not
    /// before.
    ///
    /// [`Transform`]: crate::Transform
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
/// a `Transform` is local to its parent, so the child keeps its local values and
/// moves to where those values put it inside the new frame. Unity offers both
/// ("world position stays" is its default); the editor chooses per action, and
/// the primitive stays the one that does not silently rewrite data. See
/// [`set_world_transform`] for the other half.
///
/// [`set_world_transform`]: super::world_transform::set_world_transform
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

/// The live parent of `entity`, if it has one that resolves.
pub fn parent_of(world: &World, entity: Entity) -> Option<Entity> {
    let parent = world.get::<Parent>(entity)?.entity?;
    world.is_alive(parent).then_some(parent)
}

/// Every entity whose parent is `parent`, in a stable order.
///
/// A pass over the parent links rather than a stored list — see [`Parent`] for
/// why there is no list. Sorted by index for the same reason the hierarchy panel
/// sorts: sparse-set storage order changes when an unrelated entity is
/// despawned, and a list that reshuffles itself is unusable.
pub fn children_of(world: &World, parent: Entity) -> Vec<Entity> {
    let mut children: Vec<Entity> = world
        .query::<Parent>()
        .filter(|(_, link)| link.entity == Some(parent))
        .map(|(entity, _)| entity)
        .collect();
    children.sort_unstable_by_key(Entity::index);
    children
}

/// Every entity with no live parent, in a stable order.
///
/// The tops of the trees, and what a hierarchy panel draws first. An entity
/// whose link does not resolve counts as a root: it is drawn somewhere, and
/// nowhere else is honest.
pub fn roots(world: &World) -> Vec<Entity> {
    let mut roots: Vec<Entity> = world
        .query::<SceneId>()
        .map(|(entity, _)| entity)
        .filter(|entity| parent_of(world, *entity).is_none())
        .collect();
    roots.sort_unstable_by_key(Entity::index);
    roots
}

/// `entity`'s parent, then its parent, and so on to the root.
///
/// Stops at [`MAX_DEPTH`] or at the first entity it has already seen, so a cycle
/// in a hand-edited file cannot hang the editor.
pub fn ancestors(world: &World, entity: Entity) -> Vec<Entity> {
    let mut seen = HashSet::new();
    let mut chain = Vec::new();
    let mut cursor = entity;

    while let Some(parent) = parent_of(world, cursor) {
        if chain.len() >= MAX_DEPTH || !seen.insert(parent) {
            log::warn!("parent chain above {entity:?} is a cycle or deeper than {MAX_DEPTH}");
            break;
        }
        chain.push(parent);
        cursor = parent;
    }

    chain
}

/// Whether `ancestor` is `entity`'s parent, or its parent's parent, and so on.
///
/// An entity is not its own ancestor.
pub fn is_ancestor(world: &World, ancestor: Entity, entity: Entity) -> bool {
    ancestors(world, entity).contains(&ancestor)
}

/// How many parents sit above `entity`. A root is zero.
pub fn depth(world: &World, entity: Entity) -> usize {
    ancestors(world, entity).len()
}

/// `entity`'s children, their children, and so on, parents before children.
///
/// Cycle-safe: an entity already reported is never walked a second time, so a
/// hand-edited file that loops cannot make this list grow forever.
pub fn descendants(world: &World, entity: Entity) -> Vec<Entity> {
    let mut found = Vec::new();
    let mut seen = HashSet::from([entity]);
    let mut frontier = vec![entity];

    while let Some(current) = frontier.pop() {
        for child in children_of(world, current) {
            if seen.insert(child) {
                found.push(child);
                frontier.push(child);
            }
        }
    }

    found
}

/// Despawns `entity` and everything under it, returning what went.
///
/// Deleting a parent deletes its children, as it does in Unity, Unreal and
/// Godot. The alternative — leaving them behind — silently reinterprets every
/// orphan's local transform as a world one, so a deleted parent scatters its
/// children across the scene instead of taking them with it.
///
/// Children first, so nothing is ever left pointing at a despawned parent even
/// if the caller stops reading half way.
pub fn despawn_recursive(world: &mut World, entity: Entity) -> Vec<Entity> {
    let mut going = descendants(world, entity);
    going.reverse();
    going.push(entity);

    going.retain(|e| world.despawn(*e));
    going
}

/// Whether the simulation reads this entity's transform as a world value.
fn takes_part_in_physics(world: &World, entity: Entity) -> bool {
    world.get::<RigidBody>(entity).is_some() || world.get::<Collider>(entity).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Transform;

    /// An entity that can be a parent: it carries an identity.
    fn spawn(world: &mut World) -> Entity {
        let entity = world.spawn();
        world.insert(entity, SceneId::new());
        world.insert(entity, Transform::default());
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
        world.insert(transient, Transform::default());

        assert_eq!(
            set_parent(&mut world, child, transient),
            Err(HierarchyError::ParentHasNoIdentity)
        );
    }

    #[test]
    fn a_physics_body_cannot_be_parented() {
        // Physics reads `Transform` as world space. Until the solver gathers
        // world transforms, a parented body simulates in the wrong frame.
        let mut world = World::new();
        let parent = spawn(&mut world);
        let body = spawn(&mut world);
        world.insert(body, RigidBody::new_dynamic(1.0));

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
        world.insert(body, Collider::Circle { radius: 1.0 });
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

    #[test]
    fn a_despawned_parent_leaves_a_root() {
        // Nothing despawns recursively yet, so this is what a deleted parent
        // does: the link stays, stops resolving, and the child draws as a root
        // rather than vanishing with it.
        let mut world = World::new();
        let parent = spawn(&mut world);
        let child = spawn(&mut world);
        set_parent(&mut world, child, parent).expect("a plain reparent");

        world.despawn(parent);

        assert_eq!(parent_of(&world, child), None);
        assert!(roots(&world).contains(&child));
        assert!(
            world.get::<Parent>(child).is_some(),
            "the link is kept, so a save still writes it"
        );
    }

    #[test]
    fn ancestors_run_from_the_parent_upwards() {
        let mut world = World::new();
        let grandparent = spawn(&mut world);
        let parent = spawn(&mut world);
        let child = spawn(&mut world);
        set_parent(&mut world, parent, grandparent).expect("a plain reparent");
        set_parent(&mut world, child, parent).expect("a plain reparent");

        assert_eq!(ancestors(&world, child), vec![parent, grandparent]);
        assert_eq!(depth(&world, child), 2);
        assert_eq!(depth(&world, grandparent), 0);
        assert!(is_ancestor(&world, grandparent, child));
        assert!(!is_ancestor(&world, child, grandparent));
        assert!(!is_ancestor(&world, child, child), "not its own ancestor");
    }

    #[test]
    fn a_cycle_in_the_data_terminates_every_walk() {
        // `set_parent` cannot make this. A hand-edited scene file can, and the
        // editor has to survive opening one rather than hang on the first frame.
        let mut world = World::new();
        let a = spawn(&mut world);
        let b = spawn(&mut world);
        let a_id = *world.get::<SceneId>(a).expect("spawned with one");
        let b_id = *world.get::<SceneId>(b).expect("spawned with one");
        world.insert(a, Parent::resolved(b_id, b));
        world.insert(b, Parent::resolved(a_id, a));

        // Terminates, and does not report an entity as its own ancestor twice.
        let chain = ancestors(&world, a);
        assert!(chain.len() <= MAX_DEPTH, "walked {} deep", chain.len());
    }

    #[test]
    fn descendants_reach_every_level() {
        let mut world = World::new();
        let root = spawn(&mut world);
        let child = spawn(&mut world);
        let grandchild = spawn(&mut world);
        let unrelated = spawn(&mut world);
        set_parent(&mut world, child, root).expect("a plain reparent");
        set_parent(&mut world, grandchild, child).expect("a plain reparent");

        let found = descendants(&world, root);

        assert_eq!(found.len(), 2, "got {found:?}");
        assert!(found.contains(&child) && found.contains(&grandchild));
        assert!(!found.contains(&unrelated));
        assert!(descendants(&world, grandchild).is_empty());
    }

    #[test]
    fn deleting_a_parent_takes_its_children() {
        // The alternative reinterprets every orphan's local transform as a
        // world one, scattering the children across the scene.
        let mut world = World::new();
        let root = spawn(&mut world);
        let child = spawn(&mut world);
        let grandchild = spawn(&mut world);
        set_parent(&mut world, child, root).expect("a plain reparent");
        set_parent(&mut world, grandchild, child).expect("a plain reparent");

        let gone = despawn_recursive(&mut world, root);

        assert_eq!(gone.len(), 3, "got {gone:?}");
        assert!(!world.is_alive(root));
        assert!(!world.is_alive(child));
        assert!(!world.is_alive(grandchild));
    }

    #[test]
    fn deleting_a_leaf_leaves_its_parent_alone() {
        let mut world = World::new();
        let root = spawn(&mut world);
        let child = spawn(&mut world);
        set_parent(&mut world, child, root).expect("a plain reparent");

        assert_eq!(despawn_recursive(&mut world, child), vec![child]);
        assert!(world.is_alive(root));
        assert!(children_of(&world, root).is_empty());
    }

    #[test]
    fn a_recursive_delete_terminates_on_a_cycle() {
        let mut world = World::new();
        let a = spawn(&mut world);
        let b = spawn(&mut world);
        let a_id = *world.get::<SceneId>(a).expect("spawned with one");
        let b_id = *world.get::<SceneId>(b).expect("spawned with one");
        world.insert(a, Parent::resolved(b_id, b));
        world.insert(b, Parent::resolved(a_id, a));

        let gone = despawn_recursive(&mut world, a);

        assert_eq!(gone.len(), 2, "got {gone:?}");
        assert_eq!(world.entity_count(), 0);
    }

    #[test]
    fn children_come_out_in_index_order() {
        let mut world = World::new();
        let parent = spawn(&mut world);
        let first = spawn(&mut world);
        let second = spawn(&mut world);
        set_parent(&mut world, second, parent).expect("a plain reparent");
        set_parent(&mut world, first, parent).expect("a plain reparent");

        assert_eq!(children_of(&world, parent), vec![first, second]);
    }

    #[test]
    fn a_transient_entity_is_not_a_root() {
        // Roots are what the scene contains, and the scene is what carries a
        // `SceneId` — the same predicate save, load and Clear use.
        let mut world = World::new();
        let scene_entity = spawn(&mut world);
        let transient = world.spawn();
        world.insert(transient, Transform::default());

        assert_eq!(roots(&world), vec![scene_entity]);
    }
}
