//! The server's side of a lobby: the rules, run on what its peers ask.

use std::collections::HashMap;
use std::time::Duration;

use super::super::config::LobbyConfig;
use super::super::event::LobbyEvent;
use super::super::state::Lobby;
use super::super::wire::{Snapshot, ToClient, ToServer};
use super::changes::changes;
use crate::{ChannelId, Endpoint, Message, NetEvent, PeerId};

/// What a server keeps beside its lobby.
#[derive(Debug, Default)]
pub(super) struct Server {
    /// Whether the host plays, as the lobby last saw it.
    pub(super) listen: bool,
    /// Whether the lobby changed since it was last sent.
    pub(super) dirty: bool,
    /// Peers refused, and when they are cut off if still here.
    pub(super) refused: Vec<(PeerId, Duration)>,
    /// Peers connected but not yet in, and when they are cut off if they
    /// have still not said hello ([`LobbyConfig::join_timeout`]).
    pub(super) waiting: HashMap<PeerId, Duration>,
}

impl Server {
    /// The frame's joins, leaves and requests, applied to `lobby`.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn receive(
        &mut self,
        lobby: &mut Lobby,
        config: &LobbyConfig,
        events: &mut Vec<LobbyEvent>,
        endpoint: &mut Endpoint,
        messages: Vec<Message>,
        net_events: &[NetEvent],
        now: Duration,
    ) {
        let before = lobby.clone();
        for event in net_events {
            match *event {
                NetEvent::Connected(PeerId::SERVER) => {
                    self.listen = true;
                    if let Err(refusal) = lobby.join(PeerId::SERVER, &config.name, config) {
                        log::warn!("the host's own player is not in the lobby: {refusal}");
                    }
                    lobby.make_host(PeerId::SERVER);
                }
                NetEvent::Connected(_) => {}
                NetEvent::Disconnected { peer, .. } => {
                    if peer == PeerId::SERVER {
                        self.listen = false;
                    }
                    self.refused.retain(|(gone, _)| *gone != peer);
                    self.waiting.remove(&peer);
                    lobby.leave(peer);
                }
            }
        }
        for message in messages {
            let peer = message.from;
            // Gone in this same receive (its hello and its disconnect in
            // one frame): what it asked dies with it, or it would be an
            // entry nobody ever takes out.
            if !endpoint.is_connected(peer) || self.refused.iter().any(|(r, _)| *r == peer) {
                continue;
            }
            let asked = match message.decode::<ToServer>() {
                Ok(asked) => asked,
                Err(error) => {
                    log::warn!("{peer} sent a lobby message that is not one ({error}); kicked");
                    endpoint.kick(peer);
                    lobby.leave(peer);
                    continue;
                }
            };
            match asked {
                ToServer::Hello { name } => match lobby.join(peer, &name, config) {
                    Ok(given) => {
                        self.waiting.remove(&peer);
                        log::info!("lobby: {peer} is {given:?}");
                    }
                    Err(refusal) => {
                        self.waiting.remove(&peer);
                        log::info!("lobby: {peer} refused: {refusal}");
                        if let Err(error) =
                            endpoint.send_message(peer, config.channel, &ToClient::Refused(refusal))
                        {
                            log::warn!("could not tell {peer} why: {error}");
                        }
                        self.refused.push((peer, now + config.refusal_grace));
                    }
                },
                ToServer::Ready(ready) => {
                    if let Err(error) = lobby.set_ready(peer, ready) {
                        log::debug!("lobby: {peer} cannot be ready: {error}");
                    }
                }
                ToServer::Start => match lobby.start(Some(peer)) {
                    Ok(()) => log::info!("lobby: {peer} started the match"),
                    Err(error) => log::debug!("lobby: {peer} cannot start: {error}"),
                },
            }
        }
        self.refused.retain(|&(peer, deadline)| {
            if !endpoint.is_connected(peer) {
                return false;
            }
            if now >= deadline {
                endpoint.kick(peer);
                return false;
            }
            true
        });
        self.hold_to_the_deadline(lobby, config, endpoint, now);
        // Whoever is no longer connected is out, however the frame's events
        // and messages fell: the lobby never holds a peer that is gone.
        let gone: Vec<PeerId> = lobby
            .entries()
            .iter()
            .map(|entry| entry.peer)
            .filter(|&peer| peer != PeerId::SERVER && !endpoint.is_connected(peer))
            .collect();
        for peer in gone {
            lobby.leave(peer);
        }
        if before != *lobby {
            self.dirty = true;
        }
        changes(&before, lobby, events);
    }

    /// Cuts off a connected peer that has not joined within
    /// [`LobbyConfig::join_timeout`]: a session slot is not held by a peer
    /// that never says who it is. Counted from when this lobby first saw
    /// it connected.
    fn hold_to_the_deadline(
        &mut self,
        lobby: &Lobby,
        config: &LobbyConfig,
        endpoint: &mut Endpoint,
        now: Duration,
    ) {
        let refused = &self.refused;
        let unjoined: Vec<PeerId> = endpoint
            .peers()
            .filter(|&peer| {
                peer != PeerId::SERVER
                    && !lobby.contains(peer)
                    && !refused.iter().any(|(r, _)| *r == peer)
            })
            .collect();
        self.waiting.retain(|peer, _| unjoined.contains(peer));
        for peer in unjoined {
            let deadline = *self
                .waiting
                .entry(peer)
                .or_insert(now + config.join_timeout);
            if now >= deadline {
                log::info!(
                    "lobby: {peer} did not join within {:?}; cut off",
                    config.join_timeout
                );
                endpoint.kick(peer);
                self.waiting.remove(&peer);
            }
        }
    }

    /// The lobby to everyone in it, when it changed.
    pub(super) fn send(&mut self, lobby: &Lobby, channel: ChannelId, endpoint: &mut Endpoint) {
        if !self.dirty {
            return;
        }
        self.dirty = false;
        let state = ToClient::State(Snapshot::of(lobby));
        for entry in lobby.entries() {
            if entry.peer == PeerId::SERVER {
                continue;
            }
            if let Err(error) = endpoint.send_message(entry.peer, channel, &state) {
                log::warn!("the lobby could not reach {}: {error}", entry.peer);
            }
        }
    }
}
