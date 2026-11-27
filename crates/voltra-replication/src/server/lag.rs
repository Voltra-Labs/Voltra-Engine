//! What the server knows of a client's lag, for lag compensation: the
//! moment the client says it was looking at, and what the server measured
//! itself to bound it by.

use std::time::Duration;

use voltra_net::{ConnectionStats, PeerId};

use super::ReplicationServer;
use crate::{MeasuredLag, ViewTime};

impl ReplicationServer {
    /// The moment `peer` was drawing remote entities at when it made this
    /// step's input, as it reported — the client's word, to be bounded
    /// ([`LagCompensation::target`](crate::LagCompensation::target))
    /// before a shot is rewound to it. `None` with no input this step, or
    /// one made before its first snapshot.
    pub fn view_time(&self, peer: PeerId) -> Option<ViewTime> {
        self.clients
            .get(&peer)
            .and_then(|view| view.inputs.current_view)
    }

    /// What this server measured of `peer`'s lag behind this step's input:
    /// the connection's round trip and its variation (`link`, from
    /// [`Endpoint::stats`](voltra_net::Endpoint::stats)), the snapshot
    /// interval it sends at, and how long the input waited in its queue.
    /// Nothing in it is the client's word. `None` for a peer that is not a
    /// client of this server.
    pub fn measured_lag(&self, peer: PeerId, link: ConnectionStats) -> Option<MeasuredLag> {
        let view = self.clients.get(&peer)?;
        let step = f64::from(self.step.max(1e-4));
        let ticks = |n: u32| Duration::from_secs_f64(f64::from(n) * step);
        Some(MeasuredLag {
            rtt: link.rtt,
            rtt_variation: link.rtt_variation,
            snapshot_interval: ticks(self.config.snapshot_interval.max(1)),
            input_wait: ticks(view.inputs.current_wait),
        })
    }
}
