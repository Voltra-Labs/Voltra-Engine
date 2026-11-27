//! The entities a client asks the server to send whole again, asked for
//! until each arrives.
//!
//! The ask rides the input channel, which loses datagrams: an entity asked
//! for once, in a message that was lost, would never come back if it never
//! changed (a level's coin at rest). So an id stays pending, listed in
//! every message — the first [`MAX_LISTED`], rotating through the rest
//! past that — until its spawn arrives, the server despawns it, or its
//! level goes.

use std::collections::VecDeque;

use super::ReplicationClient;
use crate::wire::{NetId, MAX_LISTED};

/// The ids waiting to be sent whole again, in the order they are listed.
#[derive(Debug, Default)]
pub(crate) struct Resync {
    pending: VecDeque<NetId>,
}

impl Resync {
    /// Asks for `id`, once however often it is asked.
    pub(crate) fn want(&mut self, id: NetId) {
        if !self.pending.contains(&id) {
            self.pending.push_back(id);
        }
    }

    /// `id` arrived, or went: no more asking.
    pub(crate) fn settle(&mut self, id: NetId) {
        self.pending.retain(|&pending| pending != id);
    }

    /// What one message lists: at most [`MAX_LISTED`], the ones listed
    /// put behind the rest so the next message lists the others first.
    pub(crate) fn listed(&mut self) -> Vec<NetId> {
        let count = self.pending.len().min(MAX_LISTED);
        let listed: Vec<NetId> = self.pending.drain(..count).collect();
        self.pending.extend(listed.iter().copied());
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

impl ReplicationClient {
    /// How many entities this client is waiting to be sent whole again.
    pub fn resync_pending(&self) -> usize {
        self.resync.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_pending_id_is_listed_in_turn_until_it_settles() {
        let mut resync = Resync::default();
        for id in 0..(MAX_LISTED as NetId + 6) {
            resync.want(id);
        }
        resync.want(3);
        assert_eq!(resync.len(), MAX_LISTED + 6, "once each");
        let first = resync.listed();
        assert_eq!(first.len(), MAX_LISTED);
        let second = resync.listed();
        assert!(
            (MAX_LISTED as NetId..MAX_LISTED as NetId + 6).all(|id| second.contains(&id)),
            "the rest come next"
        );
        assert_eq!(resync.listed().len(), MAX_LISTED, "still asked");
        resync.settle(3);
        assert!(!resync.listed().contains(&3) && !resync.listed().contains(&3));
        resync.clear();
        assert!(resync.listed().is_empty());
    }
}
