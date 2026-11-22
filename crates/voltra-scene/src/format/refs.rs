//! Components that name other entities by [`SceneId`].
//!
//! A copy — a prefab spawned, a selection pasted or duplicated — gives every
//! entity a fresh id, and a reference between two copied entities has to
//! follow them to the copies; a prefab instance compares and writes its
//! references in the file's record ids. Each of those places asks the
//! registry to rewrite whatever references a component holds, so a
//! component that names entities says so once, at registration
//! ([`ComponentRegistry::register_with_refs`]), and every copy and compare
//! honours it.
//!
//! [`ComponentRegistry::register_with_refs`]: super::ComponentRegistry::register_with_refs

use crate::scene_id::SceneId;

/// A component naming other entities by id.
pub trait SceneRefs {
    /// Rewrites every id it names through `map`, keeping one `map` does not
    /// know — a reference to an entity outside a copy keeps that entity.
    fn remap(&mut self, map: &dyn Fn(SceneId) -> Option<SceneId>);
}
