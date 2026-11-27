//! An instance saved before parts existed, given them: matched by name to
//! its file's records, and everything about it that differs from the file
//! now recorded as overridden — so its first sync changes nothing it was
//! given, and every later edit to the file reaches it.

use std::collections::HashSet;

use voltra_ecs::{Entity, World};

use super::compare::{canonical, differences, parent_record, subtree};
use super::{PrefabInstance, PrefabPart};
use crate::format::{ComponentRegistry, EntityRecord, SceneError};
use crate::hierarchy;
use crate::name::Name;
use crate::scene_id::SceneId;

/// Gives an instance saved before parts existed its parts: matched by name
/// to the file's records, everything that differs now overridden. Answers
/// how many entities were matched.
pub(super) fn adopt(
    world: &mut World,
    registry: &ComponentRegistry,
    records: &[EntityRecord],
    root: Entity,
) -> Result<usize, SceneError> {
    let name_of_record = |record: &EntityRecord| {
        record
            .components
            .get("Name")
            .and_then(|raw| raw.into_rust::<Name>().ok())
    };
    let name_of = |world: &World, entity: Entity| world.get::<Name>(entity).cloned();
    let tops: Vec<&EntityRecord> = records
        .iter()
        .filter(|r| parent_record(r).is_none())
        .collect();
    let root_name = name_of(world, root);
    let Some(top) = tops
        .iter()
        .find(|r| root_name.is_some() && name_of_record(r) == root_name)
        .or(tops.first())
    else {
        return Ok(0);
    };

    let mut matched: Vec<(Entity, &EntityRecord)> = vec![(root, *top)];
    let mut queue = vec![(root, top.id)];
    while let Some((entity, record)) = queue.pop() {
        let mut free: Vec<&EntityRecord> = records
            .iter()
            .filter(|r| parent_record(r) == Some(record))
            .collect();
        for child in hierarchy::children_of(world, entity) {
            let name = name_of(world, child);
            if let Some(at) = free
                .iter()
                .position(|r| name.is_some() && name_of_record(r) == name)
            {
                let found = free.remove(at);
                matched.push((child, found));
                queue.push((child, found.id));
            }
        }
    }

    let held: HashSet<SceneId> = matched.iter().map(|(_, r)| r.id).collect();
    let unmatched: Vec<SceneId> = subtree(records, top.id)
        .iter()
        .map(|r| r.id)
        .filter(|id| !held.contains(id))
        .collect();
    let to_record: std::collections::HashMap<SceneId, SceneId> = matched
        .iter()
        .filter_map(|&(entity, record)| Some((*world.get::<SceneId>(entity)?, record.id)))
        .collect();
    for &(entity, record) in &matched {
        let want = canonical(registry, record)?;
        let overridden = differences(world, registry, entity, entity == root, &want, &|id| {
            to_record.get(&id).copied()
        })?;
        world.insert(
            entity,
            PrefabPart {
                record: record.id,
                overridden,
            },
        );
    }
    if let Some(link) = world.get_mut::<PrefabInstance>(root) {
        link.removed.extend(unmatched);
    }
    Ok(matched.len())
}
