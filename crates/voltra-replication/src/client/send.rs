//! What a client tells the server: which snapshots arrived, its inputs,
//! and the entities to send it whole again.

use voltra_net::{Endpoint, PeerId};

use super::resync::wait_over;
use super::{ReplicationClient, MAX_PENDING_INPUTS};
use crate::wire::{self, ClientMessage, InputTick};
use crate::ViewTime;

impl ReplicationClient {
    /// The moment remote entities were last drawn at: what the player is
    /// looking at, which each input carries for the server's lag
    /// compensation. `None` before the first snapshot is drawn.
    pub fn view_time(&self) -> Option<ViewTime> {
        self.timeline.drawn_tick().and_then(ViewTime::from_ticks)
    }

    /// Queues an input for the server, sent with the next few
    /// [`send`](Self::send)s until acknowledged — with the moment the
    /// player was looking at ([`view_time`](Self::view_time)), which a
    /// shot in it is tested against.
    pub fn queue_input(&mut self, tick: InputTick, bytes: Vec<u8>) {
        if bytes.len() > self.config.max_input_size {
            log::warn!(
                "an input of {} bytes is over the {}-byte limit; the server would drop it",
                bytes.len(),
                self.config.max_input_size
            );
            return;
        }
        let view = self.view_time();
        self.inputs.push_back((tick, view, bytes));
        while self.inputs.len() > MAX_PENDING_INPUTS {
            self.inputs.pop_front();
        }
    }

    /// Tells the server which snapshots arrived, and sends the newest
    /// unacknowledged inputs and any entity to send whole again.
    pub fn send(&mut self, endpoint: &mut Endpoint) {
        if !endpoint.is_connected(PeerId::SERVER) {
            return;
        }
        let skip = self
            .inputs
            .len()
            .saturating_sub(self.config.input_redundancy.max(1));
        let message = ClientMessage {
            newest: self.received.newest,
            mask: self.received.mask,
            inputs: self.inputs.iter().skip(skip).cloned().collect(),
            resync: self
                .resync
                .listed(self.now, wait_over(endpoint.stats(PeerId::SERVER))),
        };
        if let Err(error) = endpoint.send(
            PeerId::SERVER,
            self.config.channels.input,
            &wire::encode(&message),
        ) {
            log::warn!("replication acks not sent: {error}");
        }
    }
}
