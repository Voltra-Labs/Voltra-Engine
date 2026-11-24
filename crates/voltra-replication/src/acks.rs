//! Which snapshots a client has: the newest, and a bit for each of the 64
//! before it — the same shape as the packet acks under it, one level up.

use crate::wire::ServerTick;

/// The snapshots received, as a client reports them.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct ReceivedTicks {
    /// `0` until the first.
    pub(crate) newest: ServerTick,
    /// Bit `n`: tick `newest - n - 1` arrived.
    pub(crate) mask: u64,
}

impl ReceivedTicks {
    /// Notes `tick` as received.
    pub(crate) fn insert(&mut self, tick: ServerTick) {
        if tick == 0 {
            return;
        }
        if self.newest == 0 {
            self.newest = tick;
            self.mask = 0;
        } else if tick > self.newest {
            let shift = tick - self.newest;
            self.mask = if shift >= 64 {
                0
            } else {
                // The old newest becomes bit `shift - 1`.
                (self.mask << shift) | (1 << (shift - 1))
            };
            self.newest = tick;
        } else if tick < self.newest {
            let back = self.newest - tick;
            if back <= 64 {
                self.mask |= 1 << (back - 1);
            }
        }
    }

    /// Every tick these name, newest first.
    pub(crate) fn ticks(&self) -> impl Iterator<Item = ServerTick> + '_ {
        let newest = self.newest;
        (newest != 0)
            .then_some(newest)
            .into_iter()
            .chain((0..64u32).filter_map(move |bit| {
                (self.mask & (1 << bit) != 0)
                    .then(|| newest.checked_sub(bit + 1))
                    .flatten()
                    .filter(|&t| t != 0)
            }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ticks_in_any_order_are_all_named() {
        let mut received = ReceivedTicks::default();
        for tick in [5, 3, 9, 8, 1, 70] {
            received.insert(tick);
        }
        let mut ticks: Vec<_> = received.ticks().collect();
        ticks.sort_unstable();
        // 5, 3 and 1 are more than 64 behind 70: forgotten.
        assert_eq!(ticks, vec![8, 9, 70]);

        let mut received = ReceivedTicks::default();
        for tick in [10, 12, 11, 7] {
            received.insert(tick);
        }
        let mut ticks: Vec<_> = received.ticks().collect();
        ticks.sort_unstable();
        assert_eq!(ticks, vec![7, 10, 11, 12]);
    }

    #[test]
    fn nothing_received_names_nothing() {
        assert_eq!(ReceivedTicks::default().ticks().count(), 0);
    }
}
