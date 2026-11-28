//! Keeping an instance in step with its prefab file.
//!
//! Unity's prefab propagation, by component: every entity of an instance
//! knows its record of the file ([`PrefabPart::record`]) and which of its
//! components the instance has made its own ([`PrefabPart::overridden`]).
//! A sync puts every other component back to what the file says now — set
//! where it differs, taken off where the file no longer has it — spawns the
//! records the file gained (under the entity of their parent record), and
//! despawns the parts whose record the file lost. What an instance added of
//! its own (an entity with no part, a component in its overridden set) is
//! never touched, and a record it deleted ([`PrefabInstance::removed`]) is
//! not spawned back.
//!
//! **Values are compared as their types write them** (`registry.normalize`):
//! a file is pretty-printed and a world's record is not, and text that
//! differs only in its spaces is the same value.
//!
//! **The root's transform is always the instance's own**, in either
//! dimension ([`super::Placement`]): it is where the instance was put. So
//! are the structure components — `Parent`, the link and the part — which a
//! sync rebuilds rather than copies. An instance whose root is of the other
//! dimension than its file's is left alone and reported
//! ([`SyncReport::other_dimension`]).
//!
//! **An instance saved before parts existed is adopted** on its first sync:
//! its root is matched to the file's root of the same name (or the first),
//! each child to a child record of the same name under the matched parent,
//! and everything about it that differs from the file *now* is recorded as
//! overridden — so the first sync changes nothing it was given, and every
//! later edit to the file reaches it. A record nothing matched is taken as
//! deleted from the instance, not spawned into it.
//!
//! **Nested instances follow through their outer prefab.** An instance of a
//! prefab that holds another is synced with the outer file *as the store
//! reads it* — the inner instance brought in step with the inner file
//! first, the outer file's overrides on it kept (`resolve.rs`) — so an edit
//! to the inner file reaches it without the outer one being saved again.
//! An instance root under another is left to its outer one.

mod gained;

use std::collections::{HashMap, HashSet};

use voltra_ecs::{Entity, World};

use super::adopt::adopt;
use super::compare::{canonical, differences, ids_to_records, parent_record, subtree};
use super::membership::{members, Children};
use super::{placement, Prefab, PrefabInstance, PrefabPart};
use crate::format::{ComponentRegistry, EntityRecord, SceneError};
use crate::hierarchy;
use crate::scene_id::SceneId;
use gained::gained;

/// What a sync did.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SyncReport {
    /// Components set back to the file's value, or taken off.
    pub changed: usize,
    /// Entities spawned for records the instance did not have.
    pub spawned: usize,
    /// Parts despawned because the file lost their record.
    pub despawned: usize,
    /// Entities of instances saved before parts, given one.
    pub adopted: usize,
    /// Every live entity whose components a sync set or spawned: what needs
    /// its assets looked up again, since a component read from a file has
    /// no handles.
    pub touched: Vec<Entity>,
    /// Instance roots of one dimension whose file's root is of the other —
    /// an Apply from an instance whose root changed dimension, say. A root's
    /// transform is its placement, which a sync never touches, and its parts
    /// taking the other dimension's would mix the two: such an instance is
    /// left as it is, named here, and warned about.
    pub other_dimension: Vec<Entity>,
}

impl SyncReport {
    /// Whether the sync changed nothing in the world.
    pub fn is_empty(&self) -> bool {
        self.changed == 0 && self.spawned == 0 && self.despawned == 0 && self.adopted == 0
    }

    pub(super) fn add(&mut self, other: Self) {
        self.changed += other.changed;
        self.spawned += other.spawned;
        self.despawned += other.despawned;
        self.adopted += other.adopted;
        self.touched.extend(other.touched);
        self.other_dimension.extend(other.other_dimension);
    }
}

/// Keeps the instance rooted at `root` in step with `prefab`.
///
/// An instance added under one of its parts — linked, its part naming no
/// in its own right ([`PrefabInstance::own`]) — is left alone, as an entity
/// the instance added is; [`super::sync_in`] syncs it with its own file
/// too.
pub fn sync(
    world: &mut World,
    registry: &ComponentRegistry,
    prefab: &Prefab,
    root: Entity,
) -> Result<SyncReport, SceneError> {
    let children = Children::of(world);
    sync_with(world, registry, prefab, root, &children).map(|(report, _)| report)
}

