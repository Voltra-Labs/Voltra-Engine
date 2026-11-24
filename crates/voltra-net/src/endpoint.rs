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
use crate::packet::Packet;
use crate::{
    encode, Address, ChannelId, CodecError, DisconnectReason, Message, NetConfig, NetEvent, PeerId,
    Transport,
};

mod client;
mod server;

use client::ClientState;
use server::ServerState;

/// The most datagrams one [`Endpoint::receive`] reads; the rest wait for
/// the next tick, so a flood cannot hold a frame hostage.
const MAX_DATAGRAMS_PER_RECEIVE: usize = 16_384;

/// How many times a disconnect is sent: it is not acknowledged, so it goes
/// a few times in the hope one arrives — and the timeout covers the rest.
const DISCONNECT_REPEATS: usize = 3;

/// Why a message was not queued.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SendError {
    /// No connection to that peer — never connected, or gone.
    NotConnected(PeerId),
    /// The configuration has no such channel.
    NoChannel(ChannelId),
    /// Longer than [`NetConfig::max_message_size`].
    TooLarge {
        /// The message's length.
        len: usize,
        /// The most allowed.
        max: usize,
    },
    /// The channel holds as much as it may for that peer — it has not
    /// acknowledged enough of what was sent. Try again next tick.
    ChannelFull(ChannelId),
    /// The value would not encode.
    Encode(CodecError),
}

impl std::fmt::Display for SendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotConnected(peer) => write!(f, "{peer} is not connected"),
            Self::NoChannel(channel) => write!(f, "there is no channel {}", channel.0),
            Self::TooLarge { len, max } => {
                write!(f, "a message of {len} bytes is over the limit of {max}")
            }
            Self::ChannelFull(channel) => write!(f, "channel {} is full", channel.0),
            Self::Encode(e) => write!(f, "the message would not encode: {e}"),
        }
    }
}

