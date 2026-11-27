//! Finding a world's prefab instances, and bringing them all in step.

use voltra_assets::AssetPath;
use voltra_ecs::{Entity, World};

use std::collections::HashSet;

use super::membership::{members, Children};
use super::sync::{sync_with, SyncReport};
use super::{PrefabInstance, Prefabs};
use crate::format::{ComponentRegistry, SceneError};
use crate::hierarchy;

/// Every instance root in `world` not under another instance, with its
/// link's source.
fn instance_roots(world: &World) -> Vec<(Entity, AssetPath)> {
    world
        .query::<PrefabInstance>()
        .filter(|(entity, _)| {
            hierarchy::ancestors(world, *entity)
                .iter()
                .all(|&above| world.get::<PrefabInstance>(above).is_none())
        })
        .map(|(entity, link)| (entity, link.source.clone()))
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
    let children = Children::of(world);
    sync_tree(
        world,
        registry,
        prefabs,
        root,
        &children,
        &|_| true,
        &mut total,
    )?;
    Ok(total)
}

fn sync_matching(
    world: &mut World,
    registry: &ComponentRegistry,
    prefabs: &mut Prefabs,
    matches: impl Fn(&AssetPath) -> bool,
) -> SyncReport {
    let mut total = SyncReport::default();
    let roots = instance_roots(world);
    // An outermost root is an instance in its own right, whatever a file
    // from before stage 126 says: remembered, so it stays one when moved
    // under another instance.
    for (root, _) in &roots {
        if let Some(link) = world.get_mut::<PrefabInstance>(*root) {
            link.own = true;
        }
    }
    // Gathered once: the syncs below change only their own instances.
    let children = Children::of(world);
    for (root, _) in roots {
        if !world.is_alive(root) {
            continue;
        }
        // Logged where it failed; the rest go on.
        sync_tree(
            world, registry, prefabs, root, &children, &matches, &mut total,
        )
        .ok();
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
    children: &Children,
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
        if !matches(&source) {
            queue.extend(members(world, at, children).1);
            continue;
        }
        let handle = prefabs.load(&source);
        match sync_with(world, registry, prefabs.get(handle), at, children) {
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
