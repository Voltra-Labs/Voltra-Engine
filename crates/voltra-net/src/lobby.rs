//! The room before a match: who is in it, under what name, who is ready,
//! and who may start it.
//!
//! The engine owns the protocol and the state; the game decides what
//! "start" does (load a level, spawn a player per entry). A server holds
//! the [`Lobby`] — slots, entries in join order, ready flags, a host — and
//! is the only end that changes it; it sends every peer in the lobby the
//! whole of it whenever it changes, on a reliable-ordered channel of its
//! own ([`LobbyConfig::channel`]), so a lobby message never reaches the
//! game's own messages. A client mirrors what it was sent and asks: to
//! join under a name ([`LobbySession::set_name`]), to be ready, to start.
//! Both ends see the changes as [`LobbyEvent`]s, a frame's at a time.
//!
//! Untrusted input is bounded the way the rest of the crate bounds it:
//! a name is cut to [`LobbyConfig::max_name_chars`], stripped of control
//! and invisible formatting characters and made unique; a message that
//! will not decode is dropped and logged; a client that is refused (a full
//! lobby, a match under way) is told why and cut off after a grace.
//!
//! The lobby's wire is versioned ([`LOBBY_WIRE_VERSION`]) and checked at
//! the handshake: [`LobbyConfig::net_config`] puts it in the session's
//! fingerprint, so a client without the lobby, or with another build's,
//! is refused as [`ProtocolMismatch`](crate::DisconnectReason::ProtocolMismatch).

mod config;
mod error;
mod event;
mod name;
mod refusal;
mod session;
mod state;
mod wire;

pub use config::{LateJoin, LobbyConfig, MAX_LOBBY_PLAYERS, MAX_NAME_CHARS, MIN_JOIN_TIMEOUT};
pub use error::LobbyError;
pub use event::LobbyEvent;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) use name::visible_text;
pub use name::{sanitize_name, unique_name};
pub use refusal::LobbyRefusal;
pub use session::LobbySession;
pub use state::{Lobby, LobbyEntry, LobbyPhase};

/// The version of the lobby's messages. Bump it on any change to
/// `lobby/wire.rs`: it is in the handshake's fingerprint
/// ([`NetConfig::lobby_version`](crate::NetConfig::lobby_version)), so
/// builds that would misread each other's lobby never connect.
pub const LOBBY_WIRE_VERSION: u32 = 1;
