//! Any registered component as a [`SerialTree`], and back: what a tool
//! that edits one field of a component it cannot name — the editor's
//! inspector across a selection — reads and writes.

use voltra_ecs::{Entity, World};

use super::ComponentRegistry;
use crate::format::error::SceneError;
use crate::format::serial_tree::SerialTree;

impl ComponentRegistry {
    /// The entity's value for `name` as a tree, or `None` when the name is
    /// unregistered **or** the entity has no such component — the same
    /// answer [`save_one`](Self::save_one) gives.
    pub fn save_tree(
        &self,
        world: &World,
        entity: Entity,
        name: &str,
    ) -> Option<Result<SerialTree, SceneError>> {
        let entry = self.entries.iter().find(|e| e.name == name)?;
        (entry.tree)(world, entity)
    }

    /// Inserts `tree` as `name`, replacing any there; `None` when the name
    /// is unregistered. A tree that does not read as the type is an error,
    /// and the entity is left as it was.
    pub fn load_tree(
        &self,
        world: &mut World,
        entity: Entity,
        name: &str,
        tree: &SerialTree,
    ) -> Option<Result<(), SceneError>> {
        let raw = match tree.to_raw() {
            Ok(raw) => raw,
            Err(error) => {
                // Unregistered stays `None` whatever the tree holds.
                self.entries.iter().find(|e| e.name == name)?;
                return Some(Err(error));
            }
        };
        self.load_one(world, entity, name, &raw)
    }
}
