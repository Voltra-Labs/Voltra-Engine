//! The lobby itself: slots, entries in join order, ready flags, a host —
//! and the rules that change them, run on the server.

use std::fmt;

use serde::{Deserialize, Serialize};

use super::config::{LateJoin, LobbyConfig};
use super::name::{same_name, sanitize_name, unique_name};
use crate::PeerId;

/// Whether the match is being played.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum LobbyPhase {
    /// Before the match: players join, get ready, and the host starts it.
    #[default]
    Waiting,
    /// The host started it; the game is playing. Back to
    /// [`Waiting`](Self::Waiting) when the server's game
    /// [finishes](crate::LobbySession::finish) it.
    Playing,
}

/// One player in the lobby.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LobbyEntry {
    /// Who: [`PeerId::SERVER`] for a listen host's own player.
    pub peer: PeerId,
    /// What the others see: sanitised and unique in the lobby.
    pub name: String,
    /// Whether they are ready to start.
    pub ready: bool,
}

/// Why the server would not let a peer in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LobbyRefusal {
    /// Every slot is taken.
    Full,
    /// The match has started, and the lobby takes no late joins
    /// ([`LateJoin::Refuse`]).
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
    /// Fewer players than [`LobbyConfig::min_players`].
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

/// The lobby: who is in it, and who may start the match.
///
/// The server's is the lobby; a client's is a copy of the last the server
/// sent. Entries are in the order they joined. The host — who starts the
/// match — is a listen server's own player for as long as it hosts; on a
/// dedicated server it is the player who joined first, passed to the next
/// in join order when they leave.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lobby {
    max_players: usize,
    min_players: usize,
    host: Option<PeerId>,
    entries: Vec<LobbyEntry>,
    phase: LobbyPhase,
}

impl Lobby {
    /// An empty lobby with `config`'s slots and minimum.
    pub fn new(config: &LobbyConfig) -> Self {
        let config = config.clone().validated();
        Self {
            max_players: config.max_players,
            min_players: config.min_players,
            host: None,
            entries: Vec::new(),
            phase: LobbyPhase::Waiting,
        }
    }

    /// A lobby as a server sent it.
    pub(crate) fn from_parts(
        max_players: usize,
        min_players: usize,
        host: Option<PeerId>,
        entries: Vec<LobbyEntry>,
        phase: LobbyPhase,
    ) -> Self {
        Self {
            max_players,
            min_players,
            host,
            entries,
            phase,
        }
    }

    /// The players, in the order they joined.
    pub fn entries(&self) -> &[LobbyEntry] {
        &self.entries
    }

    /// `peer`'s entry, if they are in.
    pub fn entry(&self, peer: PeerId) -> Option<&LobbyEntry> {
        self.entries.iter().find(|entry| entry.peer == peer)
    }

    /// Whether `peer` is in.
    pub fn contains(&self, peer: PeerId) -> bool {
        self.entry(peer).is_some()
    }

    /// Who starts the match; `None` in an empty lobby.
    pub fn host(&self) -> Option<PeerId> {
        self.host
    }

    /// Before the match, or in it.
    pub fn phase(&self) -> LobbyPhase {
        self.phase
    }

    /// The slots.
    pub fn max_players(&self) -> usize {
        self.max_players
    }

    /// The fewest players a match starts with.
    pub fn min_players(&self) -> usize {
        self.min_players
    }

    /// Players in the lobby.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether nobody is in.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Whether every slot is taken.
    pub fn is_full(&self) -> bool {
        self.entries.len() >= self.max_players
    }

    /// Whether every player is ready (and there is one).
    pub fn all_ready(&self) -> bool {
        !self.entries.is_empty() && self.entries.iter().all(|entry| entry.ready)
    }

    /// Whether `by` could start the match now, and why not.
    pub fn can_start(&self, by: PeerId) -> Result<(), LobbyError> {
        if !self.contains(by) {
            return Err(LobbyError::NotInLobby);
        }
        if self.host != Some(by) {
            return Err(LobbyError::NotHost);
        }
        self.ready_to_start()
    }

    /// Whether the lobby is in a state a match starts from, whoever asks:
    /// waiting, enough players, every one ready.
    pub fn ready_to_start(&self) -> Result<(), LobbyError> {
        if self.phase == LobbyPhase::Playing {
            return Err(LobbyError::AlreadyPlaying);
        }
        if self.entries.len() < self.min_players {
            return Err(LobbyError::TooFewPlayers {
                have: self.entries.len(),
                need: self.min_players,
            });
        }
        if !self.all_ready() {
            return Err(LobbyError::NotEveryoneReady);
        }
        Ok(())
    }

