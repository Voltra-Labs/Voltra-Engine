//! How a run of the arena plays: alone, hosting, or joined.

use voltra_core::net::{HostOptions, NetConfig, DEFAULT_UDP_PORT, DEFAULT_WEBSOCKET_PORT};
use voltra_core::replication::ReplicationConfig;
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
            tick.net.enable_replication(ReplicationConfig::default());
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
