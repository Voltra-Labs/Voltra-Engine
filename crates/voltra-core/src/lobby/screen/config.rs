//! What a game tells the lobby screen, what can stop it opening, and what a
//! frame of it answers.

use std::fmt;

use voltra_net::{Advertise, HostOptions, LobbyConfig, SearchOptions};
use voltra_scene::SceneError;

use crate::lobby::LOBBY_PREFAB;

/// How the screen hosts, looks for and joins games.
#[derive(Debug, Clone, PartialEq)]
pub struct LobbyScreenConfig {
    /// The prefab, under the asset root: [`LOBBY_PREFAB`] unless a game has
    /// its own.
    pub prefab: String,
    /// The game's id on the LAN: what its servers advertise and a search
    /// asks for, so another game's servers are not listed.
    pub game: String,
    /// How Host hosts: ports, the session's config (which Join joins with
    /// too) and LAN discovery. `listen` is always on — the player hosting
    /// plays. An [`Advertise`] with an empty name is named after the
    /// player ("Ada's game"), and one with no game id takes
    /// [`game`](Self::game).
    pub host: HostOptions,
    /// The lobby's rules, turned on at open if the game has not turned it
    /// on itself ([`Net::enable_lobby`](crate::Net::enable_lobby)).
    pub lobby: LobbyConfig,
    /// How Find on LAN looks: the probe port, how long, which addresses
    /// besides the broadcast. Its game and fingerprint are filled in.
    pub search: SearchOptions,
    /// What the address field starts with.
    pub address: String,
}

impl Default for LobbyScreenConfig {
    fn default() -> Self {
        Self {
            prefab: LOBBY_PREFAB.to_owned(),
            game: String::new(),
            host: HostOptions {
                discovery: Some(Advertise {
                    name: String::new(),
                    ..Advertise::default()
                }),
                ..HostOptions::default()
            },
            lobby: LobbyConfig::default(),
            search: SearchOptions::default(),
            address: if cfg!(target_arch = "wasm32") {
                format!("ws://127.0.0.1:{}", voltra_net::DEFAULT_WEBSOCKET_PORT)
            } else {
                format!("udp://127.0.0.1:{}", voltra_net::DEFAULT_UDP_PORT)
            },
        }
    }
}

impl LobbyScreenConfig {
    /// The default screen for the game `game`.
    pub fn for_game(game: &str) -> Self {
        Self {
            game: game.to_owned(),
            ..Self::default()
        }
    }
}

/// Why the screen could not open.
#[derive(Debug)]
pub enum LobbyScreenError {
    /// The prefab path is not an asset path.
    BadPath(String),
    /// The prefab would not spawn.
    Spawn(SceneError),
    /// The prefab spawned nothing: its file is missing or unreadable (the
    /// store has said why).
    Empty(String),
}

impl fmt::Display for LobbyScreenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadPath(path) => write!(f, "{path:?} is not an asset path"),
            Self::Spawn(e) => write!(f, "the lobby screen would not spawn: {e}"),
            Self::Empty(path) => write!(f, "{path} holds no lobby screen"),
        }
    }
}

impl std::error::Error for LobbyScreenError {}

/// What a frame of the screen came to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LobbyOutcome {
    /// Still showing.
    Open,
    /// The match started this frame: the game loads its level, spawns its
    /// players, and [closes](super::LobbyScreen::close) the screen — the
    /// session goes on.
    Started,
    /// Backed out of with the cancel action outside a lobby, and taken
    /// away.
    Closed,
}
