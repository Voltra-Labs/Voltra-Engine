//! Why the server would not let a peer in.

use std::fmt;

use serde::{Deserialize, Serialize};

/// Why the server would not let a peer in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LobbyRefusal {
    /// Every slot is taken.
    Full,
    /// The match has started, and the lobby takes no late joins
    /// ([`LateJoin::Refuse`](crate::LateJoin::Refuse)).
    MatchUnderway,
}

impl fmt::Display for LobbyRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Full => write!(f, "the lobby is full"),
            Self::MatchUnderway => write!(f, "the match has already started"),
        }
    }
}
