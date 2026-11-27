//! A client's side of a lobby: a copy of the server's, and what it asks.

use super::super::config::LobbyConfig;
use super::super::event::LobbyEvent;
use super::super::refusal::LobbyRefusal;
use super::super::state::Lobby;
use super::super::wire::{ToClient, ToServer};
use super::changes::changes;
use crate::{ChannelId, Endpoint, Message, NetEvent, PeerId};

/// What a client keeps beside its copy of the lobby.
#[derive(Debug, Default)]
pub(super) struct Client {
    /// This client's id, once the server let it in.
    pub(super) me: Option<PeerId>,
    /// What it asks, sent at the next flush.
    pub(super) outbox: Vec<ToServer>,
    /// Why the server would not let it in, if it would not.
    pub(super) refused: Option<LobbyRefusal>,
    /// Whether it is connected, to tell its lobby closing.
    pub(super) connected: bool,
}

impl Client {
    /// The frame's connection and what the server sent, into `lobby`.
    pub(super) fn receive(
        &mut self,
        lobby: &mut Lobby,
        config: &LobbyConfig,
        events: &mut Vec<LobbyEvent>,
        endpoint: &mut Endpoint,
        messages: Vec<Message>,
        net_events: &[NetEvent],
    ) {
        for event in net_events {
            match event {
                NetEvent::Connected(_) => {
                    self.connected = true;
                    self.me = endpoint.local_id();
                    self.outbox.push(ToServer::Hello {
                        name: config.name.clone(),
                    });
                }
                NetEvent::Disconnected { .. } => {
                    self.connected = false;
                    *lobby = Lobby::new(config);
                    events.push(LobbyEvent::Closed);
                    self.outbox.clear();
                }
            }
        }
        for message in messages {
            // What came with the goodbye is of a lobby already closed.
            if !self.connected {
                break;
            }
            match message.decode::<ToClient>() {
                // Each snapshot's changes in turn: a match started and
                // finished between two frames is both a Started and a
                // Finished, in order.
                Ok(ToClient::State(snapshot)) => match snapshot.into_lobby() {
                    Some(sent) => {
                        changes(lobby, &sent, events);
                        *lobby = sent;
                    }
                    None => log::warn!("the server sent a lobby that cannot be; ignored"),
                },
                Ok(ToClient::Refused(refusal)) => {
                    log::info!("lobby: refused: {refusal}");
                    self.refused = Some(refusal);
                    events.push(LobbyEvent::Refused(refusal));
                    endpoint.disconnect(PeerId::SERVER);
                }
                Err(error) => {
                    log::warn!("the server sent a lobby message that is not one: {error}");
                }
            }
        }
        // A close resets the copy; the entries leaving it are not worth an
        // event each — `Closed` says it all.
    }

    /// What it asked, to the server.
    pub(super) fn send(&mut self, channel: ChannelId, endpoint: &mut Endpoint) {
        for asked in self.outbox.drain(..) {
            if let Err(error) = endpoint.send_message(PeerId::SERVER, channel, &asked) {
                log::warn!("the lobby could not reach the server: {error}");
            }
        }
    }
}
