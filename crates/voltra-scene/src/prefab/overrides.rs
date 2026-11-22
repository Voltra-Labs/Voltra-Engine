//! What an instance has made its own, kept up to date as it is edited, and
//! given back to the prefab on request.
//!
//! **An override is a difference, recorded when it is made.** The editor
//! calls [`refresh_overrides`] on what it edits, and a part's overridden set
//! becomes exactly the components that now differ from its record — so an
//! edit made and then undone, or a value typed back to the prefab's, is no
//! longer one. Recorded rather than worked out at sync time, because by then
//! the file has changed and every component the edit did not touch would
//! look overridden too. Unity keeps its property modifications the same way.

use voltra_ecs::{Entity, World};

use super::compare::{canonical, differences};
use super::sync::{sync, SyncReport};
use super::{PrefabInstance, PrefabPart, Prefabs};
use crate::format::{ComponentRegistry, EntityRecord, SceneError};
use crate::hierarchy;

/// The instance `entity` belongs to: the outermost of itself and its
/// ancestors that carries a [`PrefabInstance`]. A nested instance's entities
/// belong to the outer one, as their parts do.
pub fn instance_root(world: &World, entity: Entity) -> Option<Entity> {
    let mut found = world.get::<PrefabInstance>(entity).map(|_| entity);
    let mut up = hierarchy::parent_of(world, entity);
    // Bounded: a cycle in a hand-edited file ends.
    for _ in 0..4096 {
        let Some(at) = up else {
            break;
        };
        if world.get::<PrefabInstance>(at).is_some() {
            found = Some(at);
        }
        up = hierarchy::parent_of(world, at);
    }
    found
}

/// The record `entity`'s part names, in its instance's prefab, with whether
/// the entity is that instance's root. `None` for an entity that is no part,
/// or whose prefab or record is gone.
fn record_of<'p>(
    world: &World,
    prefabs: &'p mut Prefabs,
    entity: Entity,
) -> Option<(&'p EntityRecord, bool)> {
    let root = instance_root(world, entity)?;
    let source = world.get::<PrefabInstance>(root)?.source.clone();
    let record = world.get::<PrefabPart>(entity)?.record;
    let handle = prefabs.load(&source);
    let file = prefabs.get(handle).file.as_ref()?;
    let found = file.entities.iter().find(|r| r.id == record)?;
    Some((found, entity == root))
}

/// Sets `entity`'s overridden set to what differs from its record now.
/// Answers whether the set changed — and leaves the part untouched when it
/// did not, so a frame that edited nothing records nothing.
pub fn refresh_overrides(
    world: &mut World,
    registry: &ComponentRegistry,
    prefabs: &mut Prefabs,
    entity: Entity,
) -> Result<bool, SceneError> {
    let Some((record, is_root)) = record_of(world, prefabs, entity) else {
        return Ok(false);
    };
    let want = canonical(registry, record)?;
    let differ = differences(world, registry, entity, is_root, &want)?;
    let Some(part) = world.get_mut::<PrefabPart>(entity) else {
        return Ok(false);
    };
    if part.overridden == differ {
        return Ok(false);
    }
    part.overridden = differ;
    Ok(true)
}

/// Puts `name` on `entity` back to what its prefab says — every component it
/// overrides, for `None` — and brings its instance in step.
pub fn revert(
    world: &mut World,
    registry: &ComponentRegistry,
    prefabs: &mut Prefabs,
    entity: Entity,
    name: Option<&str>,
) -> Result<SyncReport, SceneError> {
    let Some(root) = instance_root(world, entity) else {
        return Ok(SyncReport::default());
    };
    if let Some(part) = world.get_mut::<PrefabPart>(entity) {
        match name {
            Some(name) => {
                part.overridden.remove(name);
            }
            None => part.overridden.clear(),
        }
    }
    sync_root(world, registry, prefabs, root)
}

/// Puts the whole instance at `root` back to its prefab: every override on
/// every part, and every record it deleted spawned back.
pub fn revert_instance(
    world: &mut World,
    registry: &ComponentRegistry,
    prefabs: &mut Prefabs,
    root: Entity,
) -> Result<SyncReport, SceneError> {
    for entity in std::iter::once(root).chain(hierarchy::descendants(world, root)) {
        if let Some(part) = world.get_mut::<PrefabPart>(entity) {
            part.overridden.clear();
        }
    }
    if let Some(link) = world.get_mut::<PrefabInstance>(root) {
        link.removed.clear();
    }
    sync_root(world, registry, prefabs, root)
}

fn sync_root(
    world: &mut World,
    registry: &ComponentRegistry,
    prefabs: &mut Prefabs,
    root: Entity,
) -> Result<SyncReport, SceneError> {
    let Some(source) = world.get::<PrefabInstance>(root).map(|l| l.source.clone()) else {
        return Ok(SyncReport::default());
    };
    let handle = prefabs.load(&source);
    sync(world, registry, prefabs.get(handle), root)
}

/// Before `entity` is deleted from its instance: records it, and every part
/// under it, as deleted on the instance's root, so a sync does not spawn
/// them back. Answers the root it wrote to; nothing when `entity` is the
/// root itself (the whole instance goes) or belongs to no instance.
pub fn note_removed(world: &mut World, entity: Entity) -> Option<Entity> {
    let root = instance_root(world, entity)?;
    if root == entity {
        return None;
    }
    let records: Vec<_> = std::iter::once(entity)
        .chain(hierarchy::descendants(world, entity))
        .filter_map(|e| world.get::<PrefabPart>(e).map(|part| part.record))
        .collect();
    let link = world.get_mut::<PrefabInstance>(root)?;
    for record in records {
        if !link.removed.contains(&record) {
            link.removed.push(record);
        }
    }
    Some(root)
}

#[cfg(test)]
mod tests;
