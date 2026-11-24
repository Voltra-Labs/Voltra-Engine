//! An endpoint's tick: what arrived read and handled, what is due sent,
//! and connections ended — politely, or because the peer is gone.

use std::time::Duration;

use super::{Endpoint, Role, DISCONNECT_REPEATS, MAX_DATAGRAMS_PER_RECEIVE};
use crate::packet::Packet;
use crate::{Address, DisconnectReason, Message, NetEvent, PeerId};

impl Endpoint {
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

    pub(super) fn transmit(&mut self, to: Address, packet: &[u8]) {
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
    pub(super) fn end(&mut self, peer: PeerId, seen: DisconnectReason, told: DisconnectReason) {
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
    pub(super) fn drop_peer(&mut self, peer: PeerId, reason: DisconnectReason) {
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