    /// Lets `peer` in under `raw_name` (sanitised, then made unique), or
    /// renames them if they are in. The first player in becomes the host
    /// when there is none. Answers the name they were given.
    pub(crate) fn join(
        &mut self,
        peer: PeerId,
        raw_name: &str,
        config: &LobbyConfig,
    ) -> Result<String, LobbyRefusal> {
        let max_chars = config.max_name_chars.clamp(1, super::MAX_NAME_CHARS);
        let wanted = sanitize_name(raw_name, max_chars);
        let name = unique_name(&wanted, max_chars, |name| {
            self.entries
                .iter()
                .any(|entry| entry.peer != peer && same_name(&entry.name, name))
        });
        if let Some(entry) = self.entries.iter_mut().find(|entry| entry.peer == peer) {
            entry.name.clone_from(&name);
            return Ok(name);
        }
        if self.phase == LobbyPhase::Playing && config.late_join == LateJoin::Refuse {
            return Err(LobbyRefusal::MatchUnderway);
        }
        if self.is_full() {
            return Err(LobbyRefusal::Full);
        }
        self.entries.push(LobbyEntry {
            peer,
            name: name.clone(),
            // Into a match being played: in it, as everyone is.
            ready: self.phase == LobbyPhase::Playing,
        });
        if self.host.is_none() {
            self.host = Some(peer);
        }
        Ok(name)
    }

    /// Takes `peer` out. The host leaving passes the start to whoever
    /// joined next — on a listen server that cannot happen while it hosts,
    /// since its own player leaves only as the session ends. Answers the
    /// entry taken out, if they were in.
    pub(crate) fn leave(&mut self, peer: PeerId) -> Option<LobbyEntry> {
        let index = self.entries.iter().position(|entry| entry.peer == peer)?;
        let entry = self.entries.remove(index);
        if self.host == Some(peer) {
            self.host = self.entries.first().map(|entry| entry.peer);
        }
        Some(entry)
    }

    /// Makes `peer`, who must be in, the host: a listen server's own
    /// player hosts its lobby however early a client's join arrived.
    pub(crate) fn make_host(&mut self, peer: PeerId) {
        if self.contains(peer) {
            self.host = Some(peer);
        }
    }

    /// Sets `peer`'s ready flag before the match; whether it changed.
    pub(crate) fn set_ready(&mut self, peer: PeerId, ready: bool) -> Result<bool, LobbyError> {
        if self.phase == LobbyPhase::Playing {
            return Err(LobbyError::AlreadyPlaying);
        }
        let entry = self
            .entries
            .iter_mut()
            .find(|entry| entry.peer == peer)
            .ok_or(LobbyError::NotInLobby)?;
        let changed = entry.ready != ready;
        entry.ready = ready;
        Ok(changed)
    }

    /// Starts the match: by the host (`Some`), or by the server itself
    /// (`None`), which needs no host's word but the same readiness.
    pub(crate) fn start(&mut self, by: Option<PeerId>) -> Result<(), LobbyError> {
        match by {
            Some(peer) => self.can_start(peer)?,
            None => self.ready_to_start()?,
        }
        self.phase = LobbyPhase::Playing;
        Ok(())
    }

