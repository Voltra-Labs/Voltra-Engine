//! Replication through `tick.net`: the loop drives it, the game turns it
//! on, reads its clients' inputs on the server and predicts its own entity
//! on a client.
//!
//! Where the loop calls in (`app.rs`, `simulation.rs`):
//!
//! - after the network's receive, before the ticks: acknowledgements and
//!   inputs on a server; on a client, snapshots applied — spawns,
//!   despawns, values — so the tick reads the server's world;
//! - before each fixed step: the server's tick advances and each client's
//!   next input is the step's ([`Net::input`]);
//! - after the steps, before the flush: a server's snapshots, built from
//!   the world as the frame left it; a client's remote entities drawn at
//!   the interpolation clock's moment, and its acknowledgements and inputs.

use serde::de::DeserializeOwned;
use serde::Serialize;
use voltra_ecs::{Entity, World};
use voltra_net::{NetConfig, PeerId};
use voltra_replication::{
    Prediction, Replication, ReplicationClient, ReplicationConfig, ReplicationServer,
};

use super::tick_net::Net;
use super::{App, Tick};

impl Net {
    /// Turns replication on for this session and every later one: the
    /// server sends the replicated components of its [`Replicated`]
    /// entities, a client applies them. Both ends must turn it on before
    /// they host or join — it adds channels to the session's config
    /// ([`ReplicationConfig::net_config`]), which the handshake checks.
    ///
    /// [`Replicated`]: voltra_scene::Replicated
    pub fn enable_replication(&mut self, config: ReplicationConfig) {
        self.replication_config = Some(config);
        self.replication = None;
        self.replication_refused = false;
    }

    /// Whether replication is on.
    pub fn replicates(&self) -> bool {
        self.replication_config.is_some()
    }

    /// `config` with replication's channels, when it is on, and the
    /// lobby's: the config a session is made with.
    pub(super) fn session_config(&self, config: NetConfig) -> NetConfig {
        let config = match &self.replication_config {
            Some(replication) => replication.net_config(config),
            None => config,
        };
        self.lobby_session_config(config)
    }

    /// This session's replication, once it has started.
    pub fn replication(&self) -> Option<&Replication> {
        self.replication.as_ref()
    }

    /// This session's replication, mutably.
    pub fn replication_mut(&mut self) -> Option<&mut Replication> {
        self.replication.as_mut()
    }

    /// `peer`'s input for this fixed step, on a server — `None` when none
    /// came in time (leave its entity be: the client predicted no step),
    /// when it will not decode as `I`, or off a server. Read it in the
    /// fixed tick, once a step.
    pub fn input<I: DeserializeOwned>(&self, peer: PeerId) -> Option<I> {
        let bytes = self.replication.as_ref()?.server()?.input(peer)?;
        match voltra_net::decode(bytes) {
            Ok(input) => Some(input),
            Err(error) => {
                log::warn!("{peer}'s input would not decode: {error}");
                None
            }
        }
    }

    /// Sets the points `peer` is sent the world around on a server, besides
    /// the entities it owns: a spectator's camera, say. Replaces the last
    /// it was given, until the peer leaves; at most
    /// [`MAX_INTEREST_POINTS`](voltra_replication::MAX_INTEREST_POINTS).
    /// `false` off a replicating server or for a peer not joined to it.
    /// Changes nothing without
    /// [`ReplicationConfig::interest`](voltra_replication::ReplicationConfig::interest).
    /// Each point is a `Vec2` or a `Vec3`
    /// ([`InterestPoint`](voltra_replication::InterestPoint)) and sees only
    /// entities of its own dimension.
    pub fn set_interest_points<P: Into<voltra_replication::InterestPoint>>(
        &mut self,
        peer: PeerId,
        points: impl IntoIterator<Item = P>,
    ) -> bool {
        match self.replication.as_mut().and_then(Replication::server_mut) {
            Some(server) => server.set_interest_points(peer, points),
            None => false,
        }
    }

    /// The entity `peer` owns on a server: the first [`Replicated`] one
    /// carrying its [`NetOwner`](voltra_replication::NetOwner).
    ///
    /// [`Replicated`]: voltra_scene::Replicated
    pub fn owned_by(world: &World, peer: PeerId) -> Option<Entity> {
        world
            .query::<voltra_replication::NetOwner>()
            .filter(|(_, owner)| owner.0 == peer)
            .map(|(entity, _)| entity)
            .min_by_key(|entity| entity.index())
    }

