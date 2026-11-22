//! Finding the entity a track animates, by the names from the player down.

use std::collections::HashMap;

use voltra_ecs::{Entity, World};

use crate::hierarchy::children_of;
use crate::name::Name;

/// The entity `path` names under `root`: `""` is `root` itself, `"arm"` its
/// child named `arm`, `"arm/hand"` that child's child named `hand`.
///
/// By name rather than by id, the way Unity's and Godot's clips bind: a
/// clip is a file shared by every entity that plays it, and only names are
/// the same in each copy of a prefab. Two children with one name resolve to
/// the first in the hierarchy's order. Empty segments are skipped, so a
/// stray `/` does not lose the track.
pub fn resolve(world: &World, root: Entity, path: &str) -> Option<Entity> {
    let mut at = root;
    for segment in path.split('/').filter(|s| !s.is_empty()) {
        at = children_of(world, at).into_iter().find(|&child| {
            world
                .get::<Name>(child)
                .is_some_and(|n| n.as_str() == segment)
        })?;
    }
    Some(at)
}

/// [`resolve`], once per distinct path for one sample of one clip: a clip's
/// colour and position tracks on the same child walk the hierarchy once.
#[derive(Default)]
pub(super) struct Targets<'p> {
    found: HashMap<&'p str, Option<Entity>>,
}

impl<'p> Targets<'p> {
    pub fn get(&mut self, world: &World, root: Entity, path: &'p str) -> Option<Entity> {
        *self
            .found
            .entry(path)
            .or_insert_with(|| resolve(world, root, path))
    }
}
