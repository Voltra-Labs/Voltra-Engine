//! Editing every selected entity at once through the inspector, which
//! draws only the primary.
//!
//! **The panels stay one-entity.** Each of the inspector's forty-odd
//! sections edits a `&mut T` with egui widgets, and a game's components are
//! edited as text; none of them knows a selection. So a frame with several
//! selected works on the primary as before, and this module turns what that
//! did into an edit of the rest: [`MultiEdit::begin`] takes the primary's
//! components as [`SerialTree`]s before the panels draw, and
//! [`MultiEdit::finish`] takes them again after, finds the fields that
//! changed ([`differences`]) and writes **only those fields** into every
//! other selected entity's value, through the registry. Unity's
//! `SerializedObject` over several targets, with its property paths; Godot's
//! `MultiNodeEdit`, which sets the one property on every node. Through the
//! registry rather than per panel, so it holds for every component built
//! in or registered by a game, and a new panel gets it for nothing.
//!
//! **Shared, mixed, partial.** A component every selected entity has is
//! *shared*, and editable; where its fields differ across the selection
//! they are *mixed* ([`MultiEdit::mixed`]), which the inspector shows. A
//! component only some have is *partial* ([`MultiEdit::partial`]): listed,
//! not editable — its section is not drawn. A component none has is
//! editable too: adding it to the primary adds it to all.
//!
//! **One undo step.** Every selected entity is already watched by the
//! history from the top of the frame, so what this writes folds into the
//! same entry as the primary's own change, held open by the same claim: a
//! drag over three sprites is one `Ctrl+Z`.

mod write;

use std::collections::{BTreeMap, HashSet};
use std::sync::{Mutex, OnceLock};

use voltra_ecs::{Entity, World};
use voltra_scene::format::serial_tree::{differences, Step, TreePath};
use voltra_scene::format::SerialTree;
use voltra_scene::{ComponentRegistry, PrefabInstance, PrefabPart};

/// Components never written across a selection: structure, which says
/// where an entity hangs and which prefab it came from, is the entity's
/// own; and the name, which the name field writes to all itself (an
/// entity without one has an empty one, not a partial one).
const NOT_SHARED: [&str; 4] = ["Parent", PrefabInstance::NAME, PrefabPart::NAME, "Name"];

