//! Who is connected, and what happened to them.

use std::fmt;

/// A connected peer, as this end numbers it.
///
/// A server numbers its clients from 1 and never reuses a number while
/// the session runs; to a client, its server is always [`PeerId::SERVER`].
/// A client learns its own number from the server
/// ([`Endpoint::local_id`](crate::Endpoint::local_id)) — the id replication
/// (stage 114) names an owner by.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PeerId(pub u32);

impl PeerId {
    /// The server, from either end.
    pub const SERVER: Self = Self(0);
}

impl fmt::Display for PeerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if *self == Self::SERVER {
            write!(f, "the server")
        } else {
            write!(f, "peer {}", self.0)
        }
    }
}

/// Why a connection ended, or never began.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DisconnectReason {
    /// This end chose to: [`Endpoint::disconnect`](crate::Endpoint::disconnect).
    Left,
    /// The other end said it was leaving.
    Closed,
    /// The server removed this client ([`Endpoint::kick`](crate::Endpoint::kick)),
    /// as seen by either end.
    Kicked,
    /// The server had no room.
    ServerFull,
    /// The other end speaks another protocol version, or another game's
    /// [`NetConfig::protocol`](crate::NetConfig::protocol), channel layout
    /// or [`NetConfig::layer_version`](crate::NetConfig::layer_version)
    /// (another build's replication wire).
    ProtocolMismatch,
    /// The other end broke the protocol — a malformed packet, a message
    /// past the limits — and was cut off (or cut us off for it).
    Violation,
    /// The server is shutting down.
    Shutdown,
    /// Nothing arrived for [`NetConfig::timeout`](crate::NetConfig::timeout).
    Timeout,
    /// The server never answered within
    /// [`NetConfig::connect_timeout`](crate::NetConfig::connect_timeout).
    ConnectTimeout,
    /// The stream under the connection ended (a WebSocket closed), or the
    /// transport broke.
    TransportClosed,
}

impl DisconnectReason {
    /// The reason as the other end is told it, for those that are sent.
    pub(crate) fn code(self) -> u8 {
        match self {
            Self::Left | Self::Closed => 1,
            Self::Kicked => 2,
            Self::ServerFull => 3,
            Self::ProtocolMismatch => 4,
            Self::Violation => 5,
            Self::Shutdown => 6,
            // Never sent: each is something only this end can see.
            Self::Timeout | Self::ConnectTimeout | Self::TransportClosed => 1,
        }
    }

    /// The reason a code read off the wire gives; an unknown one is a plain
    /// close, not an error — a newer peer may know reasons we do not.
    pub(crate) fn from_code(code: u8) -> Self {
        match code {
            2 => Self::Kicked,
            3 => Self::ServerFull,
            4 => Self::ProtocolMismatch,
            5 => Self::Violation,
            6 => Self::Shutdown,
            _ => Self::Closed,
        }
    }
}

impl fmt::Display for DisconnectReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Left => "left",
            Self::Closed => "closed by the other end",
            Self::Kicked => "kicked",
            Self::ServerFull => "the server is full",
            Self::ProtocolMismatch => {
                "a different protocol (version, game, channels or replication wire)"
            }
            Self::Violation => "a protocol violation",
            Self::Shutdown => "the server shut down",
            Self::Timeout => "timed out",
            Self::ConnectTimeout => "the server did not answer",
            Self::TransportClosed => "the connection's stream ended",
        })
    }
}

/// Something that happened to a connection, handed out once.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetEvent {
    /// A peer finished the handshake: a client joined (on a server), or the
    /// server accepted us (on a client, `peer` is [`PeerId::SERVER`]).
    Connected(PeerId),
    /// A peer is gone, or a client's attempt to connect failed.
    Disconnected {
        /// Who.
        peer: PeerId,
        /// Why.
        reason: DisconnectReason,
    },
}