impl std::error::Error for SendError {}

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

    /// This end's own addresses, one per transport that has one.
    pub fn local_addr(&self) -> Option<Address> {
        self.transport.local_addr()
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
        let peers: Vec<PeerId> = self.peers().collect();
        for peer in peers {
            self.end(peer, DisconnectReason::Left, reason);
        }
        if let Role::Client(client) = &mut self.role {
            client.end();
        }
    }

    /// The top of a tick: what the transport has, read and handled — the
    /// handshake, acks, messages — then timeouts. Clears the last receive's
    /// messages and events first.
    pub fn receive(&mut self, now: Duration) {
        self.now = self.now.max(now);
        self.messages.clear();
        self.events = std::mem::take(&mut self.pending);
        if let Err(e) = self.transport.update(self.now) {
            self.transport_failed(e);
        }
        for _ in 0..MAX_DATAGRAMS_PER_RECEIVE {
            match self.transport.recv() {
                Ok(Some(datagram)) => self.handle(datagram.from, &datagram.payload),
                Ok(None) => break,
                Err(e) => {
                    self.transport_failed(e);
                    break;
                }
            }
        }
        while let Some(closed) = self.transport.take_closed() {
            if let Some(&peer) = self.by_address.get(&closed) {
                self.drop_peer(peer, DisconnectReason::TransportClosed);
            } else if let Role::Client(client) = &mut self.role {
                if client.server() == closed && client.is_connecting() {
                    client.end();
                    self.events.push(NetEvent::Disconnected {
                        peer: PeerId::SERVER,
                        reason: DisconnectReason::TransportClosed,
                    });
                }
            }
        }
        let timeout = self.config.timeout;
        let now = self.now;
        let late: Vec<PeerId> = self
            .connections
            .iter()
            .filter(|(_, c)| c.timed_out(now, timeout))
            .map(|(&peer, _)| peer)
            .collect();
        for peer in late {
            self.drop_peer(peer, DisconnectReason::Timeout);
        }
        if let Role::Client(client) = &mut self.role {
            if client.gave_up(now, self.config.connect_timeout) {
                self.events.push(NetEvent::Disconnected {
                    peer: PeerId::SERVER,
                    reason: DisconnectReason::ConnectTimeout,
                });
            }
        }
    }

    /// The end of a tick: the handshake's next packet on a client, then
    /// every connection's due fragments, acks and keep-alives.
    pub fn flush(&mut self, now: Duration) {
        self.now = self.now.max(now);
        let now = self.now;
        if let Role::Client(client) = &mut self.role {
            if let Some(packet) = client.handshake_packet(now, self.fingerprint) {
                let server = client.server();
                self.transmit(server, &packet);
            }
        }
        let config = &self.config;
        let mut out = Vec::new();
        for connection in self.connections.values_mut() {
            for packet in connection.flush(now, config) {
                out.push((connection.address, packet));
            }
        }
        for (to, packet) in out {
            self.transmit(to, &packet);
        }
    }

    /// [`receive`](Self::receive) then [`flush`](Self::flush), for a caller
    /// with nothing to do between them.
    pub fn update(&mut self, now: Duration) {
        self.receive(now);
        self.flush(now);
    }

    fn transmit(&mut self, to: Address, packet: &[u8]) {
        if let Err(e) = self.transport.send(to, packet) {
            log::debug!("could not send to {to}: {e}");
        }
    }

    fn transport_failed(&mut self, error: crate::TransportError) {
        if !std::mem::replace(&mut self.reported, true) {
            log::warn!("the network transport failed: {error}");
        }
    }

    fn handle(&mut self, from: Address, bytes: &[u8]) {
        let Ok(packet) = Packet::read(bytes) else {
            log::trace!("dropped a malformed packet from {from}");
            return;
        };
        match packet {
            Packet::Payload {
                session,
                sequence,
                acks,
                frames,
            } => {
                let Some(&peer) = self.by_address.get(&from) else {
                    return;
                };
                let Some(connection) = self.connections.get_mut(&peer) else {
                    return;
                };
                if connection.session != session {
                    return;
                }
                let messages = &mut self.messages;
                let result = connection.receive(
                    self.now,
                    sequence,
                    acks,
                    frames,
                    bytes.len(),
                    &mut |channel, bytes| {
                        messages.push(Message {
                            from: peer,
                            channel,
                            bytes,
                        })
                    },
                );
                if result.is_err() {
                    log::warn!("{peer} at {from} broke the protocol; disconnecting it");
                    self.end(
                        peer,
                        DisconnectReason::Violation,
                        DisconnectReason::Violation,
                    );
                }
            }
            Packet::Disconnect { session, reason } => {
                let Some(&peer) = self.by_address.get(&from) else {
                    return;
                };
                if self
                    .connections
                    .get(&peer)
                    .is_some_and(|c| c.session == session)
                {
                    self.drop_peer(peer, DisconnectReason::from_code(reason));
                }
            }
            handshake => match &mut self.role {
                Role::Server(_) => self.server_handshake(from, handshake),
                Role::Client(_) => self.client_handshake(from, handshake),
            },
        }
    }

    /// Ends `peer`'s connection from this end: tells it `told`, reports
    /// `seen` next receive.
    fn end(&mut self, peer: PeerId, seen: DisconnectReason, told: DisconnectReason) {
        let Some(connection) = self.connections.remove(&peer) else {
            return;
        };
        self.by_address.remove(&connection.address);
        let packet = Packet::Disconnect {
            session: connection.session,
            reason: told.code(),
        }
        .write();
        for _ in 0..DISCONNECT_REPEATS {
            self.transmit(connection.address, &packet);
        }
        self.transport.close(connection.address);
        if let Role::Client(client) = &mut self.role {
            client.end();
        }
        self.pending
            .push(NetEvent::Disconnected { peer, reason: seen });
    }

    /// Forgets `peer`, who is already gone; reports it this receive.
    fn drop_peer(&mut self, peer: PeerId, reason: DisconnectReason) {
        if let Some(connection) = self.connections.remove(&peer) {
            self.by_address.remove(&connection.address);
            self.transport.close(connection.address);
            if let Role::Client(client) = &mut self.role {
                client.end();
            }
            self.events.push(NetEvent::Disconnected { peer, reason });
        }
    }
}

impl Drop for Endpoint {
    /// A polite goodbye to every peer, so they need not wait for a timeout.
    fn drop(&mut self) {
        let reason = if self.is_server() {
            DisconnectReason::Shutdown
        } else {
            DisconnectReason::Closed
        };
        self.close_all(reason);
        // The goodbyes are written; a stream transport flushes them on its
        // way out.
        if let Err(e) = self.transport.update(self.now) {
            log::debug!("the last network update failed: {e}");
        }
    }
}