/// One frame of the inspector over the selection.
#[derive(Debug)]
pub(crate) struct MultiEdit {
    primary: Entity,
    /// The rest of the selection, alive, in the order picked.
    others: Vec<Entity>,
    /// The primary's value at the top of the frame of every component it
    /// may write across — shared, or held by none — `None` where it has
    /// none.
    before: Vec<(&'static str, Option<SerialTree>)>,
    /// Where each shared component's value differs across the selection.
    mixed: BTreeMap<&'static str, Vec<TreePath>>,
    /// Each partial component and how many of the selection hold it.
    partial: Vec<(&'static str, usize)>,
    /// Places a widget committed this frame, written across even where the
    /// primary's value did not move — choosing the primary's own value for
    /// a mixed field sets it on all, as in Unity.
    touched: Vec<(&'static str, TreePath, Touch)>,
}

/// How a touched place is written to the rest of the selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Touch {
    /// As the primary has it — a number, a name, a unit-only enum.
    Value,
    /// Only where the entity's value there is another variant of the enum:
    /// a kind picked again leaves each one already of it with its own
    /// parameters (limits, sizes), and turns the rest into the primary's.
    Variant,
}

impl MultiEdit {
    /// Reads the selection before the panels draw. `primary` is what they
    /// edit; `selected` is the whole selection, the primary among it or
    /// not. With nothing else selected, every query answers as for one
    /// entity and [`finish`](Self::finish) writes nothing.
    pub(crate) fn begin(
        world: &World,
        registry: &ComponentRegistry,
        primary: Entity,
        selected: impl IntoIterator<Item = Entity>,
    ) -> Self {
        let others: Vec<Entity> = selected
            .into_iter()
            .filter(|&e| e != primary && world.is_alive(e))
            .collect();
        let mut edit = Self {
            primary,
            others,
            before: Vec::new(),
            mixed: BTreeMap::new(),
            partial: Vec::new(),
            touched: Vec::new(),
        };
        if edit.others.is_empty() {
            return edit;
        }
        for name in registry.names().filter(|name| !NOT_SHARED.contains(name)) {
            let holders = edit
                .all()
                .filter(|&e| registry.has(world, e, name) == Some(true))
                .count();
            if holders == 0 {
                edit.before.push((name, None));
            } else if holders < edit.len() {
                edit.partial.push((name, holders));
            } else if let Some(primary) = tree(world, registry, primary, name) {
                let mut mixed: Vec<TreePath> = Vec::new();
                for &other in &edit.others {
                    let Some(theirs) = tree(world, registry, other, name) else {
                        continue;
                    };
                    for path in differences(&primary, &theirs) {
                        if !mixed.contains(&path) {
                            mixed.push(path);
                        }
                    }
                }
                if !mixed.is_empty() {
                    edit.mixed.insert(name, mixed);
                }
                edit.before.push((name, Some(primary)));
            } else {
                // Could not be read: not written across either, said where
                // `tree` failed.
                edit.partial.push((name, holders));
            }
        }
        edit
    }

    /// How many entities the edit covers, the primary included.
    pub(crate) fn len(&self) -> usize {
        self.others.len() + 1
    }

    /// Whether more than one entity is being edited.
    #[cfg(test)]
    pub(crate) fn is_multi(&self) -> bool {
        !self.others.is_empty()
    }

    /// The primary, then the rest.
    pub(crate) fn all(&self) -> impl Iterator<Item = Entity> + '_ {
        std::iter::once(self.primary).chain(self.others.iter().copied())
    }

    /// Whether a section over the components `names` may be drawn: none of
    /// them is partial.
    pub(crate) fn editable(&self, names: &[&str]) -> bool {
        !names
            .iter()
            .any(|name| self.partial.iter().any(|(held, _)| held == name))
    }

    /// The places where `name`'s value differs across the selection; empty
    /// for a component that agrees, or is not shared.
    pub(crate) fn mixed(&self, name: &str) -> &[TreePath] {
        self.mixed.get(name).map_or(&[], Vec::as_slice)
    }

    /// Whether anything at or under `path` in `name` differs across the
    /// selection — or the value holding it does, which differs it too.
    pub(crate) fn is_mixed(&self, name: &str, path: &[Step]) -> bool {
        self.mixed(name)
            .iter()
            .any(|place| place.starts_with(path) || path.starts_with(place))
    }

    /// The components only some of the selection hold, and how many do.
    pub(crate) fn partial(&self) -> &[(&'static str, usize)] {
        &self.partial
    }

    /// Says a widget committed `path` of `name` this frame: written to the
    /// rest of the selection, as `how` says, whether or not the primary's
    /// value moved. A name that is not written across (partial, structure)
    /// is ignored.
    pub(crate) fn touch(&mut self, name: &str, path: TreePath, how: Touch) {
        let Some(&(name, _)) = self.before.iter().find(|(held, _)| *held == name) else {
            return;
        };
        if !self.touched.contains(&(name, path.clone(), how)) {
            self.touched.push((name, path, how));
        }
    }

    /// Leaves `name` out of what [`finish`](Self::finish) writes across:
    /// for an action the inspector has already done to every entity on its
    /// own terms — a bone's rest from its own transform, a collider fitted
    /// to its own sprite — which copying the primary's result would undo.
    pub(crate) fn done_for_each(&mut self, name: &str) {
        self.before.retain(|(held, _)| *held != name);
    }
}

/// `entity`'s `name` as a tree; `None` without one, or when it cannot be
/// written out — said, since the edit then leaves that component alone.
fn tree(
    world: &World,
    registry: &ComponentRegistry,
    entity: Entity,
    name: &str,
) -> Option<SerialTree> {
    match registry.save_tree(world, entity, name)? {
        Ok(tree) => Some(tree),
        Err(error) => {
            warn_once(format!(
                "{name} cannot be edited across the selection: it could not be written out: {error}"
            ));
            None
        }
    }
}

/// Logs `message` the first time it is said this session: the inspector
/// asks every frame, and the same failure would otherwise fill the log.
/// Keyed by the whole message, so a second component — or a second
/// reason — is still said.
fn warn_once(message: String) {
    static SEEN: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    let seen = SEEN.get_or_init(|| Mutex::new(HashSet::new()));
    let first = seen
        .lock()
        .map_or(true, |mut seen| seen.insert(message.clone()));
    if first {
        log::warn!("{message}");
    }
}

#[cfg(test)]
mod tests;
