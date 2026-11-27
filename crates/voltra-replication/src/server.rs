//! The server's half: the world read each snapshot, a view per client of
//! what it has, and the snapshots that close the gap.
//!
//! Server authority: the server's world is the truth, and a client's only
//! say is its inputs. Nothing a client sends changes a replicated
//! component on the server — the game reads the inputs and moves the
//! entity itself.

use std::collections::{BTreeMap, HashMap};

use glam::Vec2;
use voltra_ecs::{Entity, World};
use voltra_net::{Endpoint, NetEvent, PeerId};
use voltra_scene::{ComponentRegistry, ReplicatedComponent};

use crate::acks::ReceivedTicks;
use crate::wire::{self, ClientMessage, Control, NetId, ServerTick, MAX_LISTED};
use crate::{Interest, InterestGrid, ReplicationConfig};

mod inputs;
mod refresh;
mod relevance;
mod snapshot;
mod view;

use snapshot::Tracked;
use view::ClientView;

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
    /// The replicated entities relevant to it at the last snapshot: all of
    /// them without interest management.
    pub relevant: usize,
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
    /// Interest management, when on: its values checked, and the grid it
    /// rebuilds each snapshot.
    interest: Option<(Interest, InterestGrid)>,
    /// The points the game watches for each client, beside its entities.
    points: HashMap<PeerId, Vec<Vec2>>,
}

impl ReplicationServer {
    /// A server replicating what `registry` marks replicated, at a tick of
    /// `step` seconds.
    pub fn new(config: ReplicationConfig, registry: &ComponentRegistry, step: f32) -> Self {
        let interest = config
            .interest
            .and_then(Interest::checked)
            .map(|interest| (interest, InterestGrid::new(interest)));
        if let Some((interest, _)) = &interest {
            log::info!(
                "replication: each client sent what is within {} units of its interest \
                 (cells of {}, leaving at {})",
                interest.radius,
                interest.cell_size,
                interest.leave_radius(interest.radius)
            );
        }
        Self {
            config,
            interest,
            points: HashMap::new(),
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
            relevant: match self.interest {
                Some(_) => view.interest.relevant.len(),
                None => self.tracked.len(),
            },
        })
    }

    /// The interest it culls by: the config's, once checked; `None` when
    /// every client is sent everything.
    pub fn interest(&self) -> Option<&Interest> {
        self.interest.as_ref().map(|(interest, _)| interest)
    }

    /// Sets the points `peer` is sent the world around, besides the
    /// entities it owns — a spectator's camera, a scout's sight, a level's
    /// area of play. Replaces the last it was given; kept until replaced or
    /// the peer leaves. Without interest management it changes nothing.
    pub fn set_interest_points(&mut self, peer: PeerId, points: Vec<Vec2>) {
        if points.is_empty() {
            self.points.remove(&peer);
        } else {
            self.points.insert(peer, points);
        }
    }

    /// Every point `peer` was sent the world around at the last snapshot:
    /// where its entities were, then the game's. Empty without interest
    /// management.
    pub fn interest_points(&self, peer: PeerId) -> &[Vec2] {
        self.clients
            .get(&peer)
            .map_or(&[], |view| view.interest.points.as_slice())
    }

    /// Whether `entity` was relevant to `peer` at the last snapshot: it is
    /// being sent to it. Every replicated entity without interest
    /// management.
    pub fn is_relevant(&self, peer: PeerId, entity: Entity) -> bool {
        let (Some(view), Some(id)) = (self.clients.get(&peer), self.ids.get(&entity)) else {
            return false;
        };
        match self.interest {
            Some(_) => view.interest.relevant.contains(id),
            None => true,
        }
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
                    self.points.remove(&peer);
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
                view.resync(id, self.components.len(), self.tick.wrapping_add(1));
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
                interest: self.interest.is_some(),
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
        let culling = match &mut self.interest {
            Some((_, grid)) => {
                relevance::update(grid, &self.tracked, &mut self.clients, &self.points);
                true
            }
            None => false,
        };

        for (&peer, view) in &mut self.clients {
            if !view.schema_sent {
                continue;
            }
            let relevant = std::mem::take(&mut view.interest.relevant);
            let bytes = snapshot::build(
                view,
                &self.tracked,
                culling.then_some(relevant.as_slice()),
                self.tick,
                self.config.bytes_per_snapshot,
            );
            view.interest.relevant = relevant;
            while view.sent.len() > self.config.history.max(1) {
                view.sent.pop_front();
            }
            if let Err(error) = endpoint.send(peer, channels.state, &bytes) {
                log::warn!("{peer}: snapshot {} not sent: {error}", self.tick);
            }
        }
    }
}
