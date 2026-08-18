//! Which entity a thing is attached to, and where that puts it.
//!
//! A [`Transform`] means "relative to my parent", and a root's parent is the
//! world. That single change is what lets a turret sit on a tank, an eye on a
//! face, and a whole scene move by moving one entity — and it is why every
//! consumer of a transform (the sprite batch, picking, the gizmos) reads a
//! *composed* matrix rather than the component.
//!
//! Split by how the parts fail. [`parent`] is the component and its file form,
//! [`link`] is the rules about which links may exist, [`resolve`] is turning a
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
//! [`Transform`]: crate::Transform

pub mod link;
pub mod parent;
pub mod resolve;
pub mod world_transform;

pub use link::{
    ancestors, can_parent, children_of, clear_parent, depth, descendants, despawn_recursive,
    is_ancestor, parent_of, roots, set_parent, HierarchyError, MAX_DEPTH,
};
pub use parent::Parent;
pub use resolve::resolve_parents;
pub use world_transform::{
    is_root, parent_matrix, set_world_transform, world_matrix, world_transform, WorldTransforms,
};
