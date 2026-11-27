//! How replication is tuned, and the channels it travels on.

use voltra_net::{ChannelId, ChannelKind, NetConfig, DEFAULT_CHANNELS};

use crate::{Interest, LagCompensation};

/// The three channels replication owns, beside the game's.
///
/// Its own, so the game's messages never queue behind a snapshot and a
/// snapshot never reaches the game's [`received`] — and so the sequenced
/// snapshot channel orders snapshots only, not snapshots among chat.
///
/// [`received`]: voltra_net::Endpoint::messages
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReplicationChannels {
    /// Snapshots, server to client: unreliable-sequenced — a late snapshot
    /// is worth nothing once a newer one is in.
    pub state: ChannelId,
    /// What must arrive: the component names a server replicates, sent once
    /// per client. Reliable-ordered.
    pub control: ChannelId,
    /// Client to server: which snapshots arrived, and the player's inputs.
    /// Unreliable-sequenced; every message carries the recent inputs again,
    /// so one lost costs nothing.
    pub input: ChannelId,
}

impl ReplicationChannels {
    /// The three right after [`DEFAULT_CHANNELS`]: `3`, `4` and `5`.
    pub const DEFAULT: Self = Self {
        state: ChannelId(DEFAULT_CHANNELS.len() as u8),
        control: ChannelId(DEFAULT_CHANNELS.len() as u8 + 1),
        input: ChannelId(DEFAULT_CHANNELS.len() as u8 + 2),
    };

    fn kinds(&self) -> [(ChannelId, ChannelKind); 3] {
        [
            (self.state, ChannelKind::UnreliableSequenced),
            (self.control, ChannelKind::ReliableOrdered),
            (self.input, ChannelKind::UnreliableSequenced),
        ]
    }

    /// Whether `config` carries these channels with the kinds replication
    /// needs. A session made without [`ReplicationConfig::net_config`]
    /// does not, and replication stays off for it rather than misreading
    /// a game channel.
    pub fn fit(&self, config: &NetConfig) -> bool {
        self.kinds()
            .iter()
            .all(|(id, kind)| config.channels.get(usize::from(id.0)) == Some(kind))
    }
}

impl Default for ReplicationChannels {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// How the server replicates, and how a client shows what it is sent.
#[derive(Debug, Clone, PartialEq)]
pub struct ReplicationConfig {
    /// The channels it travels on: [`ReplicationChannels::DEFAULT`].
    pub channels: ReplicationChannels,
    /// The most bytes of snapshot one client is sent per snapshot. The
    /// default, 1000, keeps a snapshot inside one datagram with room for
    /// its packet's header — 60 KB/s at 60 snapshots a second. What does
    /// not fit waits, its priority banked, for the next.
    pub bytes_per_snapshot: usize,
    /// A snapshot every this many server ticks: `1`, every tick.
    pub snapshot_interval: u32,
    /// Sent snapshots a server remembers per client, waiting for their
    /// acks. One older than this is taken as lost.
    pub history: usize,
    /// The most inputs a server holds per client before it drops the
    /// oldest: a client running fast, or a burst after a stall, must not
    /// queue its inputs into seconds of latency.
    pub max_buffered_inputs: usize,
    /// The largest one encoded input may be, in bytes; a bigger one is a
    /// client misbehaving, and is dropped.
    pub max_input_size: usize,
    /// How many recent inputs a client sends with every message, so a lost
    /// one is carried by the next.
    pub input_redundancy: usize,
    /// The least interpolation delay, in snapshot intervals, whatever the
    /// measured jitter: two snapshots must be in hand to draw between.
    pub min_delay_snapshots: f32,
    /// Interest management: each client is sent only the entities near what
    /// it watches. `None`, the default, sends every client every replicated
    /// entity. Only the server's is read.
    pub interest: Option<Interest>,
    /// Lag compensation: how much of the past the server keeps of its
    /// `LagCompensated` colliders, and how far back a client's shot may be
    /// tested. On by default — it records only what a game marks; `None`
    /// keeps nothing, and every rewound query is a query of the world as
    /// it is. Only the server's is read.
    pub lag_compensation: Option<LagCompensation>,
}

impl Default for ReplicationConfig {
    fn default() -> Self {
        Self {
            channels: ReplicationChannels::DEFAULT,
            bytes_per_snapshot: 1000,
            snapshot_interval: 1,
            history: 64,
            max_buffered_inputs: 8,
            max_input_size: 256,
            input_redundancy: 8,
            min_delay_snapshots: 1.0,
            interest: None,
            lag_compensation: Some(LagCompensation::default()),
        }
    }
}

impl ReplicationConfig {
    /// `base` with replication's channels in place — extended with the
    /// game's own channel kinds kept, the three put where
    /// [`channels`](Self::channels) says — and its wire version
    /// ([`REPLICATION_WIRE_VERSION`](crate::REPLICATION_WIRE_VERSION)) as
    /// the session's `layer_version`. Both ends must make their
    /// session's config this way — the handshake's fingerprint covers the
    /// channel layout and the layer version, so a client without them, or
    /// with another build's replication wire, is refused at the door.
    ///
    /// A game channel already at one of those ids is overwritten, with a
    /// warning: pick other ids for replication if a game needs more than
    /// the default three.
    pub fn net_config(&self, mut base: NetConfig) -> NetConfig {
        base.layer_version = crate::REPLICATION_WIRE_VERSION;
        for (id, kind) in self.channels.kinds() {
            let index = usize::from(id.0);
            if base.channels.len() <= index {
                base.channels
                    .resize(index + 1, ChannelKind::ReliableOrdered);
            } else if index >= DEFAULT_CHANNELS.len() && base.channels[index] != kind {
                log::warn!(
                    "channel {} is the game's {:?}; replication takes it as {:?}",
                    id.0,
                    base.channels[index],
                    kind
                );
            }
            base.channels[index] = kind;
        }
        base
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_config_gets_three_more_channels() {
        let config = ReplicationConfig::default();
        let net = config.net_config(NetConfig::default());
        assert_eq!(net.channels.len(), 6);
        assert_eq!(&net.channels[..3], &DEFAULT_CHANNELS);
        assert!(config.channels.fit(&net));
        assert_eq!(net.layer_version, crate::REPLICATION_WIRE_VERSION);
        assert!(!config.channels.fit(&NetConfig::default()));
        // Idempotent.
        assert_eq!(config.net_config(net.clone()), net);
    }
}
