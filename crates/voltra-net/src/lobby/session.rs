//! The lobby of one session, pumped with it: the server's rules, or a
//! client's copy and what it asks for.

use std::time::Duration;

use super::config::LobbyConfig;
use super::event::LobbyEvent;
use super::state::{Lobby, LobbyError, LobbyPhase, LobbyRefusal};
use super::wire::{Snapshot, ToClient, ToServer};
use crate::{Endpoint, NetEvent, PeerId};

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
    Server {
        /// Whether the host plays, as the lobby last saw it.
        listen: bool,
        /// Whether the lobby changed since it was last sent.
        dirty: bool,
        /// Peers refused, and when they are cut off if still here.
        refused: Vec<(PeerId, Duration)>,
    },
    Client {
        /// This client's id, once the server let it in.
        me: Option<PeerId>,
        /// What it asks, sent at the next flush.
        outbox: Vec<ToServer>,
        /// Why the server would not let it in, if it would not.
        refused: Option<LobbyRefusal>,
        /// Whether it is connected, to tell its lobby closing.
        connected: bool,
    },
}

impl LobbySession {
    /// The lobby for `endpoint`'s session: a server's (empty, its host's
    /// own player joining with the next receive on a listen server) or a
    /// client's (asking in under [`LobbyConfig::name`] once connected).
    pub fn new(config: LobbyConfig, endpoint: &Endpoint) -> Self {
        let config = config.validated();
        let role = if endpoint.is_server() {
            Role::Server {
                listen: false,
                dirty: false,
                refused: Vec::new(),
            }
        } else {
            Role::Client {
                me: None,
                outbox: Vec::new(),
                refused: None,
                connected: false,
            }
        };
        Self {
            lobby: Lobby::new(&config),
            config,
            events: Vec::new(),
            pending: Vec::new(),
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
        matches!(self.role, Role::Server { .. })
    }

    /// This end's own player: [`PeerId::SERVER`] on a listen server, the
    /// client's id once it is let in — `None` on a dedicated server, which
    /// has none, and on a client not yet connected.
    pub fn local_player(&self) -> Option<PeerId> {
        match &self.role {
            Role::Server { listen, .. } => listen.then_some(PeerId::SERVER),
            Role::Client { me, .. } => *me,
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
            Role::Server { .. } => None,
            Role::Client { refused, .. } => *refused,
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
            Role::Server { listen, dirty, .. } => {
                if *listen && self.lobby.contains(PeerId::SERVER) {
                    let before = self.lobby.clone();
                    if self.lobby.join(PeerId::SERVER, name, &self.config).is_ok() {
                        *dirty = true;
                    }
                    changes(&before, &self.lobby, &mut self.pending);
                }
            }
            Role::Client {
                outbox, connected, ..
            } => {
                if *connected {
                    outbox.push(ToServer::Hello {
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
            Role::Server { dirty, .. } => {
                let before = self.lobby.clone();
                self.lobby.set_ready(me, ready)?;
                *dirty = true;
                changes(&before, &self.lobby, &mut self.pending);
            }
            Role::Client { outbox, .. } => outbox.push(ToServer::Ready(ready)),
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
            Role::Server { dirty, .. } => {
                let before = self.lobby.clone();
                self.lobby.start(me)?;
                *dirty = true;
                changes(&before, &self.lobby, &mut self.pending);
            }
            Role::Client { outbox, .. } => {
                self.lobby.can_start(me.ok_or(LobbyError::NotInLobby)?)?;
                outbox.push(ToServer::Start);
            }
        }
        Ok(())
    }

    /// Ends the match, on the server: everyone back in the lobby, nobody
    /// ready — the next round starts from there.
    pub fn finish(&mut self) -> Result<(), LobbyError> {
        let Role::Server { dirty, .. } = &mut self.role else {
            return Err(LobbyError::NotServer);
        };
        let before = self.lobby.clone();
        if self.lobby.finish() {
            *dirty = true;
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
            Role::Server {
                listen,
                dirty,
                refused,
            } => {
                let before = self.lobby.clone();
                for event in &net_events {
                    match *event {
                        NetEvent::Connected(PeerId::SERVER) => {
                            *listen = true;
                            if let Err(refusal) =
                                self.lobby
                                    .join(PeerId::SERVER, &self.config.name, &self.config)
                            {
                                log::warn!("the host's own player is not in the lobby: {refusal}");
                            }
                            self.lobby.make_host(PeerId::SERVER);
                        }
                        NetEvent::Connected(_) => {}
                        NetEvent::Disconnected { peer, .. } => {
                            if peer == PeerId::SERVER {
                                *listen = false;
                            }
                            refused.retain(|(gone, _)| *gone != peer);
                            self.lobby.leave(peer);
                        }
                    }
                }
                for message in messages {
                    let peer = message.from;
                    if refused.iter().any(|(r, _)| *r == peer) {
                        continue;
                    }
                    let asked = match message.decode::<ToServer>() {
                        Ok(asked) => asked,
                        Err(error) => {
                            log::warn!(
                                "{peer} sent a lobby message that is not one ({error}); kicked"
                            );
                            endpoint.kick(peer);
                            self.lobby.leave(peer);
                            continue;
                        }
                    };
                    match asked {
                        ToServer::Hello { name } => {
                            match self.lobby.join(peer, &name, &self.config) {
                                Ok(given) => log::info!("lobby: {peer} is {given:?}"),
                                Err(refusal) => {
                                    log::info!("lobby: {peer} refused: {refusal}");
                                    if let Err(error) = endpoint.send_message(
                                        peer,
                                        self.config.channel,
                                        &ToClient::Refused(refusal),
                                    ) {
                                        log::warn!("could not tell {peer} why: {error}");
                                    }
                                    refused.push((peer, now + self.config.refusal_grace));
                                }
                            }
                        }
                        ToServer::Ready(ready) => {
                            if let Err(error) = self.lobby.set_ready(peer, ready) {
                                log::debug!("lobby: {peer} cannot be ready: {error}");
                            }
                        }
                        ToServer::Start => match self.lobby.start(Some(peer)) {
                            Ok(()) => log::info!("lobby: {peer} started the match"),
                            Err(error) => log::debug!("lobby: {peer} cannot start: {error}"),
                        },
                    }
                }
                refused.retain(|&(peer, deadline)| {
                    if !endpoint.is_connected(peer) {
                        return false;
                    }
                    if now >= deadline {
                        endpoint.kick(peer);
                        return false;
                    }
                    true
                });
                if before != self.lobby {
                    *dirty = true;
                }
                changes(&before, &self.lobby, &mut self.events);
            }
            Role::Client {
                me,
                outbox,
                refused,
                connected,
            } => {
                let before = self.lobby.clone();
                for event in &net_events {
                    match event {
                        NetEvent::Connected(_) => {
                            *connected = true;
                            *me = endpoint.local_id();
                            outbox.push(ToServer::Hello {
                                name: self.config.name.clone(),
                            });
                        }
                        NetEvent::Disconnected { .. } => {
                            *connected = false;
                            self.lobby = Lobby::new(&self.config);
                            self.events.push(LobbyEvent::Closed);
                            outbox.clear();
                        }
                    }
                }
                for message in messages {
                    match message.decode::<ToClient>() {
                        Ok(ToClient::State(snapshot)) => match snapshot.into_lobby() {
                            Some(lobby) => self.lobby = lobby,
                            None => log::warn!("the server sent a lobby that cannot be; ignored"),
                        },
                        Ok(ToClient::Refused(refusal)) => {
                            log::info!("lobby: refused: {refusal}");
                            *refused = Some(refusal);
                            self.events.push(LobbyEvent::Refused(refusal));
                            endpoint.disconnect(PeerId::SERVER);
                        }
                        Err(error) => {
                            log::warn!("the server sent a lobby message that is not one: {error}");
                        }
                    }
                }
                // A close resets the copy; the entries leaving it are not
                // worth an event each — `Closed` says it all.
                if *connected {
                    changes(&before, &self.lobby, &mut self.events);
                }
            }
        }
    }

    /// Sends what this end has to say: a server the lobby to everyone in
    /// it when it changed, a client what it asked. Before the endpoint's
    /// flush.
    pub fn send(&mut self, endpoint: &mut Endpoint) {
        let channel = self.config.channel;
        match &mut self.role {
            Role::Server { dirty, .. } => {
                if !*dirty {
                    return;
                }
                *dirty = false;
                let state = ToClient::State(Snapshot::of(&self.lobby));
                for entry in self.lobby.entries() {
                    if entry.peer == PeerId::SERVER {
                        continue;
                    }
                    if let Err(error) = endpoint.send_message(entry.peer, channel, &state) {
                        log::warn!("the lobby could not reach {}: {error}", entry.peer);
                    }
                }
            }
            Role::Client { outbox, .. } => {
                for asked in outbox.drain(..) {
                    if let Err(error) = endpoint.send_message(PeerId::SERVER, channel, &asked) {
                        log::warn!("the lobby could not reach the server: {error}");
                    }
                }
            }
        }
    }
}

/// The events that take `before` to `after`: who left, who joined or
/// changed, who hosts, whether the match began or ended — in that order,
/// so a game reading `Started` already has every `Joined`.
fn changes(before: &Lobby, after: &Lobby, events: &mut Vec<LobbyEvent>) {
    for gone in before.entries() {
        if !after.contains(gone.peer) {
            events.push(LobbyEvent::Left { peer: gone.peer });
        }
    }
    for entry in after.entries() {
        match before.entry(entry.peer) {
            None => events.push(LobbyEvent::Joined {
                peer: entry.peer,
                name: entry.name.clone(),
            }),
            Some(was) => {
                if was.name != entry.name {
                    events.push(LobbyEvent::Renamed {
                        peer: entry.peer,
                        name: entry.name.clone(),
                    });
                }
                if was.ready != entry.ready && after.phase() == before.phase() {
                    events.push(LobbyEvent::ReadyChanged {
                        peer: entry.peer,
                        ready: entry.ready,
                    });
                }
            }
        }
    }
    if before.host() != after.host() {
        events.push(LobbyEvent::HostChanged(after.host()));
    }
    match (before.phase(), after.phase()) {
        (LobbyPhase::Waiting, LobbyPhase::Playing) => events.push(LobbyEvent::Started),
        (LobbyPhase::Playing, LobbyPhase::Waiting) => events.push(LobbyEvent::Finished),
        _ => {}
    }
}
