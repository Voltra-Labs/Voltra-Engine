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
//! **The root's `Transform` is always the instance's own**: it is where the
//! instance was put. So are the structure components — `Parent`, the link
//! and the part — which a sync rebuilds rather than copies.
//!
//! **An instance saved before parts existed is adopted** on its first sync:
//! its root is matched to the file's root of the same name (or the first),
//! each child to a child record of the same name under the matched parent,
//! and everything about it that differs from the file *now* is recorded as
//! overridden — so the first sync changes nothing it was given, and every
//! later edit to the file reaches it. A record nothing matched is taken as
//! deleted from the instance, not spawned into it.
//!
//! **Not yet: nested instances.** A prefab holding an instance of another
//! holds that instance's entities in full; they are parts of the outer
//! prefab, synced with it, and an edit to the inner file reaches them only
//! once the outer prefab is saved again. An instance root under another is
//! left to its outer one.

use std::collections::{BTreeMap, HashMap, HashSet};

use ron::value::RawValue;
use voltra_ecs::{Entity, World};

use super::adopt::adopt;
use super::compare::{canonical, differences, ids_to_records, parent_record, subtree, STRUCTURE};
use super::{Prefab, PrefabInstance, PrefabPart};
use crate::format::{ComponentRegistry, EntityRecord, SceneError};
use crate::hierarchy::{self, Parent};
use crate::scene_id::SceneId;

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
    }
}

/// Keeps the instance rooted at `root` in step with `prefab`.
pub fn sync(
    world: &mut World,
    registry: &ComponentRegistry,
    prefab: &Prefab,
    root: Entity,
) -> Result<SyncReport, SceneError> {
    let Some(file) = prefab.file.as_ref() else {
        // A file that would not read: keeping the instance as it is beats
        // despawning everything a missing record would mean.
        return Ok(SyncReport::default());
    };
    let mut report = SyncReport::default();
    if world.get::<PrefabPart>(root).is_none() {
        report.adopted = adopt(world, registry, &file.entities, root)?;
    }
    let Some(top) = world.get::<PrefabPart>(root).map(|part| part.record) else {
        return Ok(report);
    };
    let records = subtree(&file.entities, top);
    if records.is_empty() {
        log::warn!("an instance's root names a record its prefab no longer has; left as it is");
        return Ok(report);
    }
    let removed: HashSet<SceneId> = world
        .get::<PrefabInstance>(root)
        .map(|link| link.removed.iter().copied().collect())
        .unwrap_or_default();

    // The instance's parts, by record.
    let mut parts: HashMap<SceneId, Entity> = HashMap::new();
    for entity in std::iter::once(root).chain(hierarchy::descendants(world, root)) {
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
    let to_record = ids_to_records(world, root);
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
        let differ = differences(world, registry, entity, entity == root, &want, &to_record)?;
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
    let mut spawned = Vec::new();
    let missing: Vec<&EntityRecord> = records
        .iter()
        .copied()
        .filter(|r| wanted(r) && !parts.contains_key(&r.id))
        .collect();
    for record in missing {
        let entity = world.spawn();
        world.insert(entity, SceneId::new());
        let components: BTreeMap<String, Box<RawValue>> = record
            .components
            .iter()
            .filter(|(name, _)| !STRUCTURE.contains(&name.as_str()))
            .map(|(name, value)| (name.clone(), value.clone()))
            .collect();
        match crate::format::record::insert_components(world, registry, entity, &components) {
            Ok(unknown) if !unknown.0.is_empty() => {
                world.insert(entity, unknown);
            }
            Ok(_) => {}
            Err(error) => {
                world.despawn(entity);
                return Err(error);
            }
        }
        world.insert(entity, PrefabPart::new(record.id));
        parts.insert(record.id, entity);
        report.touched.push(entity);
        spawned.push((entity, record));
    }
    for (entity, record) in &spawned {
        let parent = parent_record(record).and_then(|id| parts.get(&id).copied());
        if let Some(id) = parent.and_then(|p| world.get::<SceneId>(p).copied()) {
            world.insert(*entity, Parent { id, entity: None });
        }
    }
    report.spawned = spawned.len();
    // A spawned reference names records; now that every record has its
    // entity, it names those.
    if !spawned.is_empty() {
        let to_instance: HashMap<SceneId, SceneId> = parts
            .iter()
            .filter_map(|(record, &e)| Some((*record, *world.get::<SceneId>(e)?)))
            .collect();
        for (entity, _) in &spawned {
            registry.remap_refs(world, *entity, &|id| to_instance.get(&id).copied());
        }
    }

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
    Ok(report)
}

#[cfg(test)]
mod tests;
