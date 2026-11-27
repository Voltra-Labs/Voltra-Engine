//! Reading a scene file back into a world.

use std::path::Path;

use voltra_ecs::{Entity, World};

use super::error::SceneError;
use super::registry::ComponentRegistry;
use super::save::{EntityRecord, SceneFile, VERSION};
#[cfg(test)]
use crate::scene_id::SceneId;

/// Spawns every entity in `file` into `world`.
///
/// Adds rather than replaces. Whether to clear first is the caller's decision,
/// and the editor's menu is where that belongs.
///
/// All-or-nothing: if any entity fails to load, every entity this call spawned
/// is despawned before the error is returned, so a failed load leaves `world`
/// exactly as it found it. An editor that reports "could not open the scene"
/// and then shows half of it is worse than one that shows nothing.
///
/// The rollback is why the per-entity work lives in [`spawn_entities`] rather
/// than inline behind a `?`: `?` returns the instant something fails, which is
/// exactly the moment the list of entities to clean up would otherwise be
/// lost. `spawn_entities` instead returns the entities it spawned *alongside*
/// its `Result`, unconditionally, so there is a value to inspect and act on
/// before this function decides what to return.
pub fn from_scene_file(
    file: &SceneFile,
    registry: &ComponentRegistry,
    world: &mut World,
) -> Result<(), SceneError> {
    if file.version != VERSION {
        return Err(SceneError::UnsupportedVersion {
            found: file.version,
            supported: VERSION,
        });
    }

    let (spawned, result) = spawn_entities(&file.entities, registry, world);
    if let Err(error) = result {
        for entity in spawned {
            world.despawn(entity);
        }
        return Err(error);
    }

    // Only now: a file is a list, and a child may name a parent that appears
    // after it. Resolving as each entity spawned would leave every forward
    // reference permanently unresolved.
    crate::hierarchy::resolve_parents(world);

    log::info!("loaded {} entities", spawned.len());
    Ok(())
}

/// Spawns `records` into `world` under their own ids, and answers the
/// entities, in the records' order.
///
/// [`from_scene_file`]'s rules — always spawn, all-or-nothing, an unknown
/// component kept unread — for a few records rather than a file, and with
/// one difference: parent links are **not** resolved. A caller putting back
/// several batches (a client re-spawning what it set aside) resolves once,
/// with [`crate::hierarchy::resolve_parents`], after the last; resolving
/// the whole world per batch is what that saves.
///
/// The ids are the records' own, not fresh ones: the caller must know the
/// world does not hold them already, or two entities answer to one id.
pub fn spawn_records(
    records: &[EntityRecord],
    registry: &ComponentRegistry,
    world: &mut World,
) -> Result<Vec<Entity>, SceneError> {
    let (spawned, result) = spawn_entities(records, registry, world);
    match result {
        Ok(()) => Ok(spawned),
        Err(error) => {
            for entity in spawned {
                world.despawn(entity);
            }
            Err(error)
        }
    }
}

/// Spawns every entity of `records` into `world`, stopping at the first
/// error.
///
/// Always returns every entity spawned before it stopped, paired with the
/// outcome, rather than folding the two into a `Result<Vec<Entity>, (Vec<
/// Entity>, SceneError)>`: `clippy::result_large_err` correctly rejects that
/// shape (the tuple makes `SceneError`'s size infectious), and the caller
/// needs the spawned list in both the success and the failure case anyway, so
/// a plain pair is both the smaller type and the fit.
fn spawn_entities(
    records: &[EntityRecord],
    registry: &ComponentRegistry,
    world: &mut World,
) -> (Vec<Entity>, Result<(), SceneError>) {
    let mut spawned = Vec::with_capacity(records.len());

    for record in records {
        let entity = world.spawn();
        spawned.push(entity);
        world.insert(entity, record.id);

        // Shared with `apply_record`, which reads the same components onto an
        // entity that may already exist. A load always spawns, so it keeps the
        // spawn and hands over only the reading.
        let unknown =
            match super::record::insert_components(world, registry, entity, &record.components) {
                Ok(unknown) => unknown,
                // The whole load goes down rather than leaving this entity
                // half-built; the caller despawns everything in `spawned`.
                Err(error) => return (spawned, Err(error)),
            };

        if !unknown.0.is_empty() {
            world.insert(entity, unknown);
        }
    }

    (spawned, Ok(()))
}

/// Reads `path` and spawns its entities into `world`.
pub fn load(
    path: &Path,
    registry: &ComponentRegistry,
    world: &mut World,
) -> Result<(), SceneError> {
    let text = voltra_assets::fs::read_to_string(path)?;
    let file: SceneFile = ron::from_str(&text).map_err(SceneError::Parse)?;
    from_scene_file(&file, registry, world)
}

#[cfg(test)]
mod tests;
