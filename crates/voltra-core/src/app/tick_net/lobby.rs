//! The lobby through `tick.net`: the game turns it on, the loop pumps it
//! with the session, and the game reads its events and asks through it.

use voltra_net::{LobbyConfig, LobbyEvent, LobbySession, NetConfig};

use super::Net;
use crate::app::App;

impl Net {
    /// Turns the lobby on for this session and every later one: before the
    /// match, the server holds the room — names, ready flags, a host who
    /// starts it — and every end reads it through [`lobby`](Self::lobby)
    /// and [`lobby_events`](Self::lobby_events). Both ends must turn it on
    /// before they host or join — it adds its channel and wire version to
    /// the session's config ([`LobbyConfig::net_config`]), which the
    /// handshake checks.
    pub fn enable_lobby(&mut self, config: LobbyConfig) {
        self.lobby_config = Some(config);
        self.lobby = None;
        self.lobby_refused = false;
    }

    /// Whether the lobby is on.
    pub fn has_lobby(&self) -> bool {
        self.lobby_config.is_some()
    }

    /// This session's lobby, from the first frame after it started: the
    /// room, this end's player in it, and what it may ask
    /// ([`LobbySession::set_ready`], [`LobbySession::start`], …). `None`
    /// with the lobby off, or no session.
    pub fn lobby(&self) -> Option<&LobbySession> {
        self.lobby.as_ref()
    }

    /// This session's lobby, to ask through it.
    pub fn lobby_mut(&mut self) -> Option<&mut LobbySession> {
        self.lobby.as_mut()
    }

    /// What changed in the lobby this frame: joins, leaves, ready flags, the
    /// match started. Per frame, like [`events`](Self::events).
    pub fn lobby_events(&self) -> &[LobbyEvent] {
        self.lobby.as_ref().map_or(&[], LobbySession::events)
    }

    /// The name this end's player joins the lobby under (or, once in, is
    /// renamed to): the server sanitises it and makes it unique.
    pub fn set_player_name(&mut self, name: &str) {
        if let Some(config) = self.lobby_config.as_mut() {
            config.name = name.to_owned();
        }
        if let Some(lobby) = self.lobby.as_mut() {
            lobby.set_name(name);
        }
    }

    /// `config` with the lobby's channel and version, when it is on.
    pub(in crate::app) fn lobby_session_config(&self, config: NetConfig) -> NetConfig {
        match &self.lobby_config {
            Some(lobby) => lobby.net_config(config),
            None => config,
        }
    }

    /// The lobby read after the session's receive: made the first time the
    /// session can carry it.
    pub(super) fn receive_lobby(&mut self) {
        let (Some(config), Some(endpoint)) = (&self.lobby_config, self.endpoint.as_mut()) else {
            return;
        };
        if self.lobby.is_none() && !self.lobby_refused {
            if config.fits(endpoint.config()) {
                self.lobby = Some(LobbySession::new(config.clone(), endpoint));
            } else {
                log::error!(
                    "the lobby is on, but this session was made without its channel \
                     (LobbyConfig::net_config); there is no lobby"
                );
                self.lobby_refused = true;
            }
        }
        if let Some(lobby) = self.lobby.as_mut() {
            lobby.receive(endpoint, self.now);
        }
    }

    /// What the lobby has to say, before the session's flush.
    pub(super) fn send_lobby(&mut self) {
        if let (Some(lobby), Some(endpoint)) = (self.lobby.as_mut(), self.endpoint.as_mut()) {
            lobby.send(endpoint);
        }
    }
}

impl App {
    /// Turns the lobby on before the first frame; see
    /// [`Net::enable_lobby`].
    pub fn with_lobby(mut self, config: LobbyConfig) -> Self {
        self.net.enable_lobby(config);
        self
    }
}
