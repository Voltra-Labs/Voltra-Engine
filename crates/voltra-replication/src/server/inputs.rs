//! A client's inputs, held until the step that simulates each.
//!
//! One input per server step, in the client's own order: the client
//! predicted its entity with exactly these inputs, one per its own step,
//! so the server must not merge, skip or repeat any or the two diverge.
//! When none is waiting the step has no input — the game leaves the
//! entity be — rather than the last one repeated, which would move the
//! entity a step the client never took. Unity Netcode's input buffer and
//! Overwatch's per-tick command queue make the same choice.
//!
//! Each input keeps the moment its client was drawing when it made it,
//! which this step's shot is rewound to (`crate::lag`), and the server
//! tick it arrived at: how long it waited here is lag the server measured
//! itself, part of how far back that client may be rewound.

use std::collections::BTreeMap;

use crate::wire::{InputTick, ServerTick};
use crate::ViewTime;

/// One input, waiting.
#[derive(Debug)]
struct Waiting {
    view: Option<ViewTime>,
    bytes: Vec<u8>,
    /// The server tick when it first arrived.
    arrived: ServerTick,
}

/// One client's queue.
#[derive(Debug, Default)]
pub(crate) struct InputQueue {
    waiting: BTreeMap<InputTick, Waiting>,
    /// The newest simulated, which the snapshots acknowledge; `0` for none.
    pub(crate) consumed: InputTick,
    /// This step's, taken by [`next`](Self::next).
    pub(crate) current: Option<Vec<u8>>,
    /// The moment this step's input was made at, as its client reported.
    pub(crate) current_view: Option<ViewTime>,
    /// Server ticks this step's input waited, from its arrival to this
    /// step; `0` with none.
    pub(crate) current_wait: u32,
    /// Inputs dropped because the queue was full.
    pub(crate) dropped: u64,
}

impl InputQueue {
    /// Takes in what a message carried at server tick `now`: each input
    /// once, however many messages repeat it, and none already simulated.
    pub(crate) fn offer(
        &mut self,
        inputs: Vec<(InputTick, Option<ViewTime>, Vec<u8>)>,
        max_size: usize,
        cap: usize,
        now: ServerTick,
    ) {
        for (tick, view, bytes) in inputs {
            if tick <= self.consumed || bytes.len() > max_size {
                continue;
            }
            self.waiting.entry(tick).or_insert(Waiting {
                view,
                bytes,
                arrived: now,
            });
        }
        while self.waiting.len() > cap.max(1) {
            if let Some((tick, _)) = self.waiting.pop_first() {
                self.consumed = tick;
                self.dropped += 1;
            }
        }
    }

    /// Moves the oldest waiting input into [`current`](Self::current) for
    /// the step `now` about to run, or none.
    pub(crate) fn next(&mut self, now: ServerTick) {
        let (view, bytes, wait) = match self.waiting.pop_first() {
            Some((tick, waiting)) => {
                self.consumed = tick;
                (
                    waiting.view,
                    Some(waiting.bytes),
                    now.saturating_sub(waiting.arrived),
                )
            }
            None => (None, None, 0),
        };
        self.current = bytes;
        self.current_view = view;
        self.current_wait = wait;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_input_is_simulated_once_in_order() {
        let mut queue = InputQueue::default();
        queue.offer(vec![(1, None, vec![1]), (2, None, vec![2])], 8, 8, 0);
        queue.offer(
            vec![(1, None, vec![1]), (2, None, vec![2]), (3, None, vec![3])],
            8,
            8,
            0,
        );
        let mut seen = Vec::new();
        for _ in 0..5 {
            queue.next(1);
            seen.push(queue.current.clone());
        }
        assert_eq!(
            seen,
            vec![Some(vec![1]), Some(vec![2]), Some(vec![3]), None, None]
        );
        assert_eq!(queue.consumed, 3);
        // Late repeats of simulated inputs are ignored.
        queue.offer(vec![(2, None, vec![2])], 8, 8, 0);
        queue.next(1);
        assert_eq!(queue.current, None);
    }

    #[test]
    fn a_flood_is_cut_to_the_newest_and_oversized_inputs_are_dropped() {
        let mut queue = InputQueue::default();
        queue.offer(
            (1..=20).map(|t| (t, None, vec![t as u8])).collect(),
            8,
            4,
            0,
        );
        queue.offer(vec![(21, None, vec![0; 9])], 8, 4, 0);
        assert_eq!(queue.dropped, 16);
        queue.next(1);
        assert_eq!(queue.current, Some(vec![17]));
    }

    #[test]
    fn each_input_brings_its_own_view_time() {
        let view = |tick| Some(ViewTime { tick, fraction: 0 });
        let mut queue = InputQueue::default();
        queue.offer(
            vec![(1, view(40), vec![1]), (2, view(41), vec![2])],
            8,
            8,
            0,
        );
        // A repeat claiming another moment changes nothing.
        queue.offer(vec![(2, view(5), vec![2])], 8, 8, 0);
        queue.next(1);
        assert_eq!(queue.current_view, view(40));
        queue.next(1);
        assert_eq!(queue.current_view, view(41));
        // No input this step: no moment either.
        queue.next(1);
        assert_eq!((queue.current.as_deref(), queue.current_view), (None, None));
    }

    #[test]
    fn each_input_says_how_long_it_waited() {
        let mut queue = InputQueue::default();
        queue.offer(vec![(1, None, vec![1]), (2, None, vec![2])], 8, 8, 10);
        queue.offer(vec![(3, None, vec![3])], 8, 8, 12);
        queue.next(11);
        assert_eq!(queue.current_wait, 1);
        queue.next(14);
        assert_eq!(queue.current_wait, 4);
        queue.next(15);
        assert_eq!(queue.current_wait, 3);
        queue.next(16);
        assert_eq!(queue.current_wait, 0, "none this step");
    }
}
