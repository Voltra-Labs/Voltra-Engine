//! What a sync adds to an instance: the records its file gained, spawned
//! under the parts of their parent records.

use std::collections::{BTreeMap, HashMap, HashSet};

use ron::value::RawValue;
use voltra_ecs::{Entity, World};

use super::super::compare::{parent_record, STRUCTURE};
use super::super::membership::Children;
use super::super::PrefabPart;
use super::SyncReport;
use crate::format::{ComponentRegistry, EntityRecord, SceneError};
use crate::hierarchy::Parent;
use crate::scene_id::SceneId;

/// Spawns each of `records` that `wanted` keeps and the instance at `root`
/// has no part for, into `parts`, linked under its parent record's part.
/// `added` are the roots of the instances added to this one.
// The state of the one sync it is a step of, handed over whole.
#[allow(clippy::too_many_arguments)]
pub(super) fn gained(
    world: &mut World,
    registry: &ComponentRegistry,
    root: Entity,
    records: &[&EntityRecord],
    wanted: &dyn Fn(&EntityRecord) -> bool,
    parts: &mut HashMap<SceneId, Entity>,
    added: &[Entity],
    children: &Children,
    report: &mut SyncReport,
) -> Result<(), SceneError> {
    let mut spawned = Vec::new();
    // A record an instance added here already stands for — its parts as an
    // Apply writes them into this file, `SceneId::derived` from its root and
    // the record each names — is not spawned a second time beside it: an
    // Apply undone leaves the added instance as it was and the file written.
    let mut covered: HashSet<SceneId> = HashSet::new();
    let mut holders = added.to_vec();
    let mut seen_holders = HashSet::new();
    while let Some(at) = holders.pop() {
        if !seen_holders.insert(at) {
            continue;
        }
        let Some(&holder) = world.get::<SceneId>(at) else {
            continue;
        };
        let (inside, deeper) = super::super::membership::members(world, at, children);
        for entity in inside {
            if let Some(part) = world.get::<PrefabPart>(entity) {
                covered.insert(SceneId::derived(holder, part.record));
            }
        }
        holders.extend(deeper);
    }
    let missing: Vec<&EntityRecord> = records
        .iter()
        .copied()
        .filter(|r| wanted(r) && !parts.contains_key(&r.id) && !covered.contains(&r.id))
        .collect();
    let mut ids = fresh_ids(world, root, !missing.is_empty());
    for record in missing {
        let entity = world.spawn();
        world.insert(entity, ids(record.id));
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
    Ok(())
}

/// The id an entity spawned for `record` in the instance at `root` gets:
/// derived from the two ([`SceneId::derived`]), so a prefab read by the
/// store and the same file opened in prefab mode add an entity under one
/// id — but fresh where the world already holds that id (a part moved out
/// of its instance and spawned back). `needed` is whether any will be
/// asked for, so a sync that adds nothing does not gather the world's ids.
fn fresh_ids(world: &World, root: Entity, needed: bool) -> impl FnMut(SceneId) -> SceneId {
    let within = world.get::<SceneId>(root).copied();
    let mut taken: HashSet<SceneId> = if needed && within.is_some() {
        world.query::<SceneId>().map(|(_, id)| *id).collect()
    } else {
        HashSet::new()
    };
    move |record| {
        let id = match within {
            Some(within) => SceneId::derived(within, record),
            None => SceneId::new(),
        };
        if taken.insert(id) {
            id
        } else {
            SceneId::new()
        }
    }
}
