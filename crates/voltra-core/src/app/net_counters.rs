//! The session's traffic as profiler counters, once a frame.
//!
//! Per frame, like every other counter: the round trip as it is, the bytes
//! and datagrams as what this frame moved — the difference of the
//! connections' running totals — so the profiler's average of a counter is
//! the session's rate. Unity's profiler shows its Network Messages module
//! the same way, per frame.

use voltra_net::ConnectionStats;

use super::tick_net::Net;

/// The counters' names, which the editor's profiler and a bridge client
/// read back.
pub const NET_COUNTERS: [&str; 6] = [
    "net rtt ms",
    "net bytes out",
    "net bytes in",
    "net packets out",
    "net packets in",
    "net peers",
];

impl Net {
    /// Every connection's totals summed, the largest round trip, and how
    /// many peers.
    pub fn totals(&self) -> (ConnectionStats, usize) {
        let mut total = ConnectionStats::default();
        let mut peers = 0;
        for peer in self.peers() {
            let Some(stats) = self.stats(peer) else {
                continue;
            };
            peers += 1;
            total.rtt = total.rtt.max(stats.rtt);
            total.packets_sent += stats.packets_sent;
            total.packets_received += stats.packets_received;
            total.bytes_sent += stats.bytes_sent;
            total.bytes_received += stats.bytes_received;
            total.resent += stats.resent;
        }
        (total, peers)
    }

    /// Sets this frame's counters. Nothing without a session.
    pub(super) fn count(&mut self) {
        if self.endpoint.is_none() {
            self.counted = ConnectionStats::default();
            return;
        }
        let (total, peers) = self.totals();
        // A peer that left takes its totals with it: never a negative frame.
        let delta = |now: u64, then: u64| now.saturating_sub(then) as f64;
        let values = [
            total.rtt.as_secs_f64() * 1000.0,
            delta(total.bytes_sent, self.counted.bytes_sent),
            delta(total.bytes_received, self.counted.bytes_received),
            delta(total.packets_sent, self.counted.packets_sent),
            delta(total.packets_received, self.counted.packets_received),
            peers as f64,
        ];
        for (name, value) in NET_COUNTERS.into_iter().zip(values) {
            voltra_profile::count(name, value);
        }
        self.counted = total;
    }
}
