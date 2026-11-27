//! A scene spawned a record at a time, so a load can spread over frames.
//!
//! [`spawn_scene`](super::spawn_scene) makes every entity in one call,
//! which is what a level change wants and what a big world cannot afford:
//! a region of a few thousand entities is tens of milliseconds, and the
//! frame it lands in stutters. Unreal adds a streamed level's actors over
//! frames under `s.LevelStreamingActorsUpdateTimeLimit` and Unity
//! integrates an async load under `Application.backgroundLoadingPriority`;
//! this is the part that makes that possible here — the caller decides when
//! to stop.
//!
//! The rules are `spawn_scene`'s: fresh ids (or the file's own, with
//! [`StagedScene::keep_ids`]), parent links and references
//! inside the file remapped, a link to a parent outside it cut, nothing
//! placed or linked. What differs is the order: **parents before children**,
//! so every entity is complete — its parent resolved — the moment it is
//! spawned, and a load stopped half way leaves no child hanging off a
//! parent still to come. [`StagedScene::new`] works the order out and
//! touches no world, so it can run on any thread.

use std::collections::HashMap;
use std::path::Path;

use voltra_assets::AssetPath;
use voltra_ecs::{Entity, World};

use crate::format::{ComponentRegistry, EntityRecord, SceneError, SceneFile};
use crate::hierarchy::Parent;
use crate::scene_id::SceneId;

mod order;

use order::parents_first;

/// The name a parent link is stored under in a scene file.
const PARENT: &str = Parent::NAME;

/// A scene file ready to be spawned one entity at a time.
#[derive(Debug)]
pub struct StagedScene {
    /// The records still to spawn and those spawned, parents first.
    records: Vec<EntityRecord>,
    /// Each record's fresh id, in `records`' order.
    fresh: Vec<SceneId>,
    /// File id to fresh id, for every record kept.
    renamed: HashMap<SceneId, SceneId>,
    /// Whether each record keeps its file's id rather than a fresh one.
    authored: bool,
    /// Fresh id to the entity it became, once spawned.
    spawned: HashMap<SceneId, Entity>,
    entities: Vec<Entity>,
    roots: Vec<Entity>,
}

impl StagedScene {
    /// `file` with its spawn order worked out, or the version error that
    /// makes it unusable. Needs no world: safe on a worker.
    pub fn new(file: SceneFile) -> Result<Self, SceneError> {
        // An older file is renamed as a prefab's is: one path for both.
        let file = crate::format::migrate::upgrade(file)?;
        let mut staged = Self {
            records: file.entities,
            fresh: Vec::new(),
            renamed: HashMap::new(),
            authored: false,
            spawned: HashMap::new(),
            entities: Vec::new(),
            roots: Vec::new(),
        };
        let _ = staged.reorder(|_| false);
        Ok(staged)
    }

    /// Reads and parses the scene at `path` under `root`, through the
    /// asset reads (the pack, in a browser).
    pub fn read(root: &Path, path: &AssetPath) -> Result<Self, SceneError> {
        let text = voltra_assets::fs::read_to_string(&root.join(path.as_str()))?;
        Self::new(crate::format::parse(&text)?)
    }

    /// Leaves out every record `skip` picks, and everything under it in the
    /// file — a client leaving the server's replicated entities to the
    /// server. Only before the first spawn; after it, nothing changes.
    pub fn skip(&mut self, skip: impl Fn(&EntityRecord) -> bool) {
        self.take(skip);
    }

    /// [`skip`](Self::skip), answering what it left out, parents first —
    /// a replication client keeps the level's replicated entities aside, to
    /// dress the server's copies in when they arrive.
    pub fn take(&mut self, take: impl Fn(&EntityRecord) -> bool) -> Vec<EntityRecord> {
        if !self.entities.is_empty() {
            log::warn!("a staged scene can only leave records out before it starts spawning");
            return Vec::new();
        }
        self.reorder(take)
    }

    /// Spawns every entity under its file's id rather than a fresh one
    /// ([`spawn_scene_as_authored`](super::spawn_scene_as_authored)'s rule):
    /// the level of a networked session, which both ends must hold under
    /// one id per entity. Only before the first spawn.
    pub fn keep_ids(&mut self) {
        if !self.entities.is_empty() {
            log::warn!("a staged scene can only keep its ids before it starts spawning");
            return;
        }
        self.authored = true;
        self.name_records();
    }

    /// Reads every registered component of every record as its type, the
    /// way spawning will, and returns the first that will not — so a load
    /// can find a broken file on a worker, before it has taken anything
    /// out of the world, and stay all-or-nothing though it spawns over
    /// frames. Needs no world. Unregistered components are kept unread by
    /// a spawn, and pass.
    pub fn check(&self, registry: &ComponentRegistry) -> Result<(), SceneError> {
        for record in &self.records {
            for (name, value) in &record.components {
                if let Some(Err(error)) = registry.normalize(name, value) {
                    return Err(error);
                }
            }
        }
        Ok(())
    }

    /// How many entities the whole spawn makes.
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Whether the scene holds no entity at all.
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// How many have been spawned so far.
    pub fn spawned(&self) -> usize {
        self.entities.len()
    }