    /// Ends the match: back to waiting, nobody ready. Whether it was being
    /// played.
    pub(crate) fn finish(&mut self) -> bool {
        if self.phase == LobbyPhase::Waiting {
            return false;
        }
        self.phase = LobbyPhase::Waiting;
        for entry in &mut self.entries {
            entry.ready = false;
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(max: usize, min: usize) -> LobbyConfig {
        LobbyConfig {
            max_players: max,
            min_players: min,
            ..LobbyConfig::default()
        }
    }

    fn peer(n: u32) -> PeerId {
        PeerId(n)
    }

    #[test]
    fn the_first_in_hosts_and_the_host_passes_on_in_join_order() {
        let config = config(4, 1);
        let mut lobby = Lobby::new(&config);
        assert_eq!(lobby.host(), None);

        for n in 1..=3 {
            lobby.join(peer(n), "p", &config).expect("room");
        }
        assert_eq!(lobby.host(), Some(peer(1)));

        lobby.leave(peer(1)).expect("was in");
        assert_eq!(lobby.host(), Some(peer(2)));
        lobby.leave(peer(3)).expect("was in");
        assert_eq!(lobby.host(), Some(peer(2)));
        lobby.leave(peer(2)).expect("was in");
        assert_eq!(lobby.host(), None);
        assert!(lobby.leave(peer(2)).is_none(), "gone already");
    }

    #[test]
    fn names_are_sanitised_and_made_unique() {
        let config = LobbyConfig {
            max_name_chars: 8,
            ..config(4, 1)
        };
        let mut lobby = Lobby::new(&config);

        assert_eq!(lobby.join(peer(1), "Ada\u{7}", &config), Ok("Ada".into()));
        assert_eq!(lobby.join(peer(2), "ada", &config), Ok("ada 2".into()));
        assert_eq!(
            lobby.join(peer(3), "a very long name", &config),
            Ok("a very l".into())
        );
        // A rename keeps a name of one's own, and takes no one else's.
        assert_eq!(lobby.join(peer(2), "Ada", &config), Ok("Ada 2".into()));
        assert_eq!(lobby.join(peer(1), "Ada", &config), Ok("Ada".into()));
        assert_eq!(lobby.len(), 3, "a rename is not a second entry");
    }

    #[test]
    fn a_full_lobby_refuses_and_frees_a_slot_when_one_leaves() {
        let config = config(2, 1);
        let mut lobby = Lobby::new(&config);
        lobby.join(peer(1), "a", &config).expect("room");
        lobby.join(peer(2), "b", &config).expect("room");

        assert_eq!(lobby.join(peer(3), "c", &config), Err(LobbyRefusal::Full));
        assert!(lobby.is_full());
        lobby.leave(peer(1));
        assert!(lobby.join(peer(3), "c", &config).is_ok());
    }

    #[test]
    fn the_host_starts_once_everyone_is_ready_and_there_are_enough() {
        let config = config(4, 2);
        let mut lobby = Lobby::new(&config);
        lobby.join(peer(1), "a", &config).expect("room");

        assert_eq!(lobby.set_ready(peer(1), true), Ok(true));
        assert_eq!(lobby.set_ready(peer(1), true), Ok(false), "no change");
        assert_eq!(
            lobby.start(Some(peer(1))),
            Err(LobbyError::TooFewPlayers { have: 1, need: 2 })
        );
        lobby.join(peer(2), "b", &config).expect("room");
        assert_eq!(
            lobby.start(Some(peer(1))),
            Err(LobbyError::NotEveryoneReady)
        );
        lobby.set_ready(peer(2), true).expect("in");
        assert_eq!(lobby.start(Some(peer(2))), Err(LobbyError::NotHost));
        assert_eq!(lobby.start(Some(peer(9))), Err(LobbyError::NotInLobby));
        assert_eq!(lobby.start(Some(peer(1))), Ok(()));
        assert_eq!(lobby.phase(), LobbyPhase::Playing);
        assert_eq!(lobby.start(Some(peer(1))), Err(LobbyError::AlreadyPlaying));
        assert_eq!(
            lobby.set_ready(peer(1), false),
            Err(LobbyError::AlreadyPlaying)
        );

        assert!(lobby.finish());
        assert!(!lobby.finish(), "finished already");
        assert_eq!(lobby.phase(), LobbyPhase::Waiting);
        assert!(lobby.entries().iter().all(|entry| !entry.ready));
    }

    #[test]
    fn a_ready_player_leaving_can_make_the_rest_startable_or_not() {
        let config = config(4, 2);
        let mut lobby = Lobby::new(&config);
        for n in 1..=3 {
            lobby.join(peer(n), "p", &config).expect("room");
        }
        lobby.set_ready(peer(1), true).expect("in");
        lobby.set_ready(peer(2), true).expect("in");
        assert_eq!(lobby.ready_to_start(), Err(LobbyError::NotEveryoneReady));

        // The one not ready leaves: the rest can go.
        lobby.leave(peer(3));
        assert_eq!(lobby.ready_to_start(), Ok(()));
        // One more leaves: too few.
        lobby.leave(peer(2));
        assert_eq!(
            lobby.ready_to_start(),
            Err(LobbyError::TooFewPlayers { have: 1, need: 2 })
        );
    }

    #[test]
    fn a_late_joiner_is_refused_or_plays_as_configured() {
        let refuse = config(4, 1);
        let mut lobby = Lobby::new(&refuse);
        lobby.join(peer(1), "a", &refuse).expect("room");
        lobby.set_ready(peer(1), true).expect("in");
        lobby.start(None).expect("the server may start it");

        assert_eq!(
            lobby.join(peer(2), "b", &refuse),
            Err(LobbyRefusal::MatchUnderway)
        );

        let play = LobbyConfig {
            late_join: LateJoin::Play,
            ..refuse
        };
        assert!(lobby.join(peer(2), "b", &play).is_ok());
        assert!(lobby.entry(peer(2)).is_some_and(|entry| entry.ready));
    }

    #[test]
    fn a_player_not_in_cannot_be_ready() {
        let config = config(4, 1);
        let mut lobby = Lobby::new(&config);
        assert_eq!(lobby.set_ready(peer(1), true), Err(LobbyError::NotInLobby));
        assert!(!lobby.all_ready(), "an empty lobby is not ready");
    }
}
