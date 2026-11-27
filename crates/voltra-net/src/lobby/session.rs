//! The lobby of one session, pumped with it: the server's rules, or a
//! client's copy and what it asks for.

mod changes;
mod client;
mod server;

use std::time::Duration;

use super::config::LobbyConfig;
use super::error::LobbyError;
use super::event::LobbyEvent;
use super::refusal::LobbyRefusal;
use super::state::{Lobby, LobbyPhase};
use super::wire::ToServer;
use crate::{Endpoint, PeerId};
use changes::changes;
use client::Client;
use server::Server;

/// A session's lobby: the [`Lobby`], this frame's [`LobbyEvent`]s, and
/// this end's player's part in it.
///
/// Pumped like the endpoint under it — [`receive`](Self::receive) after
/// the endpoint's receive, [`send`](Self::send) before its flush (the
/// engine's loop does both for `tick.net`). What this end's player does
/// ([`set_ready`](Self::set_ready), [`start`](Self::start),
/// [`set_name`](Self::set_name)) is asked of the server on a client and
/// done at once on a server; either way the events it causes come with the
/// next receive, as everyone else's do.
#[derive(Debug)]
pub struct LobbySession {
    config: LobbyConfig,
    lobby: Lobby,
    events: Vec<LobbyEvent>,
    /// Events of this end's own actions, handed out at the next receive.
    pending: Vec<LobbyEvent>,
    role: Role,
}

#[derive(Debug)]
enum Role {
    Server(Server),
    Client(Client),
}

impl LobbySession {
    /// The lobby for `endpoint`'s session: a server's (empty, its host's
    /// own player joining with the next receive on a listen server) or a
    /// client's (asking in under [`LobbyConfig::name`] once connected).
    /// Made on a session already under way, it catches up: a listen
    /// host's player is in at once, a connected client asks in at the
    /// next send, and a server gives the clients already connected the
    /// join deadline from now.
    pub fn new(config: LobbyConfig, endpoint: &Endpoint) -> Self {
        let config = config.validated();
        let mut lobby = Lobby::new(&config);
        let mut pending = Vec::new();
        let role = if endpoint.is_server() {
            let mut server = Server {
                join_timeout: config.join_deadline(endpoint.config()),
                ..Server::default()
            };
            // Made on a session already hosting: its own player is in now,
            // not at a `Connected` this lobby will never see. Clients
            // already connected are given the join deadline from here.
            if endpoint.host_plays() {
                server.listen = true;
                let before = lobby.clone();
                if let Err(refusal) = lobby.join(PeerId::SERVER, &config.name, &config) {
                    log::warn!("the host's own player is not in the lobby: {refusal}");
                }
                lobby.make_host(PeerId::SERVER);
                server.dirty = true;
                changes(&before, &lobby, &mut pending);
            }
            Role::Server(server)
        } else {
            let mut client = Client::default();
            // Made on a session already connected: ask in now, as the
            // `Connected` it missed would have.
            if endpoint.is_connected(PeerId::SERVER) {
                client.connected = true;
                client.me = endpoint.local_id();
                client.outbox.push(ToServer::Hello {
                    name: config.name.clone(),
                });
            }
            Role::Client(client)
        };
        Self {
            lobby,
            config,
            events: Vec::new(),
            pending,
            role,
        }
    }

    /// The lobby: the server's, or a client's copy of the last it was sent.
    pub fn lobby(&self) -> &Lobby {
        &self.lobby
    }

    /// What changed since the last receive.
    pub fn events(&self) -> &[LobbyEvent] {
        &self.events
    }

    /// How it runs, with its limits in range.
    pub fn config(&self) -> &LobbyConfig {
        &self.config
    }

    /// Whether this end holds the lobby.
    pub fn is_server(&self) -> bool {
        matches!(self.role, Role::Server(_))
    }

    /// This end's own player: [`PeerId::SERVER`] on a listen server, the
    /// client's id once it is let in — `None` on a dedicated server, which
    /// has none, and on a client not yet connected.
    pub fn local_player(&self) -> Option<PeerId> {
        match &self.role {
            Role::Server(server) => server.listen.then_some(PeerId::SERVER),
            Role::Client(client) => client.me,
        }
    }

    /// Whether this end's player is in the lobby.
    pub fn is_in(&self) -> bool {
        self.local_player()
            .is_some_and(|peer| self.lobby.contains(peer))
    }

    /// Whether this end's player starts the match.
    pub fn is_host(&self) -> bool {
        self.local_player()
            .is_some_and(|peer| self.lobby.host() == Some(peer))
    }

