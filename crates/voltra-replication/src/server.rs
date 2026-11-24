//! The server's half: the world read each snapshot, a view per client of
//! what it has, and the snapshots that close the gap.
//!
//! Server authority: the server's world is the truth, and a client's only
//! say is its inputs. Nothing a client sends changes a replicated
//! component on the server — the game reads the inputs and moves the
//! entity itself.

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use voltra_ecs::{Entity, World};
use voltra_net::{Endpoint, NetEvent, PeerId};
use voltra_scene::{ComponentRegistry, PrefabInstance, Replicated, ReplicatedComponent, SceneId};

use crate::acks::ReceivedTicks;
use crate::wire::{self, ClientMessage, Control, NetId, ServerTick, Spawn, MAX_LISTED};
use crate::{NetOwner, ReplicationConfig};

mod inputs;
mod snapshot;
mod view;

use snapshot::Tracked;
use view::{ClientView, Value};

/// What the server knows of one client's replication, for a profiler, a
/// stats overlay or a test.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ClientReplicationStats {
    /// The bytes of the last snapshot sent it.
    pub snapshot_bytes: usize,
    /// Whether that snapshot held everything due, or the budget cut it.
    pub complete: bool,
    /// The newest of its inputs simulated.
    pub input_ack: u32,
    /// Its inputs dropped because too many were waiting.
    pub inputs_dropped: u64,
}

/// Replication on a server: call [`receive`](Self::receive) after the
/// endpoint's receive, [`begin_step`](Self::begin_step) before each fixed
/// step, and [`send`](Self::send) before the endpoint's flush.
pub struct ReplicationServer {
    config: ReplicationConfig,
    components: Vec<ReplicatedComponent>,
    step: f32,
    ids: HashMap<Entity, NetId>,
    tracked: BTreeMap<NetId, Tracked>,
    next_id: NetId,
    clients: BTreeMap<PeerId, ClientView>,
    tick: ServerTick,
    last_snapshot: ServerTick,
}

impl ReplicationServer {
    /// A server replicating what `registry` marks replicated, at a tick of
    /// `step` seconds.
    pub fn new(config: ReplicationConfig, registry: &ComponentRegistry, step: f32) -> Self {
        Self {
            config,
            components: registry.replicated(),
            step,
            ids: HashMap::new(),
            tracked: BTreeMap::new(),
            next_id: 1,
            clients: BTreeMap::new(),
            tick: 0,
            last_snapshot: 0,
        }
    }

    /// The config it runs with.
    pub fn config(&self) -> &ReplicationConfig {
        &self.config
    }

    /// The server tick: the fixed steps simulated so far.
    pub fn tick(&self) -> ServerTick {
        self.tick
    }

