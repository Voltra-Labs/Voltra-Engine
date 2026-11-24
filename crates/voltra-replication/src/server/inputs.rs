//! A client's inputs, held until the step that simulates each.
//!
//! One input per server step, in the client's own order: the client
//! predicted its entity with exactly these inputs, one per its own step,
//! so the server must not merge, skip or repeat any or the two diverge.
//! When none is waiting the step has no input — the game leaves the
//! entity be — rather than the last one repeated, which would move the
//! entity a step the client never took. Unity Netcode's input buffer and
//! Overwatch's per-tick command queue make the same choice.

use std::collections::BTreeMap;

use crate::wire::InputTick;

/// One client's queue.
#[derive(Debug, Default)]
pub(crate) struct InputQueue {
    waiting: BTreeMap<InputTick, Vec<u8>>,
    /// The newest simulated, which the snapshots acknowledge; `0` for none.
    pub(crate) consumed: InputTick,
    /// This step's, taken by [`next`](Self::next).
    pub(crate) current: Option<Vec<u8>>,
    /// Inputs dropped because the queue was full.
    pub(crate) dropped: u64,
}

impl InputQueue {
    /// Takes in what a message carried: each input once, however many
    /// messages repeat it, and none already simulated.
    pub(crate) fn offer(&mut self, inputs: Vec<(InputTick, Vec<u8>)>, max_size: usize, cap: usize) {
        for (tick, bytes) in inputs {
            if tick <= self.consumed || bytes.len() > max_size {
                continue;
            }
            self.waiting.entry(tick).or_insert(bytes);
        }
        while self.waiting.len() > cap.max(1) {
            if let Some((tick, _)) = self.waiting.pop_first() {
                self.consumed = tick;
                self.dropped += 1;
            }
        }
    }

    /// Moves the oldest waiting input into [`current`](Self::current) for
    /// the step about to run, or none.
    pub(crate) fn next(&mut self) {
        self.current = self.waiting.pop_first().map(|(tick, bytes)| {
            self.consumed = tick;
            bytes
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_input_is_simulated_once_in_order() {
        let mut queue = InputQueue::default();
        queue.offer(vec![(1, vec![1]), (2, vec![2])], 8, 8);
        queue.offer(vec![(1, vec![1]), (2, vec![2]), (3, vec![3])], 8, 8);
        let mut seen = Vec::new();
        for _ in 0..5 {
            queue.next();
            seen.push(queue.current.clone());
        }
        assert_eq!(
            seen,
            vec![Some(vec![1]), Some(vec![2]), Some(vec![3]), None, None]
        );
        assert_eq!(queue.consumed, 3);
        // Late repeats of simulated inputs are ignored.
        queue.offer(vec![(2, vec![2])], 8, 8);
        queue.next();
        assert_eq!(queue.current, None);
    }

    #[test]
    fn a_flood_is_cut_to_the_newest_and_oversized_inputs_are_dropped() {
        let mut queue = InputQueue::default();
        queue.offer((1..=20).map(|t| (t, vec![t as u8])).collect(), 8, 4);
        queue.offer(vec![(21, vec![0; 9])], 8, 4);
        assert_eq!(queue.dropped, 16);
        queue.next();
        assert_eq!(queue.current, Some(vec![17]));
    }
}
