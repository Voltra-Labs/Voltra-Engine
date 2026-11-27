//! Writing what the panels did to the primary into the rest of the
//! selection.

use voltra_ecs::{Entity, World};
use voltra_scene::format::serial_tree::differences;
use voltra_scene::format::SerialTree;
use voltra_scene::ComponentRegistry;

use super::{tree, warn_once, MultiEdit};

impl MultiEdit {
    /// Writes what the frame changed on the primary into every other
    /// selected entity, field by field, and answers the entities written —
    /// whose assets the caller resolves again, since a value loaded from a
    /// file carries no handles.
    ///
    /// - A **changed** component writes each changed field into each other
    ///   entity's value, leaving every other field as that entity had it;
    ///   an entity whose value has no such place (a circle, for a box's
    ///   half extents) is left alone for that field.
    /// - A component **added** to the primary is added to the rest as the
    ///   primary has it; one **removed** is removed from them.
    pub(crate) fn finish(self, world: &mut World, registry: &ComponentRegistry) -> Vec<Entity> {
        let mut written = Vec::new();
        if self.others.is_empty() {
            return written;
        }
        for (name, before) in &self.before {
            let after = tree(world, registry, self.primary, name);
            let changed = match (before, &after) {
                (Some(before), Some(after)) if before != after => {
                    self.write_fields(world, registry, name, before, after)
                }
                (None, Some(after)) => self.write_whole(world, registry, name, after),
                (Some(_), None) if registry.has(world, self.primary, name) == Some(false) => self
                    .others
                    .iter()
                    .copied()
                    .filter(|&other| {
                        registry.has(world, other, name) == Some(true)
                            && registry.remove_one(world, other, name)
                    })
                    .collect(),
                _ => Vec::new(),
            };
            for entity in changed {
                if !written.contains(&entity) {
                    written.push(entity);
                }
            }
        }
        written
    }

    /// Each place `before` and `after` differ, copied from `after` into
    /// every other entity's `name`; the entities whose value moved.
    fn write_fields(
        &self,
        world: &mut World,
        registry: &ComponentRegistry,
        name: &str,
        before: &SerialTree,
        after: &SerialTree,
    ) -> Vec<Entity> {
        let places = differences(before, after);
        let mut written = Vec::new();
        for &other in &self.others {
            let Some(mut theirs) = tree(world, registry, other, name) else {
                continue;
            };
            let mut moved = false;
            for place in &places {
                let Some(value) = after.at(place) else {
                    continue;
                };
                if theirs.at(place).is_some_and(|now| now != value) {
                    moved |= theirs.set(place, value.clone());
                }
            }
            if moved && load(world, registry, other, name, &theirs) {
                written.push(other);
            }
        }
        written
    }

    /// `value` as every other entity's `name`.
    fn write_whole(
        &self,
        world: &mut World,
        registry: &ComponentRegistry,
        name: &str,
        value: &SerialTree,
    ) -> Vec<Entity> {
        self.others
            .iter()
            .copied()
            .filter(|&other| load(world, registry, other, name, value))
            .collect()
    }
}

/// Puts `tree` on `entity` as `name`; whether it did.
fn load(
    world: &mut World,
    registry: &ComponentRegistry,
    entity: Entity,
    name: &str,
    tree: &SerialTree,
) -> bool {
    match registry.load_tree(world, entity, name, tree) {
        Some(Ok(())) => true,
        Some(Err(error)) => {
            warn_once(name, &format!("cannot be written: {error}"));
            false
        }
        None => false,
    }
}