    /// Whether this end's player is ready.
    pub fn is_ready(&self) -> bool {
        self.local_player()
            .and_then(|peer| self.lobby.entry(peer))
            .is_some_and(|entry| entry.ready)
    }

    /// Why the server would not let this client in, once it said.
    pub fn refusal(&self) -> Option<LobbyRefusal> {
        match &self.role {
            Role::Server(_) => None,
            Role::Client(client) => client.refused,
        }
    }

    /// The name this end's player asked for (the lobby's may differ: made
    /// unique, cut).
    pub fn name(&self) -> &str {
        &self.config.name
    }

    /// Asks to be called `name`: before joining, the name joined under;
    /// once in, a rename.
    pub fn set_name(&mut self, name: &str) {
        self.config.name = name.to_owned();
        match &mut self.role {
            Role::Server(server) => {
                if server.listen && self.lobby.contains(PeerId::SERVER) {
                    let before = self.lobby.clone();
                    if self.lobby.join(PeerId::SERVER, name, &self.config).is_ok() {
                        server.dirty = true;
                    }
                    changes(&before, &self.lobby, &mut self.pending);
                }
            }
            Role::Client(client) => {
                if client.connected {
                    client.outbox.push(ToServer::Hello {
                        name: name.to_owned(),
                    });
                }
            }
        }
    }

    /// Says whether this end's player is ready. Before the match, and only
    /// once in the lobby.
    pub fn set_ready(&mut self, ready: bool) -> Result<(), LobbyError> {
        let me = self.local_player().ok_or(LobbyError::NotInLobby)?;
        if !self.lobby.contains(me) {
            return Err(LobbyError::NotInLobby);
        }
        if self.lobby.phase() == LobbyPhase::Playing {
            return Err(LobbyError::AlreadyPlaying);
        }
        match &mut self.role {
            Role::Server(server) => {
                let before = self.lobby.clone();
                self.lobby.set_ready(me, ready)?;
                server.dirty = true;
                changes(&before, &self.lobby, &mut self.pending);
            }
            Role::Client(client) => client.outbox.push(ToServer::Ready(ready)),
        }
        Ok(())
    }

    /// Starts the match. A client's host asks the server, which checks
    /// again; a listen server's host starts it. A dedicated server's game
    /// may start it too — the server is the authority, and needs no
    /// host's word — when there are enough players and all are ready.
    pub fn start(&mut self) -> Result<(), LobbyError> {
        let me = self.local_player();
        match &mut self.role {
            Role::Server(server) => {
                let before = self.lobby.clone();
                self.lobby.start(me)?;
                server.dirty = true;
                changes(&before, &self.lobby, &mut self.pending);
            }
            Role::Client(client) => {
                self.lobby.can_start(me.ok_or(LobbyError::NotInLobby)?)?;
                client.outbox.push(ToServer::Start);
            }
        }
        Ok(())
    }

    /// Ends the match, on the server: everyone back in the lobby, nobody
    /// ready — the next round starts from there.
    pub fn finish(&mut self) -> Result<(), LobbyError> {
        let Role::Server(server) = &mut self.role else {
            return Err(LobbyError::NotServer);
        };
        let before = self.lobby.clone();
        if self.lobby.finish() {
            server.dirty = true;
        }
        changes(&before, &self.lobby, &mut self.pending);
        Ok(())
    }

    /// Reads what arrived on the lobby's channel and the session's joins
    /// and leaves: after the endpoint's receive, before the game reads the
    /// lobby. This frame's events replace the last frame's.
    pub fn receive(&mut self, endpoint: &mut Endpoint, now: Duration) {
        self.events = std::mem::take(&mut self.pending);
        let messages = endpoint.take_messages_on(&[self.config.channel]);
        let net_events = endpoint.events().to_vec();
        match &mut self.role {
            Role::Server(server) => server.receive(
                &mut self.lobby,
                &self.config,
                &mut self.events,
                endpoint,
                messages,
                &net_events,
                now,
            ),
            Role::Client(client) => client.receive(
                &mut self.lobby,
                &self.config,
                &mut self.events,
                endpoint,
                messages,
                &net_events,
            ),
        }
    }

    /// Sends what this end has to say: a server the lobby to everyone in
    /// it when it changed, a client what it asked. Before the endpoint's
    /// flush.
    pub fn send(&mut self, endpoint: &mut Endpoint) {
        let channel = self.config.channel;
        match &mut self.role {
            Role::Server(server) => server.send(&self.lobby, channel, endpoint),
            Role::Client(client) => client.send(channel, endpoint),
        }
    }
}
