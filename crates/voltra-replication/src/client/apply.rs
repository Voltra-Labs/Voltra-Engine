//! A schema and a snapshot taken in: the server's component names mapped
//! to this build's, then spawns, despawns and values applied to the world.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use voltra_ecs::{Entity, World};
use voltra_net::PeerId;
use voltra_scene::{hierarchy, ComponentRegistry, Prefabs, Replicated};

use super::samples::Samples;
use super::stash::KeepIds;
use super::{spawn, Bound, Correction, ReplicationClient, Schema};
use crate::wire::{Snapshot, MAX_LISTED};
use crate::Timeline;

impl ReplicationClient {
    pub(super) fn take_schema(
        &mut self,
        registry: &ComponentRegistry,
        names: &[String],
        step: f32,
        interval: u32,
        interest: bool,
    ) {
        let ours = registry.replicated();
        let local = names
            .iter()
            .map(|name| {
                let found = ours.iter().copied().find(|c| c.name == name);
                if found.is_none() {
                    log::error!(
                        "the server replicates {name}, which this build does not register \
                         as replicated; its values are ignored"
                    );
                }
                found
            })
            .collect();
        self.schema = Some(Schema {
            local,
            interest,
            replicated: ours.iter().map(|c| c.name).collect(),
        });
        if step.is_finite() && step > 0.0 {
            self.timeline = Timeline::new(step, interval, self.config.min_delay_snapshots);
        }
    }

