//! How a run of the arena plays: alone, hosting, or joined.

use voltra_core::net::{HostOptions, NetConfig, DEFAULT_UDP_PORT, DEFAULT_WEBSOCKET_PORT};
use voltra_core::replication::{Interest, ReplicationConfig};
use voltra_core::Tick;

/// The arena's protocol number: a client of another game, or another
/// version of this one, is refused at the door.
pub const PROTOCOL: u64 = 0xA2E4_0001;

/// The session config every end of an arena match uses.
pub fn net_config() -> NetConfig {
    NetConfig {
        protocol: PROTOCOL,
        ..NetConfig::default()
    }
}

/// How far a player is sent the match, in world units: the floor is 17 by
/// 9, so 20 covers it from anywhere on it — every player sees every other,
/// as the scores need — and the grid is still built and read each
/// snapshot, so the arena exercises interest management end to end. A
/// larger map would be sent by neighbourhood with no change here.
pub const INTEREST_RADIUS: f32 = 20.0;

/// The replication every end of an arena match runs: the engine's, with
/// interest management on ([`INTEREST_RADIUS`]). Only the server's is
/// read; the clients carry the same for symmetry.
pub fn replication_config() -> ReplicationConfig {
    ReplicationConfig {
        interest: Some(Interest::new(INTEREST_RADIUS)),
        ..ReplicationConfig::default()
    }
}

/// How this run plays.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Session {
    /// No network: one player, this one. What the editor's plain Play is.
    #[default]
    Solo,
    /// A listen server: this run hosts on these ports (UDP for native
    /// players, WebSocket for browsers) and plays too.
    Host {
        /// UDP port, `0` for any free one.
        udp: u16,
        /// WebSocket port, `0` for any free one.
        websocket: u16,
    },
    /// A client of the server at this address: `udp://host:port`, or
    /// `ws://host:port` (the only kind a browser can reach).
    Join(String),
}

impl Session {
    /// Hosting on the default ports.
    pub fn host() -> Self {
        Self::Host {
            udp: DEFAULT_UDP_PORT,
            websocket: DEFAULT_WEBSOCKET_PORT,
        }
    }

    /// Starts the session from the game's first turn. A session already
    /// running — the editor's network play, a dedicated server's — is
    /// left as it is: whoever started it chose.
    pub(crate) fn begin(&self, tick: &mut Tick<'_>) {
        if !tick.net.replicates() {
            tick.net.enable_replication(replication_config());
        }
        if tick.net.is_active() {
            return;
        }
        let started = match self {
            Self::Solo => Ok(()),
            Self::Host { udp, websocket } => tick.net.host(&HostOptions {
                udp_port: Some(*udp),
                websocket_port: Some(*websocket),
                listen: true,
                config: net_config(),
                ..HostOptions::default()
            }),
            Self::Join(server) => tick.net.join(server, net_config()),
        };
        match started {
            Ok(()) => log::info!("arena: {self:?}"),
            Err(error) => log::error!("arena: could not start {self:?}: {error}; playing alone"),
        }
    }
}
