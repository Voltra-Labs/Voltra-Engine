//! What a game sends and reads through `tick.net`: messages to peers,
//! the frame's messages and events.

use serde::Serialize;
use voltra_net::{ChannelId, Endpoint, Message, NetEvent, PeerId, SendError};

use super::Net;

impl Net {
    /// Queues `bytes` for `peer` on `channel`, sent at the end of the frame.
    pub fn send(
        &mut self,
        peer: PeerId,
        channel: ChannelId,
        bytes: &[u8],
    ) -> Result<(), SendError> {
        match self.endpoint.as_mut() {
            Some(endpoint) => endpoint.send(peer, channel, bytes),
            None => Err(SendError::NotConnected(peer)),
        }
    }

    /// Queues `value` (serde, as `postcard`) for `peer` on `channel`.
    pub fn send_message<T: Serialize + ?Sized>(
        &mut self,
        peer: PeerId,
        channel: ChannelId,
        value: &T,
    ) -> Result<(), SendError> {
        match self.endpoint.as_mut() {
            Some(endpoint) => endpoint.send_message(peer, channel, value),
            None => Err(SendError::NotConnected(peer)),
        }
    }

    /// Queues `bytes` for every connected peer; answers those it could not
    /// be queued for, and why.
    pub fn broadcast(&mut self, channel: ChannelId, bytes: &[u8]) -> Vec<(PeerId, SendError)> {
        match self.endpoint.as_mut() {
            Some(endpoint) => endpoint.broadcast(channel, bytes),
            None => Vec::new(),
        }
    }

    /// The messages this frame's receive brought, whole and in each
    /// channel's order. Decode one with [`Message::decode`].
    pub fn received(&self) -> &[Message] {
        self.endpoint.as_ref().map_or(&[], Endpoint::messages)
    }

    /// Takes this frame's messages, so each is handled exactly once however
    /// many hooks look.
    pub fn take_received(&mut self) -> Vec<Message> {
        self.endpoint
            .as_mut()
            .map(Endpoint::take_messages)
            .unwrap_or_default()
    }

    /// What happened this frame: peers joining and leaving, kicks,
    /// timeouts, a connect that failed.
    pub fn events(&self) -> &[NetEvent] {
        self.endpoint.as_ref().map_or(&[], Endpoint::events)
    }
}
