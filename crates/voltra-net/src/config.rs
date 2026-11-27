//! What both ends of a connection have to agree on, and the limits each
//! holds the other to.

use std::time::Duration;

use crate::channel::{fragments_for, reserve, ChannelKind, DEFAULT_CHANNELS, MAX_CHANNELS};

/// The version of this crate's wire protocol. A peer speaking another is
/// refused at the handshake with
/// [`DisconnectReason::ProtocolMismatch`](crate::DisconnectReason::ProtocolMismatch).
pub const PROTOCOL_VERSION: u16 = 1;

/// The largest message any configuration may allow: past it a message is
/// better streamed in parts the game chooses.
pub const MAX_MESSAGE_LIMIT: usize = 16 * 1024 * 1024;

/// How a session behaves. Client and server must agree on
/// [`protocol`](Self::protocol), [`layer_version`](Self::layer_version),
/// [`lobby_version`](Self::lobby_version), [`channels`](Self::channels),
/// [`max_message_size`](Self::max_message_size) and
/// [`channel_budget`](Self::channel_budget): together they make the
/// fingerprint checked at the handshake, so a mismatch is refused there
/// rather than discovered as a broken message later.
#[derive(Debug, Clone, PartialEq)]
pub struct NetConfig {
    /// The game's own protocol: a number it changes whenever its messages
    /// do, so an old client is refused instead of misreading a new server.
    pub protocol: u64,
    /// The wire version of the engine layer carried on this session's
    /// channels — replication sets its own (`REPLICATION_WIRE_VERSION`)
    /// when it is on — so a peer whose layer reads other bytes is refused
    /// at the handshake rather than failing every decode after it. `0`,
    /// the default, is no layer, and leaves the fingerprint as it was.
    pub layer_version: u32,
    /// The wire version of the lobby carried on this session —
    /// [`LOBBY_WIRE_VERSION`](crate::LOBBY_WIRE_VERSION) when
    /// [`LobbyConfig::net_config`](crate::LobbyConfig::net_config) made it —
    /// so a peer whose lobby reads other bytes, or that has none, is
    /// refused at the handshake. `0`, the default, is no lobby, and leaves
    /// the fingerprint as it was.
    pub lobby_version: u32,
    /// The channels, in [`ChannelId`](crate::ChannelId) order. At most
    /// [`MAX_CHANNELS`]; more are dropped.
    pub channels: Vec<ChannelKind>,
    /// Clients a server holds at once; one more is refused with
    /// [`DisconnectReason::ServerFull`](crate::DisconnectReason::ServerFull).
    pub max_clients: usize,
    /// The longest message, in bytes. At most [`MAX_MESSAGE_LIMIT`].
    pub max_message_size: usize,
    /// The bytes a channel may hold for one peer: sent and not yet
    /// acknowledged on one end, received and not yet whole (or not yet in
    /// order) on the other. A peer that makes us hold more is cut off as
    /// hostile. Raised to fit at least one largest message.
    pub channel_budget: usize,
    /// Silence after which a packet is sent anyway, to keep the connection
    /// (and its acks) alive.
    pub keepalive: Duration,
    /// Silence from a connected peer after which it is dropped.
    pub timeout: Duration,
    /// How long a client tries to connect before it gives up.
    pub connect_timeout: Duration,
    /// Datagrams one flush may send to one peer — the bandwidth a tick may
    /// spend on it. What does not fit waits for the next.
    pub max_packets_per_flush: usize,
}

impl Default for NetConfig {
    fn default() -> Self {
        Self {
            protocol: 0,
            layer_version: 0,
            lobby_version: 0,
            channels: DEFAULT_CHANNELS.to_vec(),
            max_clients: 64,
            max_message_size: 64 * 1024,
            channel_budget: 1024 * 1024,
            keepalive: Duration::from_millis(250),
            timeout: Duration::from_secs(10),
            connect_timeout: Duration::from_secs(5),
            max_packets_per_flush: 64,
        }
    }
}

