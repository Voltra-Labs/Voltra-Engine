//! How a lobby is run, and the channel it travels on.

use std::time::Duration;

use crate::{ChannelId, ChannelKind, NetConfig, DEFAULT_CHANNELS};

/// The most entries a lobby holds, whatever its config says: a snapshot
/// of the lobby stays a few kilobytes.
pub const MAX_LOBBY_PLAYERS: usize = 255;

/// The longest name a lobby takes, in characters, whatever its config
/// says.
pub const MAX_NAME_CHARS: usize = 64;

/// The shortest [`LobbyConfig::join_timeout`]: a client's hello needs a
/// round trip to arrive.
pub const MIN_JOIN_TIMEOUT: Duration = Duration::from_millis(250);

/// What a lobby does with a peer that asks to join while the match is
/// being played.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LateJoin {
    /// Refused with [`LobbyRefusal::MatchUnderway`](crate::LobbyRefusal::MatchUnderway):
    /// a match of fixed sides, a race that started together.
    #[default]
    Refuse,
    /// Let in, straight into the match: a
    /// [`LobbyEvent::Joined`](crate::LobbyEvent::Joined) while
    /// [`LobbyPhase::Playing`](crate::LobbyPhase::Playing), for the game to
    /// spawn a player for — drop-in, drop-out.
    Play,
}

/// How the lobby runs. Both ends make their session's config through
/// [`net_config`](Self::net_config); only the server's rules (slots, the
/// minimum, late joins) are read, and a client reads its own
/// [`name`](Self::name).
#[derive(Debug, Clone, PartialEq)]
pub struct LobbyConfig {
    /// The lobby's channel: reliable-ordered, its own, so the game's
    /// [`received`](crate::Endpoint::messages) never holds a lobby message.
    /// `6` by default — after the three default channels and the three
    /// replication takes.
    pub channel: ChannelId,
    /// Slots: the most players in the lobby at once, the listen host
    /// included. One more is refused with
    /// [`LobbyRefusal::Full`](crate::LobbyRefusal::Full). At most
    /// [`MAX_LOBBY_PLAYERS`].
    pub max_players: usize,
    /// The fewest players a match starts with.
    pub min_players: usize,
    /// The longest name, in characters; longer ones are cut. At most
    /// [`MAX_NAME_CHARS`].
    pub max_name_chars: usize,
    /// Whether a peer joining mid-match plays or is refused.
    pub late_join: LateJoin,
    /// This end's player's name, asked for on joining (and the listen
    /// host's own). Sanitised and made unique by the server.
    pub name: String,
    /// How long a refused peer is kept connected to read why before the
    /// server cuts it off. A client leaves on its own as soon as it reads
    /// the refusal.
    pub refusal_grace: Duration,
    /// How long a connected peer may take to join (its hello) before the
    /// server cuts it off, so a peer that never says who it is cannot hold
    /// a session slot. At least [`MIN_JOIN_TIMEOUT`].
    pub join_timeout: Duration,
}

impl Default for LobbyConfig {
    fn default() -> Self {
        Self {
            channel: ChannelId(DEFAULT_CHANNELS.len() as u8 + 3),
            max_players: 8,
            min_players: 2,
            max_name_chars: 16,
            late_join: LateJoin::Refuse,
            name: "Player".to_owned(),
            refusal_grace: Duration::from_secs(2),
            join_timeout: Duration::from_secs(5),
        }
    }
}

impl LobbyConfig {
    /// `base` with the lobby's channel in place — extended with
    /// reliable-ordered channels up to it if it is short — and
    /// [`LOBBY_WIRE_VERSION`](crate::LOBBY_WIRE_VERSION) as its
    /// [`lobby_version`](NetConfig::lobby_version). Both ends must make their
    /// session's config this way: the handshake's fingerprint covers both,
    /// so a client without the lobby is refused at the door.
    ///
    /// A game channel already at [`channel`](Self::channel) of another kind
    /// is overwritten, with a warning.
    pub fn net_config(&self, mut base: NetConfig) -> NetConfig {
        base.lobby_version = crate::LOBBY_WIRE_VERSION;
        let index = usize::from(self.channel.0);
        if base.channels.len() <= index {
            base.channels
                .resize(index + 1, ChannelKind::ReliableOrdered);
        } else if base.channels[index] != ChannelKind::ReliableOrdered {
            log::warn!(
                "channel {index} is the game's {:?}; the lobby takes it as reliable-ordered",
                base.channels[index]
            );
            base.channels[index] = ChannelKind::ReliableOrdered;
        }
        base
    }

    /// Whether a session made with `config` carries this lobby: its channel
    /// reliable-ordered and its wire version this build's.
    pub fn fits(&self, config: &NetConfig) -> bool {
        config.lobby_version == crate::LOBBY_WIRE_VERSION
            && config.channels.get(usize::from(self.channel.0))
                == Some(&ChannelKind::ReliableOrdered)
    }

    /// The config with every limit brought into range.
    pub(crate) fn validated(mut self) -> Self {
        self.max_players = self.max_players.clamp(1, MAX_LOBBY_PLAYERS);
        self.min_players = self.min_players.clamp(1, self.max_players);
        self.max_name_chars = self.max_name_chars.clamp(1, MAX_NAME_CHARS);
        self.join_timeout = self.join_timeout.max(MIN_JOIN_TIMEOUT);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_lobby_takes_a_channel_of_its_own_and_moves_the_fingerprint() {
        let lobby = LobbyConfig::default();
        let plain = NetConfig::default();
        let with = lobby.net_config(plain.clone());

        assert_eq!(with.channels.len(), 7);
        assert_eq!(with.channels[6], ChannelKind::ReliableOrdered);
        assert_eq!(with.channels[..3], plain.channels[..]);
        assert!(lobby.fits(&with));
        assert!(!lobby.fits(&plain));
        assert_ne!(with.fingerprint(), plain.fingerprint());

        let other_build = NetConfig {
            lobby_version: crate::LOBBY_WIRE_VERSION + 1,
            ..with.clone()
        };
        assert!(!lobby.fits(&other_build));
        assert_ne!(with.fingerprint(), other_build.fingerprint());
    }

    #[test]
    fn a_game_channel_in_the_way_is_taken_over() {
        let lobby = LobbyConfig {
            channel: ChannelId(1),
            ..LobbyConfig::default()
        };
        let mut base = NetConfig::default();
        base.channels[1] = ChannelKind::UnreliableSequenced;

        let config = lobby.net_config(base);

        assert_eq!(config.channels.len(), 3);
        assert!(lobby.fits(&config));
    }

    #[test]
    fn limits_are_brought_into_range() {
        let config = LobbyConfig {
            max_players: 10_000,
            min_players: 0,
            max_name_chars: 0,
            ..LobbyConfig::default()
        }
        .validated();
        assert_eq!(config.max_players, MAX_LOBBY_PLAYERS);
        assert_eq!(config.min_players, 1);
        assert_eq!(config.max_name_chars, 1);
        assert_eq!(
            LobbyConfig {
                join_timeout: Duration::ZERO,
                ..LobbyConfig::default()
            }
            .validated()
            .join_timeout,
            MIN_JOIN_TIMEOUT
        );

        let fewer_slots_than_needed = LobbyConfig {
            max_players: 2,
            min_players: 5,
            ..LobbyConfig::default()
        }
        .validated();
        assert_eq!(fewer_slots_than_needed.min_players, 2);
    }
}
