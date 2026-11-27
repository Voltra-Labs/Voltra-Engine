//! The client's half: snapshots applied to the world, remote entities
//! drawn between them, the owned one handed to prediction.
//!
//! Three kinds of replicated entity, by who moves them on screen:
//!
//! - **remote**, interpolated components: each value is a sample at its
//!   server tick, and [`interpolate`](ReplicationClient::interpolate)
//!   draws the [`Timeline`]'s moment between two samples;
//! - **remote**, other components: applied as they arrive;
//! - **predicted** (the entity this client owns, once a
//!   [`Prediction`](crate::Prediction) claims it): the server's values of
//!   its interpolated components are kept as a [`Correction`] for
//!   prediction to rewind to, and the world's copy is the client's own; its
//!   other components are applied as they arrive.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::Duration;

use voltra_ecs::{Entity, World};
use voltra_net::{Endpoint, NetEvent, PeerId};
use voltra_scene::{ComponentRegistry, Prefabs, ReplicatedComponent};

use crate::acks::ReceivedTicks;
use crate::wire::{self, ClientMessage, Control, InputTick, NetId, ServerTick, Snapshot};
use crate::{ReplicationConfig, Timeline};

mod apply;
mod samples;
mod spawn;

use samples::{Pick, Samples};

/// The most inputs kept waiting for the server's acknowledgement.
const MAX_PENDING_INPUTS: usize = 256;

/// The server's state of the predicted entity at one snapshot.
#[derive(Debug, Clone)]
pub struct Correction {
    /// The newest of this client's inputs the state includes: the inputs
    /// after it are to be replayed on top.
    pub input_ack: InputTick,
    /// The snapshot it came in.
    pub tick: ServerTick,
    /// Every replicated component the server says the entity has, encoded.
    pub state: Vec<(ReplicatedComponent, Arc<[u8]>)>,
}

/// What a client's replication has seen, for a stats overlay or a test.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ReplicaStats {
    /// Snapshots applied.
    pub snapshots: u64,
    /// Their bytes.
    pub bytes: u64,
    /// The newest snapshot's server tick.
    pub newest_tick: ServerTick,
    /// Replicated entities it has.
    pub entities: usize,
    /// How far behind the newest snapshot remote entities are drawn.
    pub delay: Duration,
    /// The measured jitter the delay is sized from.
    pub jitter: Duration,
}

struct Schema {
    /// By server index: this build's handle for the name, if it has one.
    local: Vec<Option<ReplicatedComponent>>,
}

struct Bound {
    entity: Entity,
    owner: Option<PeerId>,
    /// By server index: the server's newest value.
    mirror: Vec<Option<Arc<[u8]>>>,
    /// By server index: interpolation samples.
    samples: Vec<Samples>,
}

/// Replication on a client: call [`receive`](Self::receive) after the
/// endpoint's receive, [`interpolate`](Self::interpolate) once a frame
/// before drawing, and [`send`](Self::send) before the endpoint's flush.
pub struct ReplicationClient {
    config: ReplicationConfig,
    schema: Option<Schema>,
    timeline: Timeline,
    bound: HashMap<NetId, Bound>,
    by_entity: HashMap<Entity, NetId>,
    received: ReceivedTicks,
    input_ack: InputTick,
    inputs: VecDeque<(InputTick, Vec<u8>)>,
    resync: Vec<NetId>,
    predicted: Option<NetId>,
    correction: Option<Correction>,
    local: Option<PeerId>,
    /// Whether the first complete snapshot has swept away the replicated
    /// entities the server never had.
    swept: bool,
    stats: ReplicaStats,
}

impl ReplicationClient {
    /// A client replicating with `config`; it waits for the server's schema
    /// before it applies anything.
    pub fn new(config: ReplicationConfig) -> Self {
        let timeline = Timeline::new(1.0 / 60.0, 1, config.min_delay_snapshots);
        Self {
            config,
            schema: None,
            timeline,
            bound: HashMap::new(),
            by_entity: HashMap::new(),
            received: ReceivedTicks::default(),
            input_ack: 0,
            inputs: VecDeque::new(),
            resync: Vec::new(),
            predicted: None,
            correction: None,
            local: None,
            swept: false,
            stats: ReplicaStats::default(),
        }
    }

    /// What it has seen.
    pub fn stats(&self) -> ReplicaStats {
        ReplicaStats {
            entities: self.bound.len(),
            delay: self.timeline.delay(),
            jitter: self.timeline.jitter(),
            ..self.stats
        }
    }

    /// The interpolation clock.
    pub fn timeline(&self) -> &Timeline {
        &self.timeline
    }

    /// The entity the server sent as `id`.
    pub fn entity(&self, id: NetId) -> Option<Entity> {
        self.bound.get(&id).map(|b| b.entity)
    }

    /// The number the server sends `entity` by.
    pub fn net_id(&self, entity: Entity) -> Option<NetId> {
        self.by_entity.get(&entity).copied()
    }

    /// This client's own entity: the first the server says it owns.
    pub fn owned(&self) -> Option<(NetId, Entity)> {
        let local = self.local?;
        self.bound
            .iter()
            .filter(|(_, b)| b.owner == Some(local))
            .map(|(&id, b)| (id, b.entity))
            .min_by_key(|(id, _)| *id)
    }