    /// The clients it replicates to.
    pub fn clients(&self) -> impl Iterator<Item = PeerId> + '_ {
        self.clients.keys().copied()
    }

    /// The number `entity` goes by on the wire, once it has been sent.
    pub fn net_id(&self, entity: Entity) -> Option<NetId> {
        self.ids.get(&entity).copied()
    }

    /// What it last did for `peer`.
    pub fn stats(&self, peer: PeerId) -> Option<ClientReplicationStats> {
        self.clients.get(&peer).map(|view| ClientReplicationStats {
            snapshot_bytes: view.last_snapshot_bytes,
            complete: view.last_complete,
            input_ack: view.inputs.consumed,
            inputs_dropped: view.inputs.dropped,
        })
    }

    /// Joins and leaves, acknowledgements and inputs: what the endpoint's
    /// receive brought. Takes the messages on replication's channels out
    /// of the endpoint, so the game never sees them.
    pub fn receive(&mut self, endpoint: &mut Endpoint) {
        for event in endpoint.events() {
            match *event {
                NetEvent::Connected(peer) if peer != PeerId::SERVER => {
                    self.clients.insert(peer, ClientView::default());
                }
                NetEvent::Disconnected { peer, .. } => {
                    self.clients.remove(&peer);
                }
                NetEvent::Connected(_) => {}
            }
        }
        let channels = self.config.channels;
        let messages =
            endpoint.take_messages_on(&[channels.state, channels.control, channels.input]);
        for message in messages {
            if message.channel != channels.input {
                // Snapshots and schemas go one way; a client sending one is
                // confused or hostile, and either way it changes nothing.
                continue;
            }
            let Some(view) = self.clients.get_mut(&message.from) else {
                continue;
            };
            let Ok(update) = message.decode::<ClientMessage>() else {
                log::warn!("{}: an unreadable replication message", message.from);
                continue;
            };
            view.acknowledge(ReceivedTicks {
                newest: update.newest,
                mask: update.mask,
            });
            let inputs = update.inputs.into_iter().take(MAX_LISTED).collect();
            view.inputs.offer(
                inputs,
                self.config.max_input_size,
                self.config.max_buffered_inputs,
            );
            for id in update.resync.into_iter().take(MAX_LISTED) {
                view.resync(id, self.components.len());
            }
        }
    }

    /// The start of a fixed step: the tick advances, and each client's next
    /// input becomes this step's ([`input`](Self::input)).
    pub fn begin_step(&mut self) {
        self.tick = self.tick.wrapping_add(1).max(1);
        for view in self.clients.values_mut() {
            view.inputs.next();
        }
    }

    /// `peer`'s input for this step, encoded; `None` when none came in
    /// time — the step should leave its entity be.
    pub fn input(&self, peer: PeerId) -> Option<&[u8]> {
        self.clients
            .get(&peer)
            .and_then(|view| view.inputs.current.as_deref())
    }

    /// Sends each client its snapshot, if one is due: at most one per tick,
    /// every [`snapshot_interval`](ReplicationConfig::snapshot_interval)
    /// ticks. A new client is sent the schema first.
    ///
    /// `&mut World` only to give a replicated entity without a [`SceneId`]
    /// one: a client adopts by id, so every entity it is sent needs one.
    pub fn send(
        &mut self,
        world: &mut World,
        registry: &ComponentRegistry,
        endpoint: &mut Endpoint,
    ) {
        let channels = self.config.channels;
        for (&peer, view) in &mut self.clients {
            if view.schema_sent {
                continue;
            }
            let schema = Control::Schema {
                components: self.components.iter().map(|c| c.name.to_owned()).collect(),
                step: self.step,
                snapshot_interval: self.config.snapshot_interval.max(1),
            };
            match endpoint.send(peer, channels.control, &wire::encode(&schema)) {
                Ok(()) => view.schema_sent = true,
                Err(error) => log::warn!("{peer}: replication schema not sent: {error}"),
            }
        }

        let interval = self.config.snapshot_interval.max(1);
        if self.tick == 0
            || self.tick == self.last_snapshot
            || self.tick.wrapping_sub(self.last_snapshot) < interval
        {
            return;
        }
        self.last_snapshot = self.tick;
        self.refresh(world, registry);

        for (&peer, view) in &mut self.clients {
            if !view.schema_sent {
                continue;
            }
            let bytes = snapshot::build(
                view,
                &self.tracked,
                self.tick,
                self.config.bytes_per_snapshot,
            );
            while view.sent.len() > self.config.history.max(1) {
                view.sent.pop_front();
            }
            if let Err(error) = endpoint.send(peer, channels.state, &bytes) {
                log::warn!("{peer}: snapshot {} not sent: {error}", self.tick);
            }
        }
    }

    /// Reads the world: every replicated entity's spawn and components as
    /// they are now, and the entities gone since the last read.
    fn refresh(&mut self, world: &mut World, registry: &ComponentRegistry) {
        let live: Vec<(Entity, f32)> = world
            .query::<Replicated>()
            .map(|(entity, replicated)| (entity, replicated.priority))
            .collect();

        let mut seen = HashMap::with_capacity(live.len());
        for (entity, priority) in live {
            let id = match self.ids.get(&entity) {
                Some(&id) => id,
                None => {
                    let id = self.next_id;
                    self.next_id = self.next_id.wrapping_add(1).max(1);
                    self.ids.insert(entity, id);
                    id
                }
            };
            seen.insert(id, ());
            let scene_id = match world.get::<SceneId>(entity) {
                Some(&id) => id,
                None => {
                    let fresh = SceneId::new();
                    world.insert(entity, fresh);
                    fresh
                }
            };
            let spawn_value = Spawn {
                id: scene_id,
                prefab: world
                    .get::<PrefabInstance>(entity)
                    .map(|p| p.source.as_str().to_owned()),
                owner: world.get::<NetOwner>(entity).map(|o| o.0 .0),
            };
            let spawn_bytes = wire::encode(&spawn_value);
            let previous = self.tracked.get(&id);
            let spawn = match previous {
                Some(p) if *p.spawn == *spawn_bytes => p.spawn.clone(),
                _ => Arc::from(spawn_bytes),
            };
            let components: Vec<Value> = self
                .components
                .iter()
                .enumerate()
                .map(|(index, &component)| {
                    let bytes = match registry.encode_replicated(component, world, entity)? {
                        Ok(bytes) => bytes,
                        Err(error) => {
                            log::warn!("{} would not encode: {error}", component.name);
                            return None;
                        }
                    };
                    // The previous Arc when unchanged: views compare by value,
                    // and sharing keeps a still world from allocating.
                    match previous.and_then(|p| p.components[index].as_ref()) {
                        Some(old) if **old == *bytes => Some(old.clone()),
                        _ => Some(Arc::from(bytes)),
                    }
                })
                .collect();
            self.tracked.insert(
                id,
                Tracked {
                    entity,
                    spawn,
                    spawn_value,
                    components,
                    priority,
                },
            );
        }

        let gone: Vec<NetId> = self
            .tracked
            .keys()
            .copied()
            .filter(|id| !seen.contains_key(id))
            .collect();
        for id in gone {
            if let Some(tracked) = self.tracked.remove(&id) {
                self.ids.remove(&tracked.entity);
            }
            for view in self.clients.values_mut() {
                // Only a client that may have it needs telling.
                if let Some(entity) = view.entities.remove(&id) {
                    if entity.sent_spawn.is_some() {
                        view.despawned.insert(id);
                    }
                }
            }
        }
    }
}
