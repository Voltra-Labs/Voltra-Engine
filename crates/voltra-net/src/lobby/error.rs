//! Why a lobby action was not taken.

use std::fmt;

/// Why a lobby action was not taken.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LobbyError {
    /// The session has no lobby, or it has not started yet.
    NoLobby,
    /// This player is not in the lobby (not yet let in, or refused).
    NotInLobby,
    /// Only the host starts the match.
    NotHost,
    /// The match is already being played.
    AlreadyPlaying,
    /// Not every player is ready.
    NotEveryoneReady,
    /// Fewer players than [`LobbyConfig::min_players`](crate::LobbyConfig::min_players).
    TooFewPlayers {
        /// Players in the lobby.
        have: usize,
        /// Players a match needs.
        need: usize,
    },
    /// Only a server finishes a match.
    NotServer,
}

impl fmt::Display for LobbyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoLobby => write!(f, "there is no lobby"),
            Self::NotInLobby => write!(f, "not in the lobby"),
            Self::NotHost => write!(f, "only the host starts the match"),
            Self::AlreadyPlaying => write!(f, "the match is already being played"),
            Self::NotEveryoneReady => write!(f, "not everyone is ready"),
            Self::TooFewPlayers { have, need } => {
                write!(f, "{have} of the {need} players a match needs")
            }
            Self::NotServer => write!(f, "only the server finishes a match"),
        }
    }
}

impl std::error::Error for LobbyError {}
