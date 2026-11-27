//! Finding a world's prefab instances, and bringing them all in step.

use voltra_assets::AssetPath;
use voltra_ecs::{Entity, World};

use std::collections::HashSet;

use super::membership::members_in;
use super::sync::{sync_with, SyncReport};
use super::{PrefabInstance, Prefabs};
use crate::format::{ComponentRegistry, SceneError};
use crate::hierarchy;

/// Every instance root in `world` not under another instance, with its
/// link's source.
fn instance_roots(world: &World) -> Vec<(Entity, AssetPath)> {
    let roots: Vec<(Entity, AssetPath)> = world
        .query::<PrefabInstance>()
        .map(|(entity, link)| (entity, link.source.clone()))
        .collect();
    roots
        .iter()
        .filter(|(entity, _)| {
            !roots
                .iter()
                .any(|(other, _)| other != entity && hierarchy::is_ancestor(world, *other, *entity))
        })
        .cloned()
        .collect()
}

/// Syncs every instance in `world` — after a scene loads, so an edit made to
/// a prefab while the scene was closed reaches it. A prefab that will not
/// read leaves its instances as they are; an instance that will not sync is
/// logged and the rest go on.
pub fn sync_all(
    world: &mut World,
    registry: &ComponentRegistry,
    prefabs: &mut Prefabs,
) -> SyncReport {
    sync_matching(world, registry, prefabs, |_| true)
}

/// Syncs the instances of `source` — after it is hot reloaded.
pub fn sync_source(
    world: &mut World,
    registry: &ComponentRegistry,
    prefabs: &mut Prefabs,
    source: &AssetPath,
) -> SyncReport {
    sync_matching(world, registry, prefabs, |path| path == source)
}

/// Syncs the instances of every prefab in `sources` — what
/// [`Prefabs::reload`] answers: the file that changed, and every variant
/// and outer prefab that follows it.
pub fn sync_sources(
    world: &mut World,
    registry: &ComponentRegistry,
    prefabs: &mut Prefabs,
    sources: &[AssetPath],
) -> SyncReport {
    if sources.is_empty() {
        return SyncReport::default();
    }
    sync_matching(world, registry, prefabs, |path| sources.contains(path))
}

/// Syncs the instance at `root` with its file, then every instance added
/// to it with its own, and so on inwards: the outer one first, so it never
/// takes an added one's parts for its own.
pub fn sync_in(
    world: &mut World,
    registry: &ComponentRegistry,
    prefabs: &mut Prefabs,
    root: Entity,
) -> Result<SyncReport, SceneError> {
    let mut total = SyncReport::default();
    sync_tree(world, registry, prefabs, root, &|_| true, &mut total)?;
    Ok(total)
}

fn sync_matching(
    world: &mut World,
    registry: &ComponentRegistry,
    prefabs: &mut Prefabs,
    matches: impl Fn(&AssetPath) -> bool,
) -> SyncReport {
    let mut total = SyncReport::default();
    for (root, _) in instance_roots(world) {
        if !world.is_alive(root) {
            continue;
        }
        // Logged where it failed; the rest go on.
        sync_tree(world, registry, prefabs, root, &matches, &mut total).ok();
    }
    if !total.is_empty() {
        log::info!(
            "prefab instances: {} components updated, {} entities spawned, {} despawned, {} adopted",
            total.changed,
            total.spawned,
            total.despawned,
            total.adopted
        );
    }
    total
}

/// The instance at `root` and the ones added to it, inwards, each synced
/// when `matches` its file. An instance that will not sync is logged and
/// the rest go on; the first error is answered.
fn sync_tree(
    world: &mut World,
    registry: &ComponentRegistry,
    prefabs: &mut Prefabs,
    root: Entity,
    matches: &dyn Fn(&AssetPath) -> bool,
    total: &mut SyncReport,
) -> Result<(), SceneError> {
    let mut first_error = None;
    let mut queue = vec![root];
    let mut seen = HashSet::new();
    while let Some(at) = queue.pop() {
        if !world.is_alive(at) || !seen.insert(at) {
            continue;
        }
        let Some(source) = world.get::<PrefabInstance>(at).map(|l| l.source.clone()) else {
            continue;
        };
        let (_, added) = members_in(world, prefabs, at);
        if !matches(&source) {
            queue.extend(added);
            continue;
        }
        let added: HashSet<Entity> = added.into_iter().collect();
        let handle = prefabs.load(&source);
        match sync_with(world, registry, prefabs.get(handle), at, Some(&added)) {
            Ok((report, inner)) => {
                total.add(report);
                queue.extend(inner);
            }
            Err(error) => {
                log::error!(
                    "could not bring an instance of {} in step with it: {error}",
                    source.as_str()
                );
                first_error.get_or_insert(error);
            }
        }
    }
    first_error.map_or(Ok(()), Err)
}
