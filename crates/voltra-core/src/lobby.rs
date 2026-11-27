//! A lobby screen every networked game can open: host, find a game on the
//! LAN, join by address, a name, the players, ready, start.
//!
//! `assets/prefabs/lobby.prefab.ron` is the screen, built from the UI
//! controls — buttons, a toggle, two text fields, scrolling lists — and
//! found by the names of its parts, as the options screen is
//! ([`crate::options`]): a game restyles it by editing the file.
//! [`LobbyScreen`] opens it from a tick and drives `tick.net` from it —
//! [`host`](crate::Net::host) with LAN discovery on,
//! [`discover`](crate::Net::discover), [`join`](crate::Net::join), the
//! lobby's [ready](voltra_net::LobbySession::set_ready) and
//! [start](voltra_net::LobbySession::start) — and answers
//! [`LobbyOutcome::Started`] when the match begins, for the game to load
//! its level and take the screen away. What "start" does is the game's.

mod screen;

pub use screen::{LobbyOutcome, LobbyScreen, LobbyScreenConfig, LobbyScreenError};

/// The lobby screen's prefab, under the asset root.
pub const LOBBY_PREFAB: &str = "prefabs/lobby.prefab.ron";
