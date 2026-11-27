//! Finding the entity a track animates, by the names from the player down.

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};

use voltra_ecs::{Entity, World};

use crate::hierarchy::{children_of, Parent};
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

/// Every entity's children, in the hierarchy's order, from one pass over
/// the links — what resolving many paths in a frame reads, rather than
/// scanning the world for each name ([`children_of`] does).
#[derive(Debug, Default)]
pub(super) struct Children(HashMap<Entity, Vec<Entity>>);

impl Children {
    /// The children of everything in `world` as it is now.
    pub fn of(world: &World) -> Self {
        let mut map: HashMap<Entity, Vec<Entity>> = HashMap::new();
        for (child, link) in world.query::<Parent>() {
            if let Some(parent) = link.entity {
                map.entry(parent).or_default().push(child);
            }
        }
        for children in map.values_mut() {
            children.sort_unstable_by_key(Entity::index);
        }
        Self(map)
    }

    /// [`resolve`], reading this index for each step down.
    pub fn resolve(&self, world: &World, root: Entity, path: &str) -> Option<Entity> {
        let mut at = root;
        for segment in path.split('/').filter(|s| !s.is_empty()) {
            at = *self.0.get(&at)?.iter().find(|&&child| {
                world
                    .get::<Name>(child)
                    .is_some_and(|n| n.as_str() == segment)
            })?;
        }
        Some(at)
    }
}

/// [`resolve`] for one pose written, the last path asked remembered: a
/// pose is sorted by target, so a target's position, rotation and colour
/// walk the hierarchy once between them, with no map to fill.
pub(super) struct Targets<'p, 'c> {
    children: &'c Children,
    last: Option<(&'p str, Option<Entity>)>,
}

impl<'p, 'c> Targets<'p, 'c> {
    /// Resolving through `children`.
    pub fn new(children: &'c Children) -> Self {
        Self {
            children,
            last: None,
        }
    }

    pub fn get(&mut self, world: &World, root: Entity, path: &'p str) -> Option<Entity> {
        match self.last {
            Some((last, found)) if last == path => found,
            _ => {
                let found = self.children.resolve(world, root, path);
                self.last = Some((path, found));
                found
            }
        }
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
        let index = Children::of(&world);
        for (path, entity) in &listed {
            assert_eq!(index.resolve(&world, root, path), Some(*entity), "{path:?}");
        }
        assert_eq!(index.resolve(&world, root, "/arm//hand/"), Some(hand));
        assert_eq!(index.resolve(&world, root, "leg"), None);
        assert_eq!(resolve(&world, root, "leg"), None);
        assert_eq!(canonical("/arm//hand/"), "arm/hand");
        assert!(matches!(canonical("arm/hand"), Cow::Borrowed(_)));
        assert_eq!(canonical("/"), "");
    }
}
