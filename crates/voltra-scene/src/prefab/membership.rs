//! Which instance an entity belongs to, when instances sit inside instances.
//!
//! An instance in a scene can hold two kinds of instance root under it, and
//! they look alike: a **nested** one — a prefab the instance's file holds,
//! whose entities are parts of the outer instance (their `PrefabPart`s name
//! records of the *outer* file) — and an **added** one, a prefab dropped
//! under a part afterwards, or made from a selection inside it, whose parts
//! name records of *its own* file. A sync of the outer instance must leave
//! an added one alone (it is the instance's own, as an added entity is) and
//! sync it against its own file instead; treating its parts as the outer
//! file's would despawn them all as lost.
//!
//! **An added instance is a linked entity whose part names no record of the
//! outer file but one of its own** — Unity's added nested prefab instance,
//! told apart the same way, by whose file its object belongs to. Where its
//! own file is not to hand (it will not read, or the store did not follow
//! it) it counts as added: keeping an entity beats despawning one that may
//! be the author's.

use std::collections::{HashMap, HashSet};

use voltra_assets::AssetPath;
use voltra_ecs::{Entity, World};

use super::{PrefabInstance, PrefabPart, Prefabs};
use crate::hierarchy::{self, Parent};
use crate::scene_id::SceneId;

/// Whether `candidate` is an instance added to the instance whose file holds
/// `records`: linked, with a part naming no record of that file, and
/// `has_record` saying its own file has that record.
pub(super) fn is_added(
    world: &World,
    candidate: Entity,
    records: &HashSet<SceneId>,
    has_record: &mut dyn FnMut(&AssetPath, SceneId) -> bool,
) -> bool {
    let (Some(link), Some(part)) = (
        world.get::<PrefabInstance>(candidate),
        world.get::<PrefabPart>(candidate),
    ) else {
        return false;
    };
    !records.contains(&part.record) && has_record(&link.source, part.record)
}

/// The instance at `root`: `root` and every entity under it that is part of
/// it, parents before children, and the roots of the instances added to it
/// (`added` picks them), which with what is under them are left out.
/// Bounded: a loop of parent links is walked once.
pub(super) fn members(
    world: &World,
    root: Entity,
    added: &mut dyn FnMut(&World, Entity) -> bool,
) -> (Vec<Entity>, Vec<Entity>) {
    let mut children: HashMap<Entity, Vec<Entity>> = HashMap::new();
    for (entity, link) in world.query::<Parent>() {
        if let Some(parent) = link.entity {
            children.entry(parent).or_default().push(entity);
        }
    }
    for list in children.values_mut() {
        list.sort_unstable_by_key(Entity::index);
    }
    let mut inside = vec![root];
    let mut boundaries = Vec::new();
    let mut seen = HashSet::from([root]);
    let mut at = 0;
    while at < inside.len() {
        let current = inside[at];
        at += 1;
        for &child in children.get(&current).into_iter().flatten() {
            if !seen.insert(child) {
                continue;
            }
            if added(world, child) {
                boundaries.push(child);
            } else {
                inside.push(child);
            }
        }
    }
    (inside, boundaries)
}

/// The ids of the records of `source` as the store reads it.
pub(super) fn record_ids(prefabs: &mut Prefabs, source: &AssetPath) -> HashSet<SceneId> {
    let handle = prefabs.load(source);
    prefabs
        .get(handle)
        .file
        .as_ref()
        .map(|file| file.entities.iter().map(|r| r.id).collect())
        .unwrap_or_default()
}

/// Whether `source`, as the store reads it, has `record` — yes for a file
/// that will not read, which cannot say.
pub(super) fn has_record(prefabs: &mut Prefabs, source: &AssetPath, record: SceneId) -> bool {
    let handle = prefabs.load(source);
    match &prefabs.get(handle).file {
        Some(file) => file.entities.iter().any(|r| r.id == record),
        None => true,
    }
}

/// [`members`] of the instance at `root`, its file and the files of what
/// is under it read through `prefabs`.
pub(super) fn members_in(
    world: &World,
    prefabs: &mut Prefabs,
    root: Entity,
) -> (Vec<Entity>, Vec<Entity>) {
    let Some(source) = world.get::<PrefabInstance>(root).map(|l| l.source.clone()) else {
        return (
            std::iter::once(root)
                .chain(hierarchy::descendants(world, root))
                .collect(),
            Vec::new(),
        );
    };
    let records = record_ids(prefabs, &source);
    members(world, root, &mut |world, child| {
        is_added(world, child, &records, &mut |source, record| {
            has_record(prefabs, source, record)
        })
    })
}

/// The instance `entity` belongs to: the nearest of itself and its
/// ancestors carrying a [`PrefabInstance`], then outwards through every
/// instance it is a nested part of — up to the first it was added to, or
/// the outermost. `None` for an entity in no instance.
pub fn instance_root(world: &World, prefabs: &mut Prefabs, entity: Entity) -> Option<Entity> {
    let links: Vec<Entity> = std::iter::once(entity)
        .chain(hierarchy::ancestors(world, entity))
        .filter(|&e| world.get::<PrefabInstance>(e).is_some())
        .collect();
    let mut found = *links.first()?;
    for &outer in &links[1..] {
        let Some(source) = world.get::<PrefabInstance>(outer).map(|l| l.source.clone()) else {
            continue;
        };
        let records = record_ids(prefabs, &source);
        let added = is_added(world, found, &records, &mut |source, record| {
            has_record(prefabs, source, record)
        });
        if added {
            break;
        }
        found = outer;
    }
    Some(found)
}
