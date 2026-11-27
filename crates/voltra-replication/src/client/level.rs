//! What a client keeps of its own copy of the level: its replicated
//! entities set aside until the server sends each, let go of with their
//! level.

use std::collections::HashSet;

use voltra_ecs::World;
use voltra_scene::format::EntityRecord;
use voltra_scene::{ComponentRegistry, SceneId};

use super::ReplicationClient;
use crate::wire::{NetId, MAX_LISTED};

impl ReplicationClient {
    /// Lets go of the level entities set aside at the first complete
    /// snapshot that the server never sent: call it when the level they
    /// came from goes (a scene change, a region unloaded), or they would be
    /// kept for the session. What the server culled is kept — it knows.
    pub fn forget_unsent(&mut self) {
        self.stash.forget_unnumbered();
    }

    /// [`forget_unsent`](Self::forget_unsent), of the entities `ids` names
    /// alone: when one region of a streamed world unloads, the rest of the
    /// level — and its swept entities — stays.
    pub fn forget_unsent_of(&mut self, ids: &[SceneId]) {
        self.stash.forget_unnumbered_of(ids);
    }

    /// Sets aside the replicated entities of a level this client is
    /// loading — `records`, what its scene load left out
    /// (`StagedScene::take`), parents first — to dress the server's copy of
    /// each in when its spawn arrives: the server's entity is spawned from
    /// the level's own records, sprite and children and all, rather than
    /// bare. The server is the one that says which of them exist; one it
    /// never sends stays set aside until the level goes
    /// ([`forget_unsent`](Self::forget_unsent)). One whose id an entity of
    /// `world` still holds — a persistent one — is left out.
    pub fn keep_level(
        &mut self,
        world: &World,
        registry: &ComponentRegistry,
        records: Vec<EntityRecord>,
    ) {
        let replicated: Vec<&str> = registry.replicated().iter().map(|c| c.name).collect();
        let held: HashSet<SceneId> = world.query::<SceneId>().map(|(_, id)| *id).collect();
        self.stash
            .keep_records(records, &replicated, &|id| held.contains(&id));
    }

    /// Asks the server to send again every entity it sent that is gone
    /// from `world`: call it after a scene change sweeps the old level,
    /// which takes with it what the server had already sent of the new one
    /// (a server that changed first). Unasked, such an entity would only
    /// come back once it changed; sent again, it is adopted as the level's
    /// copy or spawned from what [`keep_level`](Self::keep_level) set aside.
    pub fn resync_despawned(&mut self, world: &World) {
        let gone: Vec<NetId> = self
            .bound
            .iter()
            .filter(|(_, bound)| !world.is_alive(bound.entity))
            .map(|(&id, _)| id)
            .collect();
        for id in gone {
            if let Some(bound) = self.bound.remove(&id) {
                self.by_entity.remove(&bound.entity);
            }
            // Past the list's room, the next update of each asks the same.
            if self.resync.len() < MAX_LISTED && !self.resync.contains(&id) {
                self.resync.push(id);
            }
        }
    }
}
