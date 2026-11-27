//! The lobby itself: slots, entries in join order, ready flags, a host —
//! and the rules that change them, run on the server.

use serde::{Deserialize, Serialize};

use super::config::{LateJoin, LobbyConfig};
use super::error::LobbyError;
use super::name::{same_name, sanitize_name, unique_name};
use super::refusal::LobbyRefusal;
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
    /// Matches started, wrapping.
    matches: u32,
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
            matches: 0,
        }
    }

    /// A lobby as a server sent it.
    pub(crate) fn from_parts(
        max_players: usize,
        min_players: usize,
        host: Option<PeerId>,
        entries: Vec<LobbyEntry>,
        phase: LobbyPhase,
        matches: u32,
    ) -> Self {
        Self {
            max_players,
            min_players,
            host,
            entries,
            phase,
            matches,
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

    /// How many matches this lobby has started (wrapping): a client tells
    /// by it that a match started and ended between two lobbies it was
    /// sent.
    pub fn matches(&self) -> u32 {
        self.matches
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
        let max_chars = super::config::name_cap(self.max_players, config.max_name_chars);
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
        self.matches = self.matches.wrapping_add(1);
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
mod tests;
