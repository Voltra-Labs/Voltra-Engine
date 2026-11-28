//! Making the client's copy of an entity the server spawned.
//!
//! By id first: a level loaded on both ends from one file has the same
//! [`SceneId`]s, and its entities are adopted rather than doubled. Then
//! what was set aside under that id when it went out of interest
//! ([`Stash`]), spawned back as it was. Then by
//! prefab: the client spawns the prefab the server did — the sprite, the
//! collider, the children that only draw — and the replicated components
//! are laid over it. Godot's `MultiplayerSpawner` spawns by scene path and
//! Unity's Netcode by prefab hash for the same reason: what never changes
//! in play should not be on the wire. Last, a bare entity, for one spawned
//! from code.

use std::collections::HashMap;

use voltra_assets::AssetPath;
use voltra_ecs::{Entity, World};
use voltra_net::PeerId;
use voltra_scene::{hierarchy, prefab, ComponentRegistry, Placement, Prefabs, Replicated, SceneId};

use super::stash::Stash;
use crate::wire::Spawn;
use crate::NetOwner;

/// Where the world's ids are, built the first time a snapshot needs it.
pub(crate) type SceneIndex = Option<HashMap<SceneId, Entity>>;

/// What a spawn may come from besides the world and the prefabs.
pub(crate) struct Sources<'a> {
    /// Whether an entity of the world is already another replicated one's.
    pub(crate) bound: &'a dyn Fn(Entity) -> bool,
    /// What went out of interest.
    pub(crate) stash: &'a mut Stash,
    /// Set when something came out of the stash: its parents are left to
    /// resolve once the snapshot is in.
    pub(crate) restored: &'a mut bool,
}

/// The client's entity for `spawn`, made or adopted; every entity made is
/// pushed on `made`, for the loop to resolve its assets.
pub(crate) fn create(
    world: &mut World,
    registry: &ComponentRegistry,
    prefabs: Option<&mut Prefabs>,
    spawn: &Spawn,
    index: &mut SceneIndex,
    sources: &mut Sources<'_>,
    made: &mut Vec<Entity>,
) -> Entity {
    let index = index.get_or_insert_with(|| {
        world
            .query::<SceneId>()
            .map(|(entity, id)| (*id, entity))
            .collect()
    });
    if let Some(&entity) = index.get(&spawn.id) {
        if world.is_alive(entity) && !(sources.bound)(entity) {
            set_owner(world, entity, spawn.owner);
            if world.get::<Replicated>(entity).is_none() {
                world.insert(entity, Replicated::default());
            }
            return entity;
        }
    }
    if let Some(entities) = sources.stash.restore(world, registry, spawn.id) {
        *sources.restored = true;
        let entity = entities[0];
        made.extend(entities);
        if world.get::<Replicated>(entity).is_none() {
            world.insert(entity, Replicated::default());
        }
        set_owner(world, entity, spawn.owner);
        index.insert(spawn.id, entity);
        return entity;
    }

    let entity = spawn
        .prefab
        .as_deref()
        .and_then(|path| from_prefab(world, registry, prefabs, path, made))
        .unwrap_or_else(|| {
            let entity = world.spawn();
            made.push(entity);
            entity
        });
    world.insert(entity, spawn.id);
    if world.get::<Replicated>(entity).is_none() {
        world.insert(entity, Replicated::default());
    }
    set_owner(world, entity, spawn.owner);
    index.insert(spawn.id, entity);
    entity
}

/// The owner a spawn names, or none.
pub(crate) fn set_owner(world: &mut World, entity: Entity, owner: Option<u32>) {
    match owner {
        Some(peer) => {
            world.insert(entity, NetOwner(PeerId(peer)));
        }
        None => {
            world.remove::<NetOwner>(entity);
        }
    }
}

fn from_prefab(
    world: &mut World,
    registry: &ComponentRegistry,
    prefabs: Option<&mut Prefabs>,
    path: &str,
    made: &mut Vec<Entity>,
) -> Option<Entity> {
    let prefabs = prefabs?;
    let path = match AssetPath::new(path) {
        Ok(path) => path,
        Err(error) => {
            log::warn!("the server named prefab {path:?}, not a path here: {error}");
            return None;
        }
    };
    let handle = prefabs.load(&path);
    let prefab = prefabs.try_get(handle)?;
    let instance = match prefab::spawn(world, registry, prefab, Placement::AsAuthored) {
        Ok(instance) => instance,
        Err(error) => {
            log::warn!("prefab {path:?} would not spawn for a replicated entity: {error}");
            return None;
        }
    };
    let root = instance.root()?;
    // A replicated part of the prefab is its own entity on the server, and
    // comes in its own spawn: the local copy would be a second one.
    for &entity in &instance.entities {
        if entity != root && world.is_alive(entity) && world.get::<Replicated>(entity).is_some() {
            hierarchy::despawn_recursive(world, entity);
        }
    }
    made.extend(
        instance
            .entities
            .iter()
            .copied()
            .filter(|&e| world.is_alive(e)),
    );
    Some(root)
}

#[cfg(test)]
mod tests {
    use voltra_render::glam::{Quat, Vec3};
    use voltra_scene::Transform3D;

    use super::*;

    #[test]
    fn a_3d_prefab_spawns_as_authored_for_a_replicated_entity() {
        // The server's placement arrives as the replicated transform; the
        // client's copy must not move the root on its own first.
        let root = voltra_testkit::scratch_root();
        let authored = Transform3D::from_translation(Vec3::new(4.0, 1.0, -2.0))
            .with_rotation(Quat::from_rotation_y(0.5));
        let mut source = World::new();
        let ship = source.spawn();
        source.insert(ship, SceneId::new());
        source.insert(ship, authored);
        let registry = ComponentRegistry::with_defaults();
        voltra_scene::format::save(&source, &registry, &root.join("ship.prefab.ron"))
            .expect("the fixture writes");
        let mut prefabs = Prefabs::new(&root);
        let mut world = World::new();
        let mut made = Vec::new();

        let entity = from_prefab(
            &mut world,
            &registry,
            Some(&mut prefabs),
            "ship.prefab.ron",
            &mut made,
        )
        .expect("spawned");

        assert_eq!(world.get::<Transform3D>(entity), Some(&authored));
        assert_eq!(made, vec![entity]);
    }
}
