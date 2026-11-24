//! A server's half of the handshake: challenge a stranger, admit one that
//! answers it.
//!
//! The challenge is stateless — its cookie is a keyed hash of who asked,
//! with what nonce, and when — so a flood of requests from spoofed
//! addresses costs the server a hash each and no memory. Only a response
//! carrying a cookie the server made (which needs the challenge to have
//! reached the sender's real address) makes a connection.

use std::collections::hash_map::RandomState;
use std::hash::BuildHasher;
use std::time::Duration;

use super::Endpoint;
use crate::config::PROTOCOL_VERSION;
use crate::connection::Connection;
use crate::packet::Packet;
use crate::{Address, DisconnectReason, NetEvent, PeerId};

/// How long a cookie is good for: its time slice, and the one before it.
const COOKIE_SLICE: Duration = Duration::from_secs(10);

pub(super) struct ServerState {
    /// Random keys for the cookie and session hashes, new every run.
    secret: RandomState,
    next_peer: u32,
}

impl ServerState {
    pub(super) fn new() -> Self {
        Self {
            secret: RandomState::new(),
            next_peer: 1,
        }
    }

    fn cookie(&self, from: Address, nonce: u64, slice: u64) -> u64 {
        self.secret.hash_one((from, nonce, slice, "cookie"))
    }

    fn cookie_holds(&self, from: Address, nonce: u64, cookie: u64, now: Duration) -> bool {
        let slice = now.as_secs() / COOKIE_SLICE.as_secs();
        cookie == self.cookie(from, nonce, slice)
            || slice > 0 && cookie == self.cookie(from, nonce, slice - 1)
    }
}

impl Endpoint {
    pub(super) fn server_handshake(&mut self, from: Address, packet: Packet<'_>) {
        let super::Role::Server(server) = &self.role else {
            return;
        };
        match packet {
            Packet::Request {
                version,
                fingerprint,
                nonce,
            } => {
                let reply = if version != PROTOCOL_VERSION || fingerprint != self.fingerprint {
                    Packet::Deny {
                        nonce,
                        reason: DisconnectReason::ProtocolMismatch.code(),
                    }
                } else if self.connections.len() >= self.config.max_clients
                    && !self.by_address.contains_key(&from)
                {
                    Packet::Deny {
                        nonce,
                        reason: DisconnectReason::ServerFull.code(),
                    }
                } else {
                    let slice = self.now.as_secs() / COOKIE_SLICE.as_secs();
                    Packet::Challenge {
                        nonce,
                        cookie: server.cookie(from, nonce, slice),
                    }
                };
                self.transmit(from, &reply.write());
            }
            Packet::Response { nonce, cookie } => {
                if !server.cookie_holds(from, nonce, cookie, self.now) {
                    return;
                }
                let session = server.secret.hash_one((from, nonce, cookie, "session"));
                if let Some(&peer) = self.by_address.get(&from) {
                    match self.connections.get(&peer) {
                        // Our accept was lost: say it again.
                        Some(c) if c.nonce == nonce => {
                            let accept = Packet::Accept {
                                nonce,
                                session: c.session,
                                peer: peer.0,
                            };
                            self.transmit(from, &accept.write());
                            return;
                        }
                        // A new handshake from a connected address: the
                        // client there restarted, and the old one is gone.
                        _ => self.drop_peer(peer, DisconnectReason::Closed),
                    }
                }
                if self.connections.len() >= self.config.max_clients {
                    let deny = Packet::Deny {
                        nonce,
                        reason: DisconnectReason::ServerFull.code(),
                    };
                    self.transmit(from, &deny.write());
                    return;
                }
                let peer = self.next_free_peer();
                let connection = Connection::new(from, nonce, session, &self.config, self.now);
                self.connections.insert(peer, connection);
                self.by_address.insert(from, peer);
                self.events.push(NetEvent::Connected(peer));
                let accept = Packet::Accept {
                    nonce,
                    session,
                    peer: peer.0,
                };
                self.transmit(from, &accept.write());
            }
            // A client's packets only; a server ignores the rest.
            _ => {}
        }
    }

    /// The next id no connected peer has, skipping [`PeerId::SERVER`].
    fn next_free_peer(&mut self) -> PeerId {
        let super::Role::Server(server) = &mut self.role else {
            return PeerId::SERVER;
        };
        loop {
            let peer = PeerId(server.next_peer);
            server.next_peer = server.next_peer.wrapping_add(1);
            if peer != PeerId::SERVER && !self.connections.contains_key(&peer) {
                return peer;
            }
        }
    }
}
