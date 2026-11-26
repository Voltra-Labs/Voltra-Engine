//! Finding the entity a track animates, by the names from the player down.

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};

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

/// `path` as [`resolve`] reads it: empty segments dropped, so `"/arm//hand/"`
/// is `"arm/hand"`. Borrowed when it already is — the spelling every
/// editor-made track has.
pub fn canonical(path: &str) -> Cow<'_, str> {
    let clean = !path.starts_with('/') && !path.ends_with('/') && !path.contains("//");
    if clean {
        return Cow::Borrowed(path);
    }
    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    Cow::Owned(segments.join("/"))
}

/// Every entity a track under `root` can name, with the path that names
/// it: `root` as `""`, then each named descendant, depth first. An unnamed
/// entity — and what is under it — cannot be named, and is left out; so is
/// a second child of a name already taken, which [`resolve`] would never
/// reach.
pub fn paths(world: &World, root: Entity) -> Vec<(String, Entity)> {
    let mut found = vec![(String::new(), root)];
    walk(world, root, "", &mut found);
    found
}

fn walk(world: &World, at: Entity, prefix: &str, found: &mut Vec<(String, Entity)>) {
    let mut taken = HashSet::new();
    for child in children_of(world, at) {
        let Some(name) = world.get::<Name>(child).map(Name::as_str) else {
            continue;
        };
        // A name with a separator in it cannot be spelled as one segment.
        if name.is_empty() || name.contains('/') || !taken.insert(name.to_owned()) {
            continue;
        }
        let path = if prefix.is_empty() {
            name.to_owned()
        } else {
            format!("{prefix}/{name}")
        };
        found.push((path.clone(), child));
        walk(world, child, &path, found);
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hierarchy::{resolve_parents, set_parent};
    use crate::scene_id::SceneId;

    fn named(world: &mut World, name: Option<&str>, parent: Option<Entity>) -> Entity {
        let entity = world.spawn();
        world.insert(entity, SceneId::new());
        if let Some(name) = name {
            world.insert(entity, Name::new(name));
        }
        if let Some(parent) = parent {
            set_parent(world, entity, parent).expect("a plain reparent");
        }
        entity
    }

    #[test]
    fn every_path_listed_resolves_to_its_entity() {
        let mut world = World::new();
        let root = named(&mut world, Some("rig"), None);
        let arm = named(&mut world, Some("arm"), Some(root));
        let hand = named(&mut world, Some("hand"), Some(arm));
        named(&mut world, Some("arm"), Some(root));
        let anonymous = named(&mut world, None, Some(root));
        named(&mut world, Some("lost"), Some(anonymous));
        resolve_parents(&mut world);

        let listed = paths(&world, root);
        let names: Vec<&str> = listed.iter().map(|(p, _)| p.as_str()).collect();
        assert_eq!(names, ["", "arm", "arm/hand"]);
        for (path, entity) in &listed {
            assert_eq!(resolve(&world, root, path), Some(*entity), "{path:?}");
        }
        assert_eq!(resolve(&world, root, "/arm//hand/"), Some(hand));
        assert_eq!(resolve(&world, root, "leg"), None);
        assert_eq!(canonical("/arm//hand/"), "arm/hand");
        assert!(matches!(canonical("arm/hand"), Cow::Borrowed(_)));
        assert_eq!(canonical("/"), "");
    }
}