impl NetConfig {
    /// The configuration with every limit brought into range, so nothing
    /// downstream has to ask.
    pub(crate) fn validated(mut self) -> Self {
        self.channels.truncate(MAX_CHANNELS);
        self.max_message_size = self.max_message_size.min(MAX_MESSAGE_LIMIT);
        let largest = reserve(fragments_for(self.max_message_size) as u16);
        self.channel_budget = self.channel_budget.max(largest);
        self.max_packets_per_flush = self.max_packets_per_flush.max(1);
        self.keepalive = self.keepalive.max(Duration::from_millis(1));
        self
    }

    /// The most fragments one message may be split into.
    pub(crate) fn max_fragments(&self) -> u16 {
        fragments_for(self.max_message_size) as u16
    }

    /// What both ends must agree on, hashed (FNV-1a, 64-bit): the number
    /// the handshake compares, and LAN discovery advertises so a client
    /// can tell a server it could join from one it could not. Hash the
    /// config a session is actually made with (after
    /// `ReplicationConfig::net_config` and
    /// [`LobbyConfig::net_config`](crate::LobbyConfig::net_config)); the
    /// limits are brought into range first, as an endpoint does.
    pub fn fingerprint(&self) -> u64 {
        self.clone().validated().raw_fingerprint()
    }

    /// [`fingerprint`](Self::fingerprint) of a config already validated.
    pub(crate) fn raw_fingerprint(&self) -> u64 {
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        let mut eat = |bytes: &[u8]| {
            for &byte in bytes {
                hash ^= u64::from(byte);
                hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
            }
        };
        eat(&self.protocol.to_le_bytes());
        if self.layer_version != 0 {
            eat(&self.layer_version.to_le_bytes());
        }
        // Tagged, so a lobby version is never read as a layer version.
        if self.lobby_version != 0 {
            eat(b"lobby");
            eat(&self.lobby_version.to_le_bytes());
        }
        eat(&(self.max_message_size as u64).to_le_bytes());
        eat(&(self.channel_budget as u64).to_le_bytes());
        for kind in &self.channels {
            eat(&[kind.code()]);
        }
        hash
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_must_match_changes_the_fingerprint_and_what_need_not_does_not() {
        let base = NetConfig::default().validated();
        let other_game = NetConfig {
            protocol: 1,
            ..NetConfig::default()
        }
        .validated();
        let other_channels = NetConfig {
            channels: vec![ChannelKind::ReliableOrdered],
            ..NetConfig::default()
        }
        .validated();
        let other_layer = NetConfig {
            layer_version: 2,
            ..NetConfig::default()
        }
        .validated();
        let other_lobby = NetConfig {
            lobby_version: 2,
            ..NetConfig::default()
        }
        .validated();
        let lobby_as_layer = NetConfig {
            layer_version: 2,
            lobby_version: 0,
            ..NetConfig::default()
        }
        .validated();
        let other_timeout = NetConfig {
            timeout: Duration::from_secs(1),
            ..NetConfig::default()
        }
        .validated();

        assert_ne!(base.fingerprint(), other_game.fingerprint());
        assert_ne!(base.fingerprint(), other_channels.fingerprint());
        assert_ne!(base.fingerprint(), other_layer.fingerprint());
        assert_ne!(base.fingerprint(), other_lobby.fingerprint());
        assert_ne!(other_lobby.fingerprint(), lobby_as_layer.fingerprint());
        assert_eq!(base.fingerprint(), other_timeout.fingerprint());
    }

    #[test]
    fn limits_are_brought_into_range() {
        let config = NetConfig {
            channels: vec![ChannelKind::ReliableOrdered; 100],
            max_message_size: usize::MAX,
            channel_budget: 0,
            max_packets_per_flush: 0,
            ..NetConfig::default()
        }
        .validated();

        assert_eq!(config.channels.len(), MAX_CHANNELS);
        assert_eq!(config.max_message_size, MAX_MESSAGE_LIMIT);
        assert!(config.channel_budget >= MAX_MESSAGE_LIMIT);
        assert_eq!(config.max_packets_per_flush, 1);
    }
}