    pub(super) fn apply(
        &mut self,
        snapshot: Snapshot,
        world: &mut World,
        registry: &ComponentRegistry,
        mut prefabs: Option<&mut Prefabs>,
        now: Duration,
        made: &mut Vec<Entity>,
    ) {
        let Some(schema) = self.schema.as_ref() else {
            // Not acknowledged, so the server sends it all again.
            return;
        };
        let tick = snapshot.tick;
        if tick == 0 || (self.received.newest != 0 && tick <= self.received.newest) {
            return;
        }
        self.received.insert(tick);
        self.stats.snapshots += 1;
        self.stats.newest_tick = tick;
        self.input_ack = self.input_ack.max(snapshot.input_ack);
        while self
            .inputs
            .front()
            .is_some_and(|(t, _, _)| *t <= self.input_ack)
        {
            self.inputs.pop_front();
        }

        // Out of interest first: set aside as they are, before a despawn
        // in the same snapshot takes a parent, and before a spawn brings
        // one back.
        // What this snapshot culls, as entities: a replicated child the
        // server did not cull stays when its parent goes.
        let culled: HashSet<Entity> = snapshot
            .culled
            .iter()
            .filter_map(|id| self.bound.get(id).map(|b| b.entity))
            .collect();
        for id in snapshot.culled {
            let Some(bound) = self.bound.remove(&id) else {
                continue;
            };
            if self.predicted == Some(id) {
                self.predicted = None;
                self.correction = None;
            }
            // A child of one culled before it went with its parent, and
            // was set aside then.
            if world.is_alive(bound.entity) {
                let by_entity = &self.by_entity;
                let ids = KeepIds {
                    net_id: &|e| by_entity.get(&e).copied(),
                    stays: &|e| by_entity.contains_key(&e) && !culled.contains(&e),
                };
                self.stash
                    .keep(world, registry, &schema.replicated, bound.entity, &ids);
            }
            self.by_entity.remove(&bound.entity);
        }

        for id in snapshot.despawned {
            self.stash.forget(id);
            if let Some(bound) = self.bound.remove(&id) {
                self.by_entity.remove(&bound.entity);
                if world.is_alive(bound.entity) {
                    hierarchy::despawn_recursive(world, bound.entity);
                }
                if self.predicted == Some(id) {
                    self.predicted = None;
                    self.correction = None;
                }
            }
        }

        let components = schema.local.len();
        let mut scenes = None;
        let mut restored = false;
        let mut updated = HashMap::with_capacity(snapshot.updates.len());
        for update in snapshot.updates {
            let alive = self
                .bound
                .get(&update.id)
                .is_some_and(|b| world.is_alive(b.entity));
            if !alive {
                if let Some(gone) = self.bound.remove(&update.id) {
                    self.by_entity.remove(&gone.entity);
                }
                let Some(spawn) = update.spawn.as_ref() else {
                    // Spawned before, gone here: the game despawned it.
                    if self.resync.len() < MAX_LISTED && !self.resync.contains(&update.id) {
                        self.resync.push(update.id);
                    }
                    continue;
                };
                let by_entity = &self.by_entity;
                let entity = spawn::create(
                    world,
                    registry,
                    prefabs.as_deref_mut(),
                    spawn,
                    &mut scenes,
                    &mut spawn::Sources {
                        bound: &|e| by_entity.contains_key(&e),
                        stash: &mut self.stash,
                        restored: &mut restored,
                    },
                    made,
                );
                self.by_entity.insert(entity, update.id);
                self.bound.insert(
                    update.id,
                    Bound {
                        entity,
                        owner: spawn.owner.map(PeerId),
                        mirror: vec![None; components],
                        samples: vec![Samples::default(); components],
                    },
                );
            }
            let bound = self
                .bound
                .get_mut(&update.id)
                .expect("bound above, or already");
            if let Some(spawn) = &update.spawn {
                bound.owner = spawn.owner.map(PeerId);
                spawn::set_owner(world, bound.entity, spawn.owner);
            }
            let predicted = self.predicted == Some(update.id);
            for (index, value) in update.components {
                let index = usize::from(index);
                let Some(Some(component)) = schema.local.get(index).copied() else {
                    continue;
                };
                let value: Option<Arc<[u8]>> = value.map(Arc::from);
                let had = bound.mirror[index].is_some();
                bound.mirror[index] = value.clone();
                match value {
                    None => {
                        registry.remove_replicated(component, world, bound.entity);
                        bound.samples[index].clear();
                    }
                    // Predicted: its interpolated components (where it is)
                    // are the client's own, corrected through prediction;
                    // the rest — a score, a speed — arrive as they come, and
                    // one it has never had is put on at once.
                    Some(_) if predicted && had && component.interpolated => {}
                    Some(value) => {
                        if component.interpolated && !predicted {
                            bound.samples[index].push(tick, value.clone());
                            if had {
                                continue;
                            }
                        }
                        if let Err(error) =
                            registry.apply_replicated(component, world, bound.entity, &value)
                        {
                            log::warn!("{}: {error}", component.name);
                        }
                    }
                }
            }
            updated.insert(update.id, ());
            if predicted {
                self.correction = Some(Correction {
                    input_ack: snapshot.input_ack,
                    tick,
                    state: bound
                        .mirror
                        .iter()
                        .enumerate()
                        .filter_map(|(i, v)| Some((schema.local[i]?, v.clone()?)))
                        .collect(),
                });
            }
        }

        if restored {
            // What came back hangs under its parents again — and a child
            // that came back before its parent finds it now.
            hierarchy::resolve_parents(world);
        }

        if snapshot.complete {
            // Everything left out is unchanged at this tick: a sample, so
            // a remote entity resting and then moving moves from where it
            // rested, not from where it was last sent.
            for (id, bound) in &mut self.bound {
                if !updated.contains_key(id) && self.predicted != Some(*id) {
                    for samples in &mut bound.samples {
                        samples.hold(tick);
                    }
                }
            }
            // And, once, a replicated entity the server never sent is not
            // the server's: the level's copy of one it despawned before this
            // client joined. Once only — a replicated entity spawned here
            // later races the server's spawn of the same, and is the game's.
            if !self.swept {
                self.swept = true;
                let orphans: Vec<Entity> = world
                    .query::<Replicated>()
                    .map(|(entity, _)| entity)
                    .filter(|entity| !self.by_entity.contains_key(entity))
                    .collect();
                for entity in orphans {
                    if !world.is_alive(entity) {
                        continue;
                    }
                    if schema.interest {
                        // Under interest, never sent may be only far
                        // away: set aside, to come back as the level had
                        // it when the server sends it.
                        let ids = KeepIds {
                            net_id: &|_| None,
                            stays: &|e| self.by_entity.contains_key(&e),
                        };
                        self.stash
                            .keep(world, registry, &schema.replicated, entity, &ids);
                    } else {
                        hierarchy::despawn_recursive(world, entity);
                    }
                }
            }
        }
        self.timeline.arrived(tick, now);
    }
}
