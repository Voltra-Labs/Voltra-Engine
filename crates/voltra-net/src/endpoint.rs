//! An endpoint: a transport, the connections over it, and the handshake
//! that makes them — a server's many clients or a client's one server.
//!
//! One type for both roles, because everything after the handshake is the
//! same on both ends: a set of peers, messages to them on channels, events
//! about them. A server's peers are its clients; a client's only peer is
//! [`PeerId::SERVER`]. Godot's `MultiplayerPeer` and Unity's `NetworkDriver`
//! are one type for the same reason.
//!
//! Polled like the transport under it: [`receive`](Endpoint::receive) at
//! the top of a tick, [`flush`](Endpoint::flush) at the end, the caller's
//! clock passed to both.

use std::collections::{BTreeMap, HashMap};
use std::time::Duration;

use serde::Serialize;

use crate::connection::{Connection, ConnectionStats};
use crate::{
    encode, Address, ChannelId, DisconnectReason, Message, NetConfig, NetEvent, PeerId, Transport,
};

mod client;
mod error;
mod pump;
mod server;

pub use error::SendError;

use client::ClientState;
use server::ServerState;

/// The most datagrams one [`Endpoint::receive`] reads; the rest wait for
/// the next tick, so a flood cannot hold a frame hostage.
pub(super) const MAX_DATAGRAMS_PER_RECEIVE: usize = 16_384;

/// How many times a disconnect is sent: it is not acknowledged, so it goes
/// a few times in the hope one arrives — and the timeout covers the rest.
pub(super) const DISCONNECT_REPEATS: usize = 3;

enum Role {
    Server(ServerState),
    Client(ClientState),
}

/// A server or a client: connections, channels and messages over a
/// [`Transport`].
pub struct Endpoint {
    transport: Box<dyn Transport>,
    config: NetConfig,
    fingerprint: u64,
    role: Role,
    connections: BTreeMap<PeerId, Connection>,
    by_address: HashMap<Address, PeerId>,
    messages: Vec<Message>,
    events: Vec<NetEvent>,
    /// Events from between two receives — a kick from the tick — handed
    /// out with the next receive's, so each is seen once.
    pending: Vec<NetEvent>,
    now: Duration,
    /// Whether the transport's failure was logged, so a dead socket warns
    /// once rather than every tick.
    reported: bool,
    /// A listen server: the hosting process plays as [`PeerId::SERVER`].
    host_plays: bool,
}

impl Endpoint {
    /// A server accepting clients through `transport`.
    pub fn server(transport: impl Transport + 'static, config: NetConfig) -> Self {
        Self::new(
            Box::new(transport),
            config,
            Role::Server(ServerState::new()),
        )
    }

    /// A client connecting to `server` through `transport`. The handshake
    /// starts with the first [`flush`](Self::flush); a
    /// [`NetEvent::Connected`] says when it is done.
    pub fn client(transport: impl Transport + 'static, server: Address, config: NetConfig) -> Self {
        Self::new(
            Box::new(transport),
            config,
            Role::Client(ClientState::new(server)),
        )
    }

    fn new(transport: Box<dyn Transport>, config: NetConfig, role: Role) -> Self {
        let config = config.validated();
        Self {
            transport,
            fingerprint: config.fingerprint(),
            config,
            role,
            connections: BTreeMap::new(),
            by_address: HashMap::new(),
            messages: Vec::new(),
            events: Vec::new(),
            pending: Vec::new(),
            now: Duration::ZERO,
            reported: false,
            host_plays: false,
        }
    }

    /// Makes a server a listen server — the hosting process plays too, as
    /// [`PeerId::SERVER`] — or a dedicated one again. Turned on, the host's
    /// own player joins as a client would: a [`NetEvent::Connected`] for
    /// [`PeerId::SERVER`] in the next receive's events, so one code path
    /// spawns every player (Unity's Netcode raises its client-connected
    /// callback for the host's own client the same way); turned off, or at
    /// [`close_all`](Self::close_all), a [`NetEvent::Disconnected`] for it.
    /// Nothing on a client.
    pub fn set_host_plays(&mut self, plays: bool) {
        if !self.is_server() || plays == self.host_plays {
            return;
        }
        self.host_plays = plays;
        self.pending.push(if plays {
            NetEvent::Connected(PeerId::SERVER)
        } else {
            NetEvent::Disconnected {
                peer: PeerId::SERVER,
                reason: DisconnectReason::Left,
            }
        });
    }

    /// Whether this is a listen server, whose host plays.
    pub fn host_plays(&self) -> bool {
        self.host_plays
    }

    /// Everyone playing in this session that this end knows of: on a
    /// server its clients, and itself first if the host plays; on a client,
    /// itself once it is in. (A client learns of the others through the
    /// game's own messages, or replication.)
    pub fn players(&self) -> Vec<PeerId> {
        match &self.role {
            Role::Server(_) => self
                .host_plays
                .then_some(PeerId::SERVER)
                .into_iter()
                .chain(self.peers())
                .collect(),
            Role::Client(client) => client.local_id().into_iter().collect(),
        }
    }

