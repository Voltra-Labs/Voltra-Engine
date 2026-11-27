//! The lobby's messages, as they travel (serde over `postcard`). Any
//! change here bumps [`LOBBY_WIRE_VERSION`](crate::LOBBY_WIRE_VERSION).

use serde::{Deserialize, Serialize};

use super::config::MAX_LOBBY_PLAYERS;
use super::name::sanitize_name;
use super::state::{Lobby, LobbyEntry, LobbyPhase, LobbyRefusal};
use super::MAX_NAME_CHARS;
use crate::PeerId;

/// A client asking.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum ToServer {
    /// Let me in under this name — or, once in, call me this.
    Hello {
        /// As typed: the server sanitises it.
        name: String,
    },
    /// I am ready, or not.
    Ready(bool),
    /// Start the match (the host's).
    Start,
}

/// The server telling.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum ToClient {
    /// The whole lobby, as it is now.
    State(Snapshot),
    /// You are not let in, and why.
    Refused(LobbyRefusal),
}

/// One entry on the wire.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct WireEntry {
    peer: u32,
    name: String,
    ready: bool,
}

/// The lobby on the wire.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Snapshot {
    max_players: u16,
    min_players: u16,
    host: Option<u32>,
    phase: LobbyPhase,
    entries: Vec<WireEntry>,
}

impl Snapshot {
    pub(crate) fn of(lobby: &Lobby) -> Self {
        Self {
            max_players: lobby.max_players().min(MAX_LOBBY_PLAYERS) as u16,
            min_players: lobby.min_players().min(MAX_LOBBY_PLAYERS) as u16,
            host: lobby.host().map(|peer| peer.0),
            phase: lobby.phase(),
            entries: lobby
                .entries()
                .iter()
                .map(|entry| WireEntry {
                    peer: entry.peer.0,
                    name: entry.name.clone(),
                    ready: entry.ready,
                })
                .collect(),
        }
    }

    /// The lobby it describes — `None` for one no honest server sends:
    /// more entries than slots or than any lobby holds, a peer twice, a
    /// host not in it. Names are sanitised again: a client shows them.
    pub(crate) fn into_lobby(self) -> Option<Lobby> {
        let max_players = usize::from(self.max_players);
        if max_players > MAX_LOBBY_PLAYERS || self.entries.len() > max_players {
            return None;
        }
        let mut entries: Vec<LobbyEntry> = Vec::with_capacity(self.entries.len());
        for entry in self.entries {
            let peer = PeerId(entry.peer);
            if entries.iter().any(|seen| seen.peer == peer) {
                return None;
            }
            entries.push(LobbyEntry {
                peer,
                name: sanitize_name(&entry.name, MAX_NAME_CHARS),
                ready: entry.ready,
            });
        }
        let host = self.host.map(PeerId);
        if host.is_some_and(|host| !entries.iter().any(|entry| entry.peer == host)) {
            return None;
        }
        Some(Lobby::from_parts(
            max_players,
            usize::from(self.min_players),
            host,
            entries,
            self.phase,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{decode, encode, LobbyConfig};

    fn lobby() -> Lobby {
        let config = LobbyConfig::default();
        let mut lobby = Lobby::new(&config);
        lobby.join(PeerId(1), "Ada", &config).expect("room");
        lobby.join(PeerId(2), "Bob", &config).expect("room");
        lobby.set_ready(PeerId(2), true).expect("in");
        lobby
    }

    #[test]
    fn a_lobby_round_trips() {
        let lobby = lobby();
        let bytes = encode(&ToClient::State(Snapshot::of(&lobby))).expect("encodes");
        let ToClient::State(back) = decode::<ToClient>(&bytes).expect("decodes") else {
            panic!("a state");
        };
        assert_eq!(back.into_lobby(), Some(lobby));
    }

    #[test]
    fn a_lobby_no_server_would_send_is_refused() {
        let mut twice = Snapshot::of(&lobby());
        twice.entries[1].peer = 1;
        assert_eq!(twice.into_lobby(), None);

        let mut stranger_hosts = Snapshot::of(&lobby());
        stranger_hosts.host = Some(9);
        assert_eq!(stranger_hosts.into_lobby(), None);

        let mut overfull = Snapshot::of(&lobby());
        overfull.max_players = 1;
        assert_eq!(overfull.into_lobby(), None);

        let mut shouting = Snapshot::of(&lobby());
        shouting.entries[0].name = "\u{1b}[2J".into();
        let shown = shouting.into_lobby().expect("a fine lobby");
        assert_eq!(shown.entries()[0].name, "[2J");
    }

    #[test]
    fn random_bytes_never_decode_into_a_panic() {
        let mut state = 0x1234_5678_u64;
        for len in 0..2000 {
            let bytes: Vec<u8> = (0..len % 97)
                .map(|_| {
                    state ^= state << 13;
                    state ^= state >> 7;
                    state ^= state << 17;
                    state as u8
                })
                .collect();
            if let Ok(ToClient::State(snapshot)) = decode::<ToClient>(&bytes) {
                let _ = snapshot.into_lobby();
            }
            let _ = decode::<ToServer>(&bytes);
        }
    }
}
