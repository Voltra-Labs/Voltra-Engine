//! The lobby's messages, as they travel (serde over `postcard`). Any
//! change here bumps [`LOBBY_WIRE_VERSION`](crate::LOBBY_WIRE_VERSION).

use serde::{Deserialize, Serialize};

use super::config::MAX_LOBBY_PLAYERS;
use super::name::sanitize_name;
use super::refusal::LobbyRefusal;
use super::state::{Lobby, LobbyEntry, LobbyPhase};
use super::MAX_NAME_CHARS;
use crate::PeerId;

/// The bytes a snapshot takes at most beside its entries: the message's
/// tag, the slots, the minimum, the host, the phase, the matches and the
/// entries' count, each varint at its longest (with room to spare).
pub(crate) const SNAPSHOT_FIXED: usize = 32;

/// The bytes one entry takes at most beside its name's: its peer (a
/// `u32` varint, 5), the name's length (2 for any name a lobby takes) and
/// the ready flag.
pub(crate) const ENTRY_FIXED: usize = 8;

/// The largest snapshot of a lobby of `players` slots whose names are at
/// most `chars` characters, each at most four bytes of UTF-8.
pub(crate) fn largest_snapshot(players: usize, chars: usize) -> usize {
    SNAPSHOT_FIXED + players * (ENTRY_FIXED + 4 * chars)
}

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
    matches: u32,
    entries: Vec<WireEntry>,
}

impl Snapshot {
    pub(crate) fn of(lobby: &Lobby) -> Self {
        Self {
            max_players: lobby.max_players().min(MAX_LOBBY_PLAYERS) as u16,
            min_players: lobby.min_players().min(MAX_LOBBY_PLAYERS) as u16,
            host: lobby.host().map(|peer| peer.0),
            phase: lobby.phase(),
            matches: lobby.matches(),
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
            self.matches,
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
    fn a_lobby_at_every_cap_fits_its_bound_and_the_session_carries_it() {
        use crate::{LobbyConfig, NetConfig, MAX_LOBBY_PLAYERS, MAX_NAME_CHARS};
        let config = LobbyConfig {
            max_players: MAX_LOBBY_PLAYERS,
            min_players: 1,
            max_name_chars: MAX_NAME_CHARS,
            ..LobbyConfig::default()
        }
        .validated();
        let mut lobby = Lobby::new(&config);
        // Four-byte characters, each name unique and as long as allowed;
        // peers with the longest varints.
        for n in 0..config.max_players {
            let name: String = std::iter::repeat_n('😀', config.max_name_chars - 3)
                .chain(format!("{n:03}").chars())
                .collect();
            let peer = PeerId(u32::MAX - n as u32);
            let given = lobby.join(peer, &name, &config).expect("room");
            assert_eq!(given, name, "the name kept whole");
            lobby.set_ready(peer, true).expect("in");
        }
        lobby.start(Some(PeerId(u32::MAX))).expect("startable");
        let bytes = encode(&ToClient::State(Snapshot::of(&lobby))).expect("encodes");
        let bound = largest_snapshot(config.max_players, config.max_name_chars);
        assert!(bytes.len() <= bound, "{} > {bound}", bytes.len());
        assert!(bound <= crate::lobby::config::MAX_LOBBY_SNAPSHOT);

        let session = config.net_config(NetConfig {
            max_message_size: 1024,
            ..NetConfig::default()
        });
        assert!(
            session.max_message_size >= bound,
            "a game's smaller message limit is raised to the lobby's"
        );
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
