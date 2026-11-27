//! The entities a client asks the server to send whole again, asked for
//! until each arrives.
//!
//! The ask rides the input channel, which loses datagrams: an entity asked
//! for once, in a message that was lost, would never come back if it never
//! changed (a level's coin at rest). So an id stays pending until its
//! spawn arrives, the server despawns it (the server answers an ask for an
//! entity it no longer has with a despawn), it goes out of interest, or
//! its level goes.
//!
//! **Asked again only after a round trip.** Each ask the server reads
//! starts the entity over in that client's view, so the whole entity is
//! sent again; an ask repeated in every message would have it re-sent
//! every frame of the round trip before the first answer could land. So
//! an id is listed, then not again until the answer could have come back:
//! the connection's round trip plus [`RTT_VARIATIONS`] times its variation
//! (RFC 6298's retransmission bound, as TCP waits before it resends).
//! Throttled on the client rather than the server: the client is the one
//! that knows an ask is outstanding, and the server keeps no time per
//! entity per client to check against. At most [`MAX_LISTED`] a message,
//! the ones listed longest ago first.

use std::collections::VecDeque;
use std::time::Duration;

use voltra_net::ConnectionStats;

use super::ReplicationClient;
use crate::wire::{NetId, MAX_LISTED};

/// How many round-trip variations past the round trip an ask waits before
/// it is listed again: RFC 6298's `K`.
pub(crate) const RTT_VARIATIONS: u32 = 4;

/// The ids waiting to be sent whole again, each with when it was last
/// listed, in the order they are listed.
#[derive(Debug, Default)]
pub(crate) struct Resync {
    pending: VecDeque<(NetId, Option<Duration>)>,
}

impl Resync {
    /// Asks for `id`, once however often it is asked.
    pub(crate) fn want(&mut self, id: NetId) {
        if !self.pending.iter().any(|&(pending, _)| pending == id) {
            self.pending.push_back((id, None));
        }
    }

    /// `id` arrived, or went: no more asking.
    pub(crate) fn settle(&mut self, id: NetId) {
        self.pending.retain(|&(pending, _)| pending != id);
    }

    /// What one message sent at `now` lists: at most [`MAX_LISTED`] of the
    /// ids not listed within `wait`, put behind the rest so the next
    /// message lists the others first.
    pub(crate) fn listed(&mut self, now: Duration, wait: Duration) -> Vec<NetId> {
        let mut listed = Vec::new();
        let mut kept = VecDeque::with_capacity(self.pending.len());
        let mut again = Vec::new();
        for (id, last) in self.pending.drain(..) {
            let due = last.is_none_or(|last| now.saturating_sub(last) >= wait);
            if due && listed.len() < MAX_LISTED {
                listed.push(id);
                again.push((id, Some(now)));
            } else {
                kept.push_back((id, last));
            }
        }
        kept.extend(again);
        self.pending = kept;
        listed
    }

    /// Asks for nothing more: a new session, or the level gone.
    pub(crate) fn clear(&mut self) {
        self.pending.clear();
    }

    /// How many are waiting.
    pub(crate) fn len(&self) -> usize {
        self.pending.len()
    }
}

/// How long an ask waits before it is listed again, over `link`.
pub(crate) fn wait_over(link: Option<ConnectionStats>) -> Duration {
    link.map_or(Duration::ZERO, |link| {
        link.rtt + link.rtt_variation * RTT_VARIATIONS
    })
}

impl ReplicationClient {
    /// How many entities this client is waiting to be sent whole again.
    pub fn resync_pending(&self) -> usize {
        self.resync.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WAIT: Duration = Duration::from_millis(100);

    fn at(ms: u64) -> Duration {
        Duration::from_millis(ms)
    }

    #[test]
    fn every_pending_id_is_listed_in_turn_until_it_settles() {
        let mut resync = Resync::default();
        for id in 0..(MAX_LISTED as NetId + 6) {
            resync.want(id);
        }
        resync.want(3);
        assert_eq!(resync.len(), MAX_LISTED + 6, "once each");
        let first = resync.listed(at(0), Duration::ZERO);
        assert_eq!(first.len(), MAX_LISTED);
        let second = resync.listed(at(0), Duration::ZERO);
        assert!(
            (MAX_LISTED as NetId..MAX_LISTED as NetId + 6).all(|id| second.contains(&id)),
            "the rest come next"
        );
        resync.settle(3);
        for _ in 0..3 {
            assert!(!resync.listed(at(0), Duration::ZERO).contains(&3));
        }
        resync.clear();
        assert!(resync.listed(at(0), Duration::ZERO).is_empty());
    }

    #[test]
    fn an_ask_is_listed_again_only_after_a_round_trip() {
        let mut resync = Resync::default();
        resync.want(7);
        assert_eq!(resync.listed(at(0), WAIT), [7], "at once");
        assert!(resync.listed(at(16), WAIT).is_empty(), "in flight");
        assert!(resync.listed(at(99), WAIT).is_empty(), "in flight");
        resync.want(8);
        assert_eq!(resync.listed(at(99), WAIT), [8], "a new one at once");
        assert_eq!(resync.listed(at(100), WAIT), [7], "unanswered: again");
        assert_eq!(resync.len(), 2);
    }

    #[test]
    fn the_wait_is_the_round_trip_and_four_variations() {
        let link = ConnectionStats {
            rtt: Duration::from_millis(60),
            rtt_variation: Duration::from_millis(10),
            ..ConnectionStats::default()
        };
        assert_eq!(wait_over(Some(link)), Duration::from_millis(100));
        assert_eq!(wait_over(None), Duration::ZERO);
    }
}
