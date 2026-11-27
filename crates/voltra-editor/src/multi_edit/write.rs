//! Writing what the panels did to the primary into the rest of the
//! selection.

use voltra_ecs::{Entity, World};
use voltra_scene::format::serial_tree::{differences, Found, Step, TreePath};
use voltra_scene::format::SerialTree;
use voltra_scene::ComponentRegistry;

use super::{tree, warn_once, MultiEdit, Touch};

impl MultiEdit {
    /// Writes what the frame changed on the primary into every other
    /// selected entity, field by field, and answers the entities written —
    /// whose assets the caller resolves again, since a value loaded from a
    /// file carries no handles.
    ///
    /// - A **changed** component writes each changed field into each other
    ///   entity's value, leaving every other field as that entity had it;
    ///   a field the primary now leaves out (`skip_serializing_if`, at its
    ///   default) is left out of theirs too, and one it now writes is added.
    ///   An entity whose value has no such place (a circle, for a box's
    ///   half extents) is left alone for that field. A list that grew or
    ///   shrank is one place: the whole list is written.
    /// - A field a widget [`touch`](Self::touch)ed is written the same way
    ///   whether or not the primary's value moved.
    /// - A component **added** to the primary is added to the rest as the
    ///   primary has it; one **removed** is removed from them.
    ///
    /// Nothing is written when the primary is gone — despawned by whatever
    /// the frame did — since its absence says nothing about the rest.
    pub(crate) fn finish(self, world: &mut World, registry: &ComponentRegistry) -> Vec<Entity> {
        let mut written = Vec::new();
        if self.others.is_empty() || !world.is_alive(self.primary) {
            return written;
        }
        for (name, before) in &self.before {
            let after = tree(world, registry, self.primary, name);
            let touched: Vec<(TreePath, Touch)> = self
                .touched
                .iter()
                .filter(|(held, _, _)| held == name)
                .map(|(_, path, how)| (path.clone(), *how))
                .collect();
            let changed = match (before, &after) {
                (Some(before), Some(after)) if before != after || !touched.is_empty() => {
                    let mut places: Vec<(TreePath, Touch)> = differences(before, after)
                        .into_iter()
                        .map(|path| (path, Touch::Value))
                        .collect();
                    for (path, how) in touched {
                        if !places.iter().any(|(place, _)| *place == path) {
                            places.push((path, how));
                        }
                    }
                    self.write_places(world, registry, name, &places, after)
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

    /// Each of `places`, as `after` has it, written into every other
    /// entity's `name`; the entities whose value moved.
    fn write_places(
        &self,
        world: &mut World,
        registry: &ComponentRegistry,
        name: &str,
        places: &[(TreePath, Touch)],
        after: &SerialTree,
    ) -> Vec<Entity> {
        let mut written = Vec::new();
        for &other in &self.others {
            let Some(mut theirs) = tree(world, registry, other, name) else {
                continue;
            };
            let mut moved = false;
            for (place, how) in places {
                if *how == Touch::Variant && same_variant(after, &theirs, place) {
                    continue;
                }
                moved |= match after.find(place) {
                    Found::Here(value) => theirs.put(place, Some(value)),
                    Found::Absent => theirs.put(place, None),
                    Found::Nowhere => false,
                };
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

/// Whether `a` and `b` hold the same variant of an enum at `place`: what
/// a [`Touch::Variant`] leaves alone. Not an enum on either side, or
/// nothing there, is not the same.
fn same_variant(a: &SerialTree, b: &SerialTree, place: &[Step]) -> bool {
    match (a.find(place), b.find(place)) {
        (Found::Here(a), Found::Here(b)) => a.variant().is_some() && a.variant() == b.variant(),
        _ => false,
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
            warn_once(format!(
                "{name} was left as it was on a selected entity: the edited value does not read as one: {error}"
            ));
            false
        }
        None => false,
    }
}
