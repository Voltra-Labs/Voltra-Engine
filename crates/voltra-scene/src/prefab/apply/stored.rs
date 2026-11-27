//! The instances a prefab holds, kept through an Apply: their links and
//! parts put back on the records written from a scene's instance, and what
//! each overrides worked out again against its own file.

use std::collections::{HashMap, HashSet};

use ron::value::RawValue;
use voltra_ecs::{Entity, World};

use super::super::compare::parent_record;
use super::super::membership::instance_root;
use super::super::overrides::refresh_overrides;
use super::super::resolve::{spawn_stored, written};
use super::super::{PrefabInstance, PrefabPart, Prefabs};
use crate::format::{ComponentRegistry, EntityRecord, SceneError, SceneFile};
use crate::scene_id::SceneId;

/// Puts back on `records` — written from `kept`, the instance at `root` —
/// the links and parts the file held (`resolved`, as the store reads it).
///
/// - A record the file had takes the file's link and part: a variant's root
///   stays an instance of its base, a nested prefab's entities parts of it.
/// - A record the instance added keeps the link of a prefab dropped into
///   the instance, and its parts where they name records of that prefab:
///   it becomes an instance stored in the file.
/// - A part of a stored instance the instance deleted is remembered on that
///   instance, so reading the file does not spawn it back.
pub(super) fn restore(
    world: &World,
    prefabs: &mut Prefabs,
    resolved: &SceneFile,
    kept: &[Entity],
    records: &mut [EntityRecord],
    root: Entity,
) -> Result<(), SceneError> {
    let mut by_id: HashMap<SceneId, &EntityRecord> = HashMap::new();
    for record in &resolved.entities {
        by_id.entry(record.id).or_insert(record);
    }
    let present: HashSet<SceneId> = records.iter().map(|r| r.id).collect();

    for (record, &entity) in records.iter_mut().zip(kept) {
        if let Some(was) = by_id.get(&record.id) {
            record.components.remove(PrefabInstance::NAME);
            for name in [PrefabInstance::NAME, PrefabPart::NAME] {
                if let Some(value) = was.components.get(name) {
                    record.components.insert(name.to_owned(), value.clone());
                }
            }
            continue;
        }
        let Some(part) = world.get::<PrefabPart>(entity) else {
            continue;
        };
        // Its part names a record of the instance added to this one it is
        // in, which becomes an instance stored in the file.
        let Some(holder) = instance_root(world, entity).filter(|&h| h != root) else {
            continue;
        };
        let Some(source) = world
            .get::<PrefabInstance>(holder)
            .map(|l| l.source.clone())
        else {
            continue;
        };
        let handle = prefabs.load(&source);
        let names_its_record = prefabs
            .get(handle)
            .file
            .as_ref()
            .is_some_and(|file| file.entities.iter().any(|r| r.id == part.record));
        if names_its_record {
            let value = RawValue::from_rust(&PrefabPart::new(part.record))
                .map_err(SceneError::Serialize)?;
            record.components.insert(PrefabPart::NAME.to_owned(), value);
        }
    }

    // Parts the instance deleted, remembered on their stored instance.
    let index: HashMap<SceneId, usize> = records
        .iter()
        .enumerate()
        .map(|(at, r)| (r.id, at))
        .collect();
    for was in &resolved.entities {
        if present.contains(&was.id) {
            continue;
        }
        let Some(part) = was
            .components
            .get(PrefabPart::NAME)
            .and_then(|raw| raw.into_rust::<PrefabPart>().ok())
        else {
            continue;
        };
        let Some(holder) = outermost_link(&by_id, was) else {
            continue;
        };
        let Some(&at) = index.get(&holder) else {
            continue;
        };
        let record = &mut records[at];
        let Some(mut link) = record
            .components
            .get(PrefabInstance::NAME)
            .and_then(|raw| raw.into_rust::<PrefabInstance>().ok())
        else {
            continue;
        };
        if !link.removed.contains(&part.record) {
            link.removed.push(part.record);
            let value = RawValue::from_rust(&link).map_err(SceneError::Serialize)?;
            record
                .components
                .insert(PrefabInstance::NAME.to_owned(), value);
        }
    }
    Ok(())
}

/// `file` with what every part of an instance stored in it overrides
/// worked out again against that instance's own file — the stored layer's
/// overrides are what differs from it now, as an instance's in a scene are.
pub(super) fn refresh(
    registry: &ComponentRegistry,
    prefabs: &mut Prefabs,
    file: &SceneFile,
) -> Result<SceneFile, SceneError> {
    let holds = file
        .entities
        .iter()
        .any(|r| r.components.contains_key(PrefabInstance::NAME));
    if !holds {
        return Ok(file.clone());
    }
    let mut world = World::new();
    let entities = spawn_stored(&mut world, registry, file)?;
    for &entity in &entities {
        if world.get::<PrefabPart>(entity).is_some() {
            refresh_overrides(&mut world, registry, prefabs, entity)?;
        }
    }
    written(&world, registry, &entities)
}

/// The outermost of `record` and the records above it carrying a link: the
/// instance stored in the file that its part belongs to.
fn outermost_link(
    by_id: &HashMap<SceneId, &EntityRecord>,
    record: &EntityRecord,
) -> Option<SceneId> {
    let mut found = None;
    let mut at = Some(record);
    for _ in 0..=by_id.len() {
        let Some(here) = at else {
            break;
        };
        if here.components.contains_key(PrefabInstance::NAME) {
            found = Some(here.id);
        }
        at = parent_record(here).and_then(|p| by_id.get(&p).copied());
    }
    found
}
