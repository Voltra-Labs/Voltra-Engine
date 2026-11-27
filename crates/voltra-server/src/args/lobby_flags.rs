//! The lobby's flags, laid over the lobby a game's server sets.

use crate::ServerOptions;
use voltra_core::net::LobbyConfig;

/// The lobby flags given, each `None` when not.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct LobbyFlags {
    /// `--lobby`, or any other lobby flag: hold one.
    pub on: bool,
    /// `--min-players`.
    pub min_players: Option<usize>,
    /// `--max-players`.
    pub max_players: Option<usize>,
}

impl LobbyFlags {
    /// `options` with these flags laid over its lobby — the engine's
    /// default one when the flags ask for a lobby and it has none.
    pub fn apply(&self, options: &mut ServerOptions) {
        if !self.on {
            return;
        }
        let lobby = options.lobby.get_or_insert_with(LobbyConfig::default);
        if let Some(min) = self.min_players {
            lobby.min_players = min;
        }
        if let Some(max) = self.max_players {
            lobby.max_players = max;
        }
    }
}