    pub(super) fn begin_fixed_step(&mut self) {
        if let Some(server) = self.replication.as_mut().and_then(Replication::server_mut) {
            server.begin_step();
        }
    }

    /// The session's replication, made the first time it is asked for and
    /// the session can carry it.
    fn ensure_replication(&mut self, registry: &voltra_scene::ComponentRegistry, step: f32) {
        let (Some(config), Some(endpoint)) = (&self.replication_config, &self.endpoint) else {
            return;
        };
        if self.replication.is_some() || self.replication_refused {
            return;
        }
        if !config.channels.fit(endpoint.config()) {
            log::error!(
                "replication is on, but this session was made without its channels \
                 (ReplicationConfig::net_config); nothing is replicated"
            );
            self.replication_refused = true;
            return;
        }
        self.lag = None;
        self.replication = Some(if endpoint.is_server() {
            self.lag = config.lag_compensation.map(|lag| {
                if lag.max_rewind > lag.history {
                    log::warn!(
                        "lag compensation keeps {:?} of history; a rewind is bounded by it, \
                         not by max_rewind {:?}",
                        lag.history,
                        lag.max_rewind
                    );
                }
                voltra_physics::PoseHistory::covering(lag.history, step)
            });
            Replication::Server(Box::new(ReplicationServer::new(
                config.clone(),
                registry,
                step,
            )))
        } else {
            Replication::Client(Box::new(ReplicationClient::new(config.clone())))
        });
    }
}

impl App {
    /// Turns replication on before the first frame; see
    /// [`Net::enable_replication`].
    pub fn with_replication(mut self, config: ReplicationConfig) -> Self {
        self.net.enable_replication(config);
        self
    }

    pub(super) fn receive_replication(&mut self) {
        let _replication = voltra_profile::scope("replication");
        let step = self.physics_world.step();
        self.net.ensure_replication(&self.registry, step);
        let now = self.net.now();
        let (Some(replication), Some(endpoint)) =
            (self.net.replication.as_mut(), self.net.endpoint.as_mut())
        else {
            return;
        };
        match replication {
            Replication::Server(server) => {
                server.receive(endpoint);
                // With a lobby, the match goes to its players only: not to
                // a peer that has not joined, nor one refused and waiting
                // out its grace.
                match &self.net.lobby {
                    Some(lobby) => {
                        server.set_audience(lobby.lobby().entries().iter().map(|entry| entry.peer));
                    }
                    None => server.clear_audience(),
                }
            }
            Replication::Client(client) => {
                let prefabs = super::spawning::prefab_store(
                    &mut self.prefabs,
                    &self.asset_root,
                    &self.registry,
                );
                let made = client.receive(
                    endpoint,
                    &mut self.world,
                    &self.registry,
                    Some(prefabs),
                    now,
                );
                self.spawned.extend(made);
            }
        }
    }

    pub(super) fn send_replication(&mut self, delta: f32) {
        let _replication = voltra_profile::scope("replication");
        let now = self.net.now();
        let (Some(replication), Some(endpoint)) =
            (self.net.replication.as_mut(), self.net.endpoint.as_mut())
        else {
            return;
        };
        match replication {
            Replication::Server(server) => {
                server.send(&mut self.world, &self.registry, endpoint);
            }
            Replication::Client(client) => {
                client.interpolate(&mut self.world, &self.registry, now, delta);
                client.send(endpoint);
            }
        }
        // A prefab a snapshot spawned is drawn this frame, its sprite
        // resolved.
        self.resolve_spawned();
    }
}

impl Tick<'_> {
    /// Steps `prediction` for this client's own entity: reconciles it with
    /// the newest server state, then moves it by `input` through
    /// `simulate` and sends the input. Call it from the fixed tick, once a
    /// step. `None` off a replicating client, or before the server has
    /// sent the entity this client owns.
    ///
    /// See [`Prediction`] for what `simulate` must promise.
    pub fn predict<I: Serialize>(
        &mut self,
        prediction: &mut Prediction<I>,
        input: I,
        simulate: impl FnMut(&mut World, Entity, &I, f32),
    ) -> Option<Entity> {
        let client = self.net.replication_mut()?.client_mut()?;
        prediction.step(
            self.world,
            client,
            self.registry,
            input,
            self.delta,
            simulate,
        )
    }
}