    /// Makes `id` the predicted entity: its server state is kept as a
    /// [`Correction`] instead of being drawn. `None` stops predicting.
    pub fn set_predicted(&mut self, id: Option<NetId>) {
        if self.predicted != id {
            self.correction = None;
        }
        self.predicted = id;
    }

    /// The predicted entity's newest server state, once each.
    pub fn take_correction(&mut self) -> Option<Correction> {
        self.correction.take()
    }

    /// The newest of this client's inputs the server has simulated.
    pub fn input_ack(&self) -> InputTick {
        self.input_ack
    }

    /// Queues an input for the server, sent with the next few
    /// [`send`](Self::send)s until acknowledged.
    pub fn queue_input(&mut self, tick: InputTick, bytes: Vec<u8>) {
        if bytes.len() > self.config.max_input_size {
            log::warn!(
                "an input of {} bytes is over the {}-byte limit; the server would drop it",
                bytes.len(),
                self.config.max_input_size
            );
            return;
        }
        self.inputs.push_back((tick, bytes));
        while self.inputs.len() > MAX_PENDING_INPUTS {
            self.inputs.pop_front();
        }
    }

    fn forget(&mut self) {
        self.schema = None;
        self.bound.clear();
        self.by_entity.clear();
        self.received = ReceivedTicks::default();
        self.input_ack = 0;
        self.inputs.clear();
        self.resync.clear();
        self.predicted = None;
        self.correction = None;
        self.swept = false;
    }

    /// Applies what the endpoint's receive brought: the schema, then each
    /// snapshot in order. Answers the entities it made, whose assets the
    /// loop resolves. `prefabs` is where a spawn's prefab is read from;
    /// without it, spawned entities are bare.
    pub fn receive(
        &mut self,
        endpoint: &mut Endpoint,
        world: &mut World,
        registry: &ComponentRegistry,
        mut prefabs: Option<&mut Prefabs>,
        now: Duration,
    ) -> Vec<Entity> {
        for event in endpoint.events() {
            match event {
                // A new session: what the last one bound means nothing now.
                // Its entities stay — the game decides what to do with them.
                NetEvent::Connected(_) | NetEvent::Disconnected { .. } => self.forget(),
            }
        }
        self.local = endpoint.local_id();
        let channels = self.config.channels;
        let messages =
            endpoint.take_messages_on(&[channels.state, channels.control, channels.input]);
        let mut made = Vec::new();
        for message in messages {
            if message.from != PeerId::SERVER {
                continue;
            }
            if message.channel == channels.control {
                match message.decode::<Control>() {
                    Ok(Control::Schema {
                        components,
                        step,
                        snapshot_interval,
                    }) => self.take_schema(registry, &components, step, snapshot_interval),
                    Err(error) => log::warn!("an unreadable replication message: {error}"),
                }
            } else if message.channel == channels.state {
                match message.decode::<Snapshot>() {
                    Ok(snapshot) => {
                        self.stats.bytes += message.bytes.len() as u64;
                        self.apply(
                            snapshot,
                            world,
                            registry,
                            prefabs.as_deref_mut(),
                            now,
                            &mut made,
                        );
                    }
                    Err(error) => log::warn!("an unreadable snapshot: {error}"),
                }
            }
        }
        made
    }

    /// Draws every remote entity's interpolated components at the
    /// timeline's moment, which this moves on by `delta`.
    pub fn interpolate(
        &mut self,
        world: &mut World,
        registry: &ComponentRegistry,
        now: Duration,
        delta: f32,
    ) {
        let Some(schema) = self.schema.as_ref() else {
            return;
        };
        let Some(drawn) = self.timeline.advance(now, delta) else {
            return;
        };
        for (id, bound) in &mut self.bound {
            if self.predicted == Some(*id) || !world.is_alive(bound.entity) {
                continue;
            }
            for (index, samples) in bound.samples.iter_mut().enumerate() {
                if samples.is_empty() {
                    continue;
                }
                let Some(Some(component)) = schema.local.get(index).copied() else {
                    continue;
                };
                let value = match samples.at(drawn) {
                    Some(Pick::Hold(value)) => Ok(value.to_vec()),
                    Some(Pick::Between(from, to, t)) => {
                        match registry.lerp_replicated(component, from, to, t) {
                            Some(result) => result,
                            None => Ok(to.to_vec()),
                        }
                    }
                    None => continue,
                };
                let applied = value.and_then(|bytes| {
                    registry.apply_replicated(component, world, bound.entity, &bytes)
                });
                if let Err(error) = applied {
                    log::warn!("{}: {error}", component.name);
                }
            }
        }
    }

    /// Tells the server which snapshots arrived, and sends the newest
    /// unacknowledged inputs and any entity to send whole again.
    pub fn send(&mut self, endpoint: &mut Endpoint) {
        if !endpoint.is_connected(PeerId::SERVER) {
            return;
        }
        let skip = self
            .inputs
            .len()
            .saturating_sub(self.config.input_redundancy.max(1));
        let message = ClientMessage {
            newest: self.received.newest,
            mask: self.received.mask,
            inputs: self.inputs.iter().skip(skip).cloned().collect(),
            resync: std::mem::take(&mut self.resync),
        };
        if let Err(error) = endpoint.send(
            PeerId::SERVER,
            self.config.channels.input,
            &wire::encode(&message),
        ) {
            log::warn!("replication acks not sent: {error}");
        }
    }
}
