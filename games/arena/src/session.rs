//! How a run of the arena plays: alone, hosting, or joined.

use voltra_core::net::{
    Advertise, HostOptions, LateJoin, LobbyConfig, NetConfig, DEFAULT_UDP_PORT,
    DEFAULT_WEBSOCKET_PORT,
};
use voltra_core::replication::{Interest, ReplicationConfig};
use voltra_core::Tick;

/// The arena's protocol number: a client of another game, or another
/// version of this one, is refused at the door.
pub const PROTOCOL: u64 = 0xA2E4_0001;

/// The arena's id on the LAN: what its servers advertise and its searches
/// ask for.
pub const GAME_ID: &str = "arena";

/// The lobby every end of an arena match runs: two to four players (one
/// per corner), and drop-in — a player joining a match under way plays at
/// once, as a race for a coin allows. Only the server's rules are read.
pub fn lobby_config() -> LobbyConfig {
    LobbyConfig {
        max_players: 4,
        min_players: 2,
        late_join: LateJoin::Play,
        ..LobbyConfig::default()
    }
}

/// How the arena is found on the LAN when it hosts: as `name`.
pub fn advertise(name: &str) -> Advertise {
    Advertise {
        name: name.to_owned(),
        game: GAME_ID.to_owned(),
        ..Advertise::default()
    }
}

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
    /// The lobby screen: host, find a match on the LAN, or join one by
    /// address. What the shipped game starts on.
    Menu,
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
    /// left as it is: whoever started it chose (and the lobby with it — a
    /// session made without one plays at once, as the editor's does).
    pub(crate) fn begin(&self, tick: &mut Tick<'_>) {
        if !tick.net.replicates() {
            tick.net.enable_replication(replication_config());
        }
        if tick.net.is_active() {
            return;
        }
        if *self != Self::Solo && !tick.net.has_lobby() {
            tick.net.enable_lobby(lobby_config());
        }
        let started = match self {
            Self::Solo | Self::Menu => Ok(()),
            Self::Host { udp, websocket } => tick.net.host(&host_options(*udp, *websocket)),
            Self::Join(server) => tick.net.join(server, net_config()),
        };
        match started {
            Ok(()) if *self == Self::Menu => {}
            Ok(()) => log::info!("arena: {self:?}"),
            Err(error) => log::error!("arena: could not start {self:?}: {error}"),
        }
    }
}

/// How the arena hosts on these ports: a listen server, found on the LAN.
pub fn host_options(udp: u16, websocket: u16) -> HostOptions {
    HostOptions {
        udp_port: Some(udp),
        websocket_port: Some(websocket),
        listen: true,
        config: net_config(),
        discovery: Some(advertise("Arena")),
        ..HostOptions::default()
    }
}
