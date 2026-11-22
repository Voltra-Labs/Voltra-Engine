//! Finding a world's prefab instances, and bringing them all in step.

use voltra_assets::AssetPath;
use voltra_ecs::{Entity, World};

use super::sync::{sync, SyncReport};
use super::{PrefabInstance, Prefabs};
use crate::format::ComponentRegistry;
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

fn sync_matching(
    world: &mut World,
    registry: &ComponentRegistry,
    prefabs: &mut Prefabs,
    matches: impl Fn(&AssetPath) -> bool,
) -> SyncReport {
    let mut total = SyncReport::default();
    for (root, source) in instance_roots(world) {
        if !matches(&source) || !world.is_alive(root) {
            continue;
        }
        let handle = prefabs.load(&source);
        match sync(world, registry, prefabs.get(handle), root) {
            Ok(report) => total.add(report),
            Err(error) => log::error!(
                "could not bring an instance of {} in step with it: {error}",
                source.as_str()
            ),
        }
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
