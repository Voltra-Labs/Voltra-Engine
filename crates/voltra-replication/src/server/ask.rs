//! A client's asks for entities to be sent whole again.
//!
//! An entity the server still tracks is started over in the client's view
//! and sent whole. One it no longer has is answered with a despawn, in
//! every snapshot until acknowledged: despawned while its view was being
//! started over (no spawn sent, so no despawn owed), the client would
//! otherwise ask for it for good. Only ids the server has issued are
//! answered, and at most [`MAX_ASKED_GONE`] such despawns wait per client
//! at once — a client asking for made-up ids, never acknowledging, cannot
//! grow what the server keeps for it. An ask past the cap is skipped; the
//! client asks again a round trip later.

use std::collections::BTreeMap;

use super::snapshot::Tracked;
use super::view::ClientView;
use crate::wire::{NetId, ServerTick, MAX_LISTED};

/// The most despawns answering asks for entities gone that wait for one
/// client's acknowledgement at once: four messages' worth of asks. Apart
/// from the real despawns, which are never refused.
pub(super) const MAX_ASKED_GONE: usize = 4 * MAX_LISTED;

/// The ids the server issues, and which it has: every one below `next`,
/// from 1 — or, once `next` has wrapped, every one but 0.
#[derive(Debug, Clone, Copy)]
pub(super) struct Issued {
    /// The next id to be issued.
    pub(super) next: NetId,
    /// Whether the ids have wrapped round.
    pub(super) wrapped: bool,
}

impl Default for Issued {
    fn default() -> Self {
        Self {
            next: 1,
            wrapped: false,
        }
    }
}

impl Issued {
    /// The next id, never 0.
    pub(super) fn issue(&mut self) -> NetId {
        let id = self.next;
        if id == NetId::MAX {
            self.wrapped = true;
        }
        self.next = self.next.wrapping_add(1).max(1);
        id
    }

    fn contains(self, id: NetId) -> bool {
        id != 0 && (self.wrapped || id < self.next)
    }
}

/// Answers `asks` from the client `view` is of: each entity still
/// `tracked` is started over, to be sent whole in the next snapshot after
/// `tick`; each one `issued` but gone is despawned, within the cap.
pub(super) fn answer(
    view: &mut ClientView,
    asks: Vec<NetId>,
    tracked: &BTreeMap<NetId, Tracked>,
    issued: Issued,
    components: usize,
    tick: ServerTick,
) {
    for id in asks.into_iter().take(MAX_LISTED) {
        if !issued.contains(id) {
            continue;
        }
        if tracked.contains_key(&id) {
            view.resync(id, components, tick.wrapping_add(1));
        } else if !view.despawned.contains(&id) && view.asked_gone.len() < MAX_ASKED_GONE {
            view.despawned.insert(id);
            view.asked_gone.insert(id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_issued_ids_are_answered_and_the_answers_are_capped() {
        let tracked = BTreeMap::new();
        let mut view = ClientView::default();
        let issued = Issued {
            next: 10_000,
            wrapped: false,
        };
        // Never issued: zero, and ids at or past the next one.
        let made_up: Vec<NetId> = std::iter::once(0).chain(10_000..10_063).collect();
        answer(&mut view, made_up, &tracked, issued, 1, 5);
        assert!(view.despawned.is_empty(), "nothing made up is kept");

        // A flood of issued ids, all gone, never acknowledged.
        for batch in 0..100 {
            let asks = (1 + batch * 64..1 + (batch + 1) * 64).collect();
            answer(&mut view, asks, &tracked, issued, 1, 5);
        }
        assert_eq!(view.despawned.len(), MAX_ASKED_GONE, "bounded");
        assert_eq!(view.asked_gone.len(), MAX_ASKED_GONE);

        // Wrapped, every id but zero has been issued.
        let wrapped = Issued {
            next: 3,
            wrapped: true,
        };
        assert!(wrapped.contains(NetId::MAX) && !wrapped.contains(0));
        assert!(!issued.contains(10_000) && issued.contains(9_999));
        let mut near = Issued {
            next: NetId::MAX,
            wrapped: false,
        };
        assert_eq!(near.issue(), NetId::MAX);
        assert_eq!(near.issue(), 1, "past the top, from 1 again");
        assert!(near.wrapped && near.contains(500));
    }

    #[test]
    fn a_real_despawn_is_never_refused_past_the_cap() {
        let mut view = ClientView::default();
        let issued = Issued {
            next: 1_000,
            wrapped: false,
        };
        let asks: Vec<NetId> = (1..=MAX_ASKED_GONE as NetId).collect();
        for chunk in asks.chunks(MAX_LISTED) {
            answer(&mut view, chunk.to_vec(), &BTreeMap::new(), issued, 1, 5);
        }
        assert_eq!(view.asked_gone.len(), MAX_ASKED_GONE);
        // A real despawn, as the refresh makes one, is not capped.
        view.despawned.insert(999);
        assert!(view.despawned.contains(&999));
        assert_eq!(view.despawned.len(), MAX_ASKED_GONE + 1);
    }
}
