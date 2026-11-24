//! A client's half of the handshake: ask, answer the challenge, be let in.

use std::collections::hash_map::RandomState;
use std::hash::BuildHasher;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use super::Endpoint;
use crate::config::PROTOCOL_VERSION;
use crate::connection::Connection;
use crate::packet::Packet;
use crate::{Address, DisconnectReason, NetEvent, PeerId};

/// How often a handshake packet is sent again while no answer comes.
const RETRY: Duration = Duration::from_millis(100);

enum Phase {
    Requesting,
    Responding { cookie: u64 },
    Connected { id: PeerId },
    Ended,
}

pub(super) struct ClientState {
    server: Address,
    nonce: u64,
    phase: Phase,
    started: Option<Duration>,
    last_sent: Option<Duration>,
}

/// A number no other handshake from this process has used, and unlikely
/// to be one any other process picks: what tells a restarted client from
/// its old self at the same address.
fn fresh_nonce() -> u64 {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let count = COUNTER.fetch_add(1, Ordering::Relaxed);
    RandomState::new().hash_one((count, entropy()))
}

/// Something that differs between two processes started at once: the
/// process id natively; in a browser — where `std` has neither a process id
/// nor random hash keys — `Math.random`.
#[cfg(not(target_arch = "wasm32"))]
fn entropy() -> u64 {
    u64::from(std::process::id())
}

#[cfg(target_arch = "wasm32")]
fn entropy() -> u64 {
    (js_sys::Math::random() * (1u64 << 53) as f64) as u64
}

impl ClientState {
    pub(super) fn new(server: Address) -> Self {
        Self {
            server,
            nonce: fresh_nonce(),
            phase: Phase::Requesting,
            started: None,
            last_sent: None,
        }
    }

    pub(super) fn server(&self) -> Address {
        self.server
    }

    pub(super) fn local_id(&self) -> Option<PeerId> {
        match self.phase {
            Phase::Connected { id } => Some(id),
            _ => None,
        }
    }

    pub(super) fn is_connecting(&self) -> bool {
        matches!(self.phase, Phase::Requesting | Phase::Responding { .. })
    }

    pub(super) fn end(&mut self) {
        self.phase = Phase::Ended;
    }

    /// Whether the handshake has run out of time — once, the tick it does.
    pub(super) fn gave_up(&mut self, now: Duration, timeout: Duration) -> bool {
        let started = *self.started.get_or_insert(now);
        if self.is_connecting() && now.saturating_sub(started) > timeout {
            self.phase = Phase::Ended;
            true
        } else {
            false
        }
    }

    /// The handshake packet due now, if one is.
    pub(super) fn handshake_packet(&mut self, now: Duration, fingerprint: u64) -> Option<Vec<u8>> {
        self.started.get_or_insert(now);
        if self
            .last_sent
            .is_some_and(|at| now.saturating_sub(at) < RETRY)
        {
            return None;
        }
        let packet = match self.phase {
            Phase::Requesting => Packet::Request {
                version: PROTOCOL_VERSION,
                fingerprint,
                nonce: self.nonce,
            },
            Phase::Responding { cookie } => Packet::Response {
                nonce: self.nonce,
                cookie,
            },
            Phase::Connected { .. } | Phase::Ended => return None,
        };
        self.last_sent = Some(now);
        Some(packet.write())
    }
}

impl Endpoint {
    pub(super) fn client_handshake(&mut self, from: Address, packet: Packet<'_>) {
        let super::Role::Client(client) = &mut self.role else {
            return;
        };
        if from != client.server {
            return;
        }
        match packet {
            Packet::Challenge { nonce, cookie }
                if nonce == client.nonce && matches!(client.phase, Phase::Requesting) =>
            {
                client.phase = Phase::Responding { cookie };
                // Answer at once rather than at the next retry.
                client.last_sent = None;
            }
            Packet::Accept {
                nonce,
                session,
                peer,
            } if nonce == client.nonce && client.is_connecting() && peer != 0 => {
                client.phase = Phase::Connected { id: PeerId(peer) };
                let connection = Connection::new(from, nonce, session, &self.config, self.now);
                self.connections.insert(PeerId::SERVER, connection);
                self.by_address.insert(from, PeerId::SERVER);
                self.events.push(NetEvent::Connected(PeerId::SERVER));
            }
            Packet::Deny { nonce, reason } if nonce == client.nonce && client.is_connecting() => {
                client.phase = Phase::Ended;
                self.events.push(NetEvent::Disconnected {
                    peer: PeerId::SERVER,
                    reason: DisconnectReason::from_code(reason),
                });
            }
            // A server's packets only, or stale ones; ignored.
            _ => {}
        }
    }
}