    /// Whether every entity is in the world.
    pub fn is_done(&self) -> bool {
        self.entities.len() == self.records.len()
    }

    /// Every entity spawned so far, parents before children.
    pub fn entities(&self) -> &[Entity] {
        &self.entities
    }

    /// The spawned entities with no parent inside the scene.
    pub fn roots(&self) -> &[Entity] {
        &self.roots
    }

    /// The records still to spawn and those spawned, in spawn order.
    pub fn records(&self) -> &[EntityRecord] {
        &self.records
    }

    /// Spawns the next entity, complete: components read, parent resolved,
    /// references remapped. `None` once every entity is in.
    ///
    /// A component that will not read despawns that entity and returns the
    /// error; the ones spawned before it stay, for the caller to
    /// [`despawn`](Self::despawn) — the load is all-or-nothing, as
    /// `spawn_scene`'s is, but only the caller knows when it gives up.
    pub fn spawn_next(
        &mut self,
        world: &mut World,
        registry: &ComponentRegistry,
    ) -> Result<Option<Entity>, SceneError> {
        let index = self.entities.len();
        let Some(record) = self.records.get(index) else {
            return Ok(None);
        };
        let id = self.fresh[index];
        let entity = world.spawn();
        world.insert(entity, id);
        match crate::format::record::insert_components(world, registry, entity, &record.components)
        {
            Ok(unknown) if !unknown.0.is_empty() => {
                world.insert(entity, unknown);
            }
            Ok(_) => {}
            Err(error) => {
                world.despawn(entity);
                return Err(error);
            }
        }
        self.entities.push(entity);
        self.spawned.insert(id, entity);

        match world.get::<Parent>(entity).map(|link| link.id) {
            None => self.roots.push(entity),
            Some(link) => match self.renamed.get(&link).copied() {
                // Parents come first, so it is in — unless the file's links
                // loop, where one of the loop has to come before its parent
                // and keeps the link unresolved, drawn as a root, as a load
                // leaves a hand-edited cycle.
                Some(parent) => {
                    let link = match self.spawned.get(&parent) {
                        Some(&parent_entity) => Parent::resolved(parent, parent_entity),
                        None => Parent::new(parent),
                    };
                    world.insert(entity, link);
                }
                None => {
                    log::debug!(
                        "scene entity names a parent outside the scene; spawning it as a root"
                    );
                    world.remove::<Parent>(entity);
                    self.roots.push(entity);
                }
            },
        }
        let renamed = &self.renamed;
        registry.remap_refs(world, entity, &|id| renamed.get(&id).copied());
        Ok(Some(entity))
    }

    /// Despawns every entity spawned so far, children first: a load given
    /// up on, or a region unloaded before it finished.
    pub fn despawn(&mut self, world: &mut World) {
        for entity in self.entities.drain(..).rev() {
            world.despawn(entity);
        }
        self.roots.clear();
        self.spawned.clear();
    }

    /// Puts the records kept in parent-first order and names each. `skip`
    /// drops a record and its subtree; answers what it dropped, parents
    /// first.
    fn reorder(&mut self, skip: impl Fn(&EntityRecord) -> bool) -> Vec<EntityRecord> {
        let records = std::mem::take(&mut self.records);
        // The first record under a duplicated id is the one a link names,
        // as it is in a load.
        let mut index_of: HashMap<SceneId, usize> = HashMap::with_capacity(records.len());
        for (index, record) in records.iter().enumerate() {
            index_of.entry(record.id).or_insert(index);
        }
        let parent: Vec<Option<usize>> = records
            .iter()
            .map(|record| {
                let link = record.components.get(PARENT)?;
                let link: Parent = link.into_rust().ok()?;
                index_of.get(&link.id).copied()
            })
            .collect();

        let order = parents_first(&parent);
        let mut dropped = vec![false; records.len()];
        for &index in &order {
            let parent_dropped = parent[index].is_some_and(|p| dropped[p]);
            dropped[index] = parent_dropped || skip(&records[index]);
        }

        let mut slots: Vec<Option<EntityRecord>> = records.into_iter().map(Some).collect();
        let (kept, left_out): (Vec<usize>, Vec<usize>) =
            order.into_iter().partition(|&index| !dropped[index]);
        self.records = kept
            .into_iter()
            .filter_map(|index| slots[index].take())
            .collect();
        self.name_records();
        left_out
            .into_iter()
            .filter_map(|index| slots[index].take())
            .collect()
    }

    /// Gives each record the id it spawns under — a fresh one, or its own
    /// when [`keep_ids`](Self::keep_ids) asked — and maps the file's ids to
    /// them for the links.
    fn name_records(&mut self) {
        self.fresh = self
            .records
            .iter()
            .map(|record| {
                if self.authored {
                    record.id
                } else {
                    SceneId::new()
                }
            })
            .collect();
        self.renamed = HashMap::with_capacity(self.records.len());
        for (record, fresh) in self.records.iter().zip(&self.fresh) {
            self.renamed.entry(record.id).or_insert(*fresh);
        }
    }
}

#[cfg(test)]
mod tests;
