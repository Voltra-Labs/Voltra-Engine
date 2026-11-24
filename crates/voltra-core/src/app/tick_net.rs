//! What a game's tick may do with the network: `tick.net`.
//!
//! The loop owns the session and pumps it — received at the top of every
//! frame, before the ticks, and flushed at the bottom, after them — so a
//! game only ever sends, reads and reacts. The same shape as Godot's
//! `multiplayer.poll()` run by the `SceneTree` for every scene, rather than
//! by each script.

use std::time::Duration;

use serde::Serialize;
use voltra_net::{
    Address, ChannelId, ConnectionStats, DisconnectReason, Endpoint, Message, NetConfig, NetEvent,
    PeerId, SendError, Transport,
};

use super::App;

/// The game's network session: none, a server, or a client.
///
/// What [`Tick::net`](super::Tick::net) hands a game. Messages and events
/// are per *frame*: [`received`](Self::received) and [`events`](Self::events)
/// hold what the frame's receive brought, for every hook in that frame to
/// read. A fixed tick that runs twice in a frame sees them twice — read
/// them in the per-frame hook, or take them with
/// [`take_received`](Self::take_received), which hands each out once.
#[derive(Default)]
pub struct Net {
    endpoint: Option<Endpoint>,
    /// The session's clock: the sum of the frames' deltas, so a headless
    /// run is as deterministic as its steps.
    now: Duration,
    /// A session [`stop`](Self::stop)ped, kept for one more receive so the
    /// game hears its own goodbye.
    stopping: bool,
    /// That receive has happened: the session goes at this frame's flush.
    goodbye_heard: bool,
}

impl Net {
    /// Runs `endpoint` as the game's session from the next frame, ending
    /// any session before it. The seam for a session built by hand — a
    /// custom transport, a [`Conditioned`](voltra_net::Conditioned) one in a
    /// test, several transports at once.
    pub fn start(&mut self, endpoint: Endpoint) {
        // The old one says goodbye as it drops.
        self.endpoint = Some(endpoint);
        self.stopping = false;
        self.goodbye_heard = false;
    }

    /// Hosts a server over `transport`.
    pub fn host(&mut self, transport: impl Transport + 'static, config: NetConfig) {
        self.start(Endpoint::server(transport, config));
    }

    /// Joins the server at `server` over `transport`; a
    /// [`NetEvent::Connected`] says when it let us in.
    pub fn join(
        &mut self,
        transport: impl Transport + 'static,
        server: Address,
        config: NetConfig,
    ) {
        self.start(Endpoint::client(transport, server, config));
    }

    /// Ends the session: a client leaves, a server tells every client it is
    /// shutting down. The disconnects are in the next frame's
    /// [`events`](Self::events), and then the session is gone.
    pub fn stop(&mut self) {
        if let Some(endpoint) = self.endpoint.as_mut() {
            let reason = if endpoint.is_server() {
                DisconnectReason::Shutdown
            } else {
                DisconnectReason::Closed
            };
            endpoint.close_all(reason);
            self.stopping = true;
        }
    }

    /// Whether there is a session.
    pub fn is_active(&self) -> bool {
        self.endpoint.is_some()
    }

    /// Whether this game is the server of its session.
    pub fn is_server(&self) -> bool {
        self.endpoint.as_ref().is_some_and(Endpoint::is_server)
    }

    /// Whether this game is a client still waiting to be let in.
    pub fn is_connecting(&self) -> bool {
        self.endpoint.as_ref().is_some_and(Endpoint::is_connecting)
    }

    /// This game's own id in the session: [`PeerId::SERVER`] when hosting,
    /// the number the server gave it when joined.
    pub fn local_id(&self) -> Option<PeerId> {
        self.endpoint.as_ref().and_then(Endpoint::local_id)
    }

