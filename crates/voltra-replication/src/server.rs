//! The server's half: the world read each snapshot, a view per client of
//! what it has, and the snapshots that close the gap.
//!
//! Server authority: the server's world is the truth, and a client's only
//! say is its inputs. Nothing a client sends changes a replicated
//! component on the server — the game reads the inputs and moves the
//! entity itself.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use glam::Vec2;
use voltra_ecs::{Entity, World};
use voltra_net::{Endpoint, NetEvent, PeerId};
use voltra_scene::{ComponentRegistry, ReplicatedComponent};

use crate::acks::ReceivedTicks;
use crate::wire::{self, ClientMessage, Control, NetId, ServerTick, MAX_LISTED};
use crate::{Interest, InterestGrid, ReplicationConfig};

mod ask;
mod audience;
mod inputs;
mod interest_points;
mod lag;
mod refresh;
mod relevance;
mod snapshot;
mod view;

use snapshot::Tracked;
use view::ClientView;

/// The most interest points the game may set for one client
/// ([`ReplicationServer::set_interest_points`]): each is a query of the
/// grid every snapshot, so a few cameras or sights, not a path of them.
pub const MAX_INTEREST_POINTS: usize = 16;

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
    /// Every connected client, replicated to or not.
    connected: BTreeSet<PeerId>,
    /// The clients replicated to, when the game names them
    /// ([`set_audience`](Self::set_audience)); `None` is every connected one.
    audience: Option<BTreeSet<PeerId>>,
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
            connected: BTreeSet::new(),
            audience: None,
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

    /// A server tick's length, in seconds.
    pub fn step(&self) -> f32 {
        self.step
    }

    /// The clients it replicates to: every connected one, or those of
    /// them in its [`audience`](Self::set_audience).
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

    /// Joins and leaves, acknowledgements and inputs: what the endpoint's
    /// receive brought. Takes the messages on replication's channels out
    /// of the endpoint, so the game never sees them.
    pub fn receive(&mut self, endpoint: &mut Endpoint) {
        for event in endpoint.events() {
            match *event {
                NetEvent::Connected(peer) if peer != PeerId::SERVER => {
                    self.connected.insert(peer);
                    if self.admits(peer) {
                        self.clients.insert(peer, ClientView::default());
                    }
                }
                NetEvent::Disconnected { peer, .. } => {
                    self.connected.remove(&peer);
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
                self.tick,
            );
            ask::answer(
                view,
                update.resync,
                &self.tracked,
                self.components.len(),
                self.tick,
            );
        }
    }

    /// The start of a fixed step: the tick advances, and each client's next
    /// input becomes this step's ([`input`](Self::input)).
    pub fn begin_step(&mut self) {
        self.tick = self.tick.wrapping_add(1).max(1);
        for view in self.clients.values_mut() {
            view.inputs.next(self.tick);
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