/// [`sync`], with the world's `children` gathered once by a caller syncing
/// many instances. Answers the roots of the instances added to this one,
/// still alive, for the caller to sync in turn.
pub(super) fn sync_with(
    world: &mut World,
    registry: &ComponentRegistry,
    prefab: &Prefab,
    root: Entity,
    children: &Children,
) -> Result<(SyncReport, Vec<Entity>), SceneError> {
    let Some(file) = prefab.file.as_ref() else {
        // A file that would not read: keeping the instance as it is beats
        // despawning everything a missing record would mean.
        return Ok((SyncReport::default(), Vec::new()));
    };
    let mut report = SyncReport::default();
    if world.get::<PrefabPart>(root).is_none() {
        report.adopted = adopt(world, registry, &file.entities, root)?;
    }
    let Some(top) = world.get::<PrefabPart>(root).map(|part| part.record) else {
        return Ok((report, Vec::new()));
    };
    let records = subtree(&file.entities, top);
    if records.is_empty() {
        log::warn!("an instance's root names a record its prefab no longer has; left as it is");
        return Ok((report, Vec::new()));
    }
    if let Some(file_root) = records.iter().find(|record| record.id == top) {
        let theirs = placement::record_dimensions(file_root);
        let ours = hierarchy::Dimension::of(world, root);
        if !theirs.is_empty() && !ours.is_empty() && theirs != ours {
            // Not synced at all: its parts would take the file's transforms
            // of the other dimension under a root that keeps its own, and
            // the instance would mix the two.
            log::warn!(
                "{root:?} is an instance of {}, whose root is now of the other dimension; \
                 the instance is left as it is: replace it (an Apply from it would convert \
                 the prefab back, and its other instances with it)",
                prefab
                    .source()
                    .map_or("an unsaved prefab", |path| path.as_str())
            );
            report.other_dimension.push(root);
            return Ok((report, Vec::new()));
        }
    }
    let removed: HashSet<SceneId> = world
        .get::<PrefabInstance>(root)
        .map(|link| link.removed.iter().copied().collect())
        .unwrap_or_default();

    // The instance's parts, by record — not those of an instance added to
    // it, which are its own file's.
    let (members, added) = members(world, root, children);
    let mut parts: HashMap<SceneId, Entity> = HashMap::new();
    for &entity in &members {
        if let Some(part) = world.get::<PrefabPart>(entity) {
            parts.entry(part.record).or_insert(entity);
        }
    }

    // What the instance should hold: every record but those it deleted and
    // what was under them.
    let by_id: HashMap<SceneId, &EntityRecord> = records.iter().map(|r| (r.id, *r)).collect();
    let wanted = |record: &EntityRecord| {
        let mut at = Some(record.id);
        for _ in 0..=records.len() {
            let Some(id) = at else {
                return true;
            };
            if removed.contains(&id) {
                return false;
            }
            if id == top {
                return true;
            }
            at = by_id.get(&id).and_then(|r| parent_record(r));
        }
        false
    };

    // Every part put back, but for what it overrides. A value naming other
    // entities is compared in the file's ids and put back in the instance's.
    let to_record = ids_to_records(world, &members);
    let to_instance: HashMap<SceneId, SceneId> = to_record
        .iter()
        .map(|(own, record)| (*record, *own))
        .collect();
    for record in &records {
        let Some(&entity) = parts.get(&record.id) else {
            continue;
        };
        let want = canonical(registry, record)?;
        let overridden = world
            .get::<PrefabPart>(entity)
            .map(|part| part.overridden.clone())
            .unwrap_or_default();
        let differ = differences(world, registry, entity, entity == root, &want, &|id| {
            to_record.get(&id).copied()
        })?;
        if differ.difference(&overridden).next().is_some() {
            report.touched.push(entity);
        }
        for name in differ.difference(&overridden) {
            match want.get(name) {
                Some(value) => {
                    let value = registry
                        .rewrite_refs(name, value, &|id| to_instance.get(&id).copied())?
                        .unwrap_or_else(|| value.clone());
                    if let Some(result) = registry.load_one(world, entity, name, &value) {
                        result?;
                    }
                }
                None => {
                    registry.remove_one(world, entity, name);
                }
            }
            report.changed += 1;
        }
    }

    // What the file gained.
    gained(
        world,
        registry,
        root,
        &records,
        &wanted,
        &mut parts,
        &added,
        children,
        &mut report,
    )?;

    // What the file lost: a part whose record is gone. What was added under
    // it goes with it, as a deleted parent's children do.
    let present: HashSet<SceneId> = records.iter().map(|r| r.id).collect();
    let lost: Vec<Entity> = parts
        .iter()
        .filter(|(record, _)| !present.contains(record))
        .map(|(_, &entity)| entity)
        .collect();
    for entity in lost {
        if world.is_alive(entity) {
            report.despawned += hierarchy::despawn_recursive(world, entity).len().max(1);
        }
    }

    if report.spawned > 0 {
        hierarchy::resolve_parents(world);
    }
    report.touched.retain(|&entity| world.is_alive(entity));
    let added = added.into_iter().filter(|&e| world.is_alive(e)).collect();
    Ok((report, added))
}

#[cfg(test)]
mod tests;
