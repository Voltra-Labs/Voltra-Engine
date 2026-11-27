//! What changed in the lobby this frame.

use super::state::LobbyRefusal;
use crate::PeerId;

/// A change to the lobby, seen the same on the server and on every client
/// in it, in the frame it arrived. The game reacts to these: a name to
/// show, a player to spawn at [`Started`](Self::Started) (and at a
/// [`Joined`](Self::Joined) mid-match, with
/// [`LateJoin::Play`](crate::LateJoin::Play)), one to take away at
/// [`Left`](Self::Left).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LobbyEvent {
    /// `peer` is in, as `name`.
    Joined {
        /// Who.
        peer: PeerId,
        /// The name the lobby gave them.
        name: String,
    },
    /// `peer` is out: left, timed out, or kicked.
    Left {
        /// Who.
        peer: PeerId,
    },
    /// `peer` is now called `name`.
    Renamed {
        /// Who.
        peer: PeerId,
        /// The name the lobby gave them.
        name: String,
    },
    /// `peer` is ready, or no longer.
    ReadyChanged {
        /// Who.
        peer: PeerId,
        /// Whether they are ready now.
        ready: bool,
    },
    /// Someone else starts the match now — the host left — or nobody does.
    HostChanged(Option<PeerId>),
    /// The match started: the game loads its level, spawns its players.
    Started,
    /// The server's game ended the match: everyone is back in the lobby,
    /// nobody ready.
    Finished,
    /// The server would not let this client in, and why. The client leaves.
    Refused(LobbyRefusal),
    /// This client is no longer in a lobby: the session to the server
    /// ended (on a listen server, the host leaving ends it for everyone).
    Closed,
}