    /// The connected peers: a server's clients, or a client's server.
    pub fn peers(&self) -> impl Iterator<Item = PeerId> + '_ {
        self.endpoint.iter().flat_map(Endpoint::peers)
    }

    /// Queues `bytes` for `peer` on `channel`, sent at the end of the frame.
    pub fn send(
        &mut self,
        peer: PeerId,
        channel: ChannelId,
        bytes: &[u8],
    ) -> Result<(), SendError> {
        match self.endpoint.as_mut() {
            Some(endpoint) => endpoint.send(peer, channel, bytes),
            None => Err(SendError::NotConnected(peer)),
        }
    }

    /// Queues `value` (serde, as `postcard`) for `peer` on `channel`.
    pub fn send_message<T: Serialize + ?Sized>(
        &mut self,
        peer: PeerId,
        channel: ChannelId,
        value: &T,
    ) -> Result<(), SendError> {
        match self.endpoint.as_mut() {
            Some(endpoint) => endpoint.send_message(peer, channel, value),
            None => Err(SendError::NotConnected(peer)),
        }
    }

    /// Queues `bytes` for every connected peer; answers those it could not
    /// be queued for, and why.
    pub fn broadcast(&mut self, channel: ChannelId, bytes: &[u8]) -> Vec<(PeerId, SendError)> {
        match self.endpoint.as_mut() {
            Some(endpoint) => endpoint.broadcast(channel, bytes),
            None => Vec::new(),
        }
    }

    /// The messages this frame's receive brought, whole and in each
    /// channel's order. Decode one with [`Message::decode`].
    pub fn received(&self) -> &[Message] {
        self.endpoint.as_ref().map_or(&[], Endpoint::messages)
    }

    /// Takes this frame's messages, so each is handled exactly once however
    /// many hooks look.
    pub fn take_received(&mut self) -> Vec<Message> {
        self.endpoint
            .as_mut()
            .map(Endpoint::take_messages)
            .unwrap_or_default()
    }

    /// What happened this frame: peers joining and leaving, kicks,
    /// timeouts, a connect that failed.
    pub fn events(&self) -> &[NetEvent] {
        self.endpoint.as_ref().map_or(&[], Endpoint::events)
    }

    /// Ends the connection to `peer`.
    pub fn disconnect(&mut self, peer: PeerId) {
        if let Some(endpoint) = self.endpoint.as_mut() {
            endpoint.disconnect(peer);
        }
    }

    /// Removes a client, telling it why. A server's call.
    pub fn kick(&mut self, peer: PeerId) {
        if let Some(endpoint) = self.endpoint.as_mut() {
            endpoint.kick(peer);
        }
    }

    /// The connection to `peer`: its RTT, packets and bytes.
    pub fn stats(&self, peer: PeerId) -> Option<ConnectionStats> {
        self.endpoint.as_ref().and_then(|e| e.stats(peer))
    }

    /// The session itself, for engine code built on it (replication) and
    /// for what this surface does not cover.
    pub fn endpoint(&self) -> Option<&Endpoint> {
        self.endpoint.as_ref()
    }

    /// The session itself, mutably.
    pub fn endpoint_mut(&mut self) -> Option<&mut Endpoint> {
        self.endpoint.as_mut()
    }

    /// The session's clock: every frame's delta so far.
    pub fn now(&self) -> Duration {
        self.now
    }

    fn receive(&mut self, delta: f32) {
        if delta.is_finite() && delta > 0.0 {
            self.now += Duration::from_secs_f32(delta);
        }
        if let Some(endpoint) = self.endpoint.as_mut() {
            endpoint.receive(self.now);
        }
        self.goodbye_heard = self.stopping;
    }

    fn flush(&mut self) {
        if let Some(endpoint) = self.endpoint.as_mut() {
            endpoint.flush(self.now);
        }
        if self.goodbye_heard {
            self.endpoint = None;
            self.stopping = false;
            self.goodbye_heard = false;
        }
    }
}

impl App {
    /// The top of the frame's network: what arrived, before the ticks read
    /// it. Every frame, simulating or not, so an editor paused mid-session
    /// still keeps its connections alive.
    pub(super) fn receive_net(&mut self, delta: f32) {
        let _net = voltra_profile::scope("net");
        self.net.receive(delta);
    }

    /// The bottom of the frame's network: everything the ticks queued, the
    /// acks and the keep-alives, sent.
    pub(super) fn flush_net(&mut self) {
        let _net = voltra_profile::scope("net");
        self.net.flush();
    }

    /// The game's network session.
    pub fn net(&self) -> &Net {
        &self.net
    }

    /// The game's network session, to start one before the first frame.
    pub fn net_mut(&mut self) -> &mut Net {
        &mut self.net
    }
}