    /// The configuration, with its limits brought into range.
    pub fn config(&self) -> &NetConfig {
        &self.config
    }

    /// Whether this end is the server.
    pub fn is_server(&self) -> bool {
        matches!(self.role, Role::Server(_))
    }

    /// This end's id: [`PeerId::SERVER`] on a server, the number the server
    /// gave us on a connected client, `None` on a client not (yet) in.
    pub fn local_id(&self) -> Option<PeerId> {
        match &self.role {
            Role::Server(_) => Some(PeerId::SERVER),
            Role::Client(client) => client.local_id(),
        }
    }

    /// Whether a client is still trying to connect.
    pub fn is_connecting(&self) -> bool {
        matches!(&self.role, Role::Client(client) if client.is_connecting())
    }

    /// Whether `peer` is connected.
    pub fn is_connected(&self, peer: PeerId) -> bool {
        self.connections.contains_key(&peer)
    }

    /// The connected peers, in id order.
    pub fn peers(&self) -> impl Iterator<Item = PeerId> + '_ {
        self.connections.keys().copied()
    }

    /// How many peers are connected.
    pub fn peer_count(&self) -> usize {
        self.connections.len()
    }

    /// Where `peer` is, as its transport names it.
    pub fn address(&self, peer: PeerId) -> Option<Address> {
        self.connections.get(&peer).map(|c| c.address)
    }

    /// What the connection to `peer` has done: its RTT, packets and bytes.
    pub fn stats(&self, peer: PeerId) -> Option<ConnectionStats> {
        self.connections.get(&peer).map(Connection::stats)
    }

    /// This end's own address, the first transport's that has one.
    pub fn local_addr(&self) -> Option<Address> {
        self.transport.local_addr()
    }

    /// Every address this end is reached at — a host on UDP and WebSockets
    /// has one of each.
    pub fn local_addrs(&self) -> Vec<Address> {
        self.transport.local_addrs()
    }

    /// The messages the last [`receive`](Self::receive) delivered, whole and
    /// in each channel's order.
    pub fn messages(&self) -> &[Message] {
        &self.messages
    }

    /// Takes the messages the last receive delivered, leaving none: for a
    /// caller that must see each exactly once however often it looks.
    pub fn take_messages(&mut self) -> Vec<Message> {
        std::mem::take(&mut self.messages)
    }

    /// What happened since the receive before the last one: joins, leaves,
    /// kicks, timeouts, and on a client the end of a failed connect.
    pub fn events(&self) -> &[NetEvent] {
        &self.events
    }

    /// Queues `bytes` for `peer` on `channel`; it goes with the next
    /// [`flush`](Self::flush).
    pub fn send(
        &mut self,
        peer: PeerId,
        channel: ChannelId,
        bytes: &[u8],
    ) -> Result<(), SendError> {
        let max = self.config.max_message_size;
        if bytes.len() > max {
            return Err(SendError::TooLarge {
                len: bytes.len(),
                max,
            });
        }
        if usize::from(channel.0) >= self.config.channels.len() {
            return Err(SendError::NoChannel(channel));
        }
        self.connections
            .get_mut(&peer)
            .ok_or(SendError::NotConnected(peer))?
            .queue(channel, bytes)
    }

    /// Queues `value`, encoded with [`encode`], for `peer` on `channel`.
    pub fn send_message<T: Serialize + ?Sized>(
        &mut self,
        peer: PeerId,
        channel: ChannelId,
        value: &T,
    ) -> Result<(), SendError> {
        let bytes = encode(value).map_err(SendError::Encode)?;
        self.send(peer, channel, &bytes)
    }

    /// Queues `bytes` for every connected peer; answers the peers it could
    /// not be queued for, and why.
    pub fn broadcast(&mut self, channel: ChannelId, bytes: &[u8]) -> Vec<(PeerId, SendError)> {
        let peers: Vec<PeerId> = self.peers().collect();
        peers
            .into_iter()
            .filter_map(|peer| self.send(peer, channel, bytes).err().map(|e| (peer, e)))
            .collect()
    }

    /// Ends the connection to `peer`, telling it so. On a client,
    /// [`PeerId::SERVER`] leaves the server.
    pub fn disconnect(&mut self, peer: PeerId) {
        self.end(peer, DisconnectReason::Left, DisconnectReason::Closed);
    }

    /// Removes a client, telling it it was kicked. Only a server kicks; on
    /// a client this is [`disconnect`](Self::disconnect).
    pub fn kick(&mut self, peer: PeerId) {
        if self.is_server() {
            self.end(peer, DisconnectReason::Kicked, DisconnectReason::Kicked);
        } else {
            self.disconnect(peer);
        }
    }

    /// Ends every connection, telling each `reason` — a server shutting
    /// down says [`DisconnectReason::Shutdown`].
    pub fn close_all(&mut self, reason: DisconnectReason) {
        self.set_host_plays(false);
        let peers: Vec<PeerId> = self.peers().collect();
        for peer in peers {
            self.end(peer, DisconnectReason::Left, reason);
        }
        if let Role::Client(client) = &mut self.role {
            client.end();
        }
    }
}
