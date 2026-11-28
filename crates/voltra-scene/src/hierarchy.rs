//! Which entity a thing is attached to, and where that puts it.
//!
//! A [`Transform2D`] means "relative to my parent", and a root's parent is the
//! world. That single change is what lets a turret sit on a tank, an eye on a
//! face, and a whole scene move by moving one entity — and it is why every
//! consumer of a transform (the sprite batch, picking, the gizmos) reads a
//! *composed* matrix rather than the component.
//!
//! Split by how the parts fail. [`parent`] is the component and its file form,
//! [`link`] is the rules about which links may exist, [`walk`] is following
//! them up and down the tree, [`resolve`] is turning a
//! stored identity back into a live entity, and [`world_transform`] is the
//! arithmetic of composing one. Nothing here touches the GPU or egui, so all of
//! it is unit-tested.
//!
//! **The hierarchy is a transform relationship, not a container.** Despawning a
//! parent does not despawn its children, and a scene file is a flat list of
//! entities in identity order rather than a nested tree. That is Unity's shape
//! (a `Transform` with a parent pointer) rather than Godot's (a node tree that
//! *is* the scene): a flat file keeps the diff of a moved entity to one line,
//! and keeps the loader from having to reason about order.
//!
//! [`Transform2D`]: crate::Transform2D

pub mod link;
pub mod parent;
pub mod resolve;
pub mod walk;
pub mod world_transform;

pub use link::{can_parent, clear_parent, set_parent, HierarchyError};
pub use walk::{
    ancestors, children_of, depth, descendants, despawn_recursive, is_ancestor, parent_of, roots,
    MAX_DEPTH,
};

/// The 0.4 name, where 0.4 had it; see [`crate::renamed`].
#[allow(deprecated)]
pub use crate::renamed::WorldTransforms;
pub use parent::Parent;
// The 0.4 names, where 0.4 had them.
#[allow(deprecated)]
pub use crate::renamed::{parent_matrix, set_world_transform, world_matrix, world_transform};
pub use resolve::resolve_parents;
pub use world_transform::{
    is_root, parent_matrix_2d, set_world_transform_2d, world_matrix_2d, world_transform_2d,
    WorldTransforms2D,
};
