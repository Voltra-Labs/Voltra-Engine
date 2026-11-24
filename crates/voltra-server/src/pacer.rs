//! Keeps a server's ticks on the wall clock: one every step, caught up when
//! it falls behind, but never by more than a few at once.

use std::time::Duration;

/// The most ticks one catch-up runs; time owed past them is let go. A
/// server stalled for a second (a debugger, a swapped-out page) would
/// otherwise run sixty ticks back to back and send every client a burst.
pub const MAX_CATCH_UP: u32 = 5;

/// When the next tick is due, from a clock the caller reads.
#[derive(Debug, Clone)]
pub struct Pacer {
    step: Duration,
    next: Duration,
}

impl Pacer {
    /// Ticks every `step`, the first one at once.
    pub fn new(step: Duration) -> Self {
        Self {
            step: step.max(Duration::from_micros(100)),
            next: Duration::ZERO,
        }
    }

    /// How many ticks are owed at `now`: zero if it is not time yet, at
    /// most [`MAX_CATCH_UP`].
    pub fn due(&mut self, now: Duration) -> u32 {
        let mut ticks = 0;
        while self.next <= now && ticks < MAX_CATCH_UP {
            self.next += self.step;
            ticks += 1;
        }
        if self.next <= now {
            // Too far behind to catch up: start again from now.
            self.next = now + self.step;
        }
        ticks
    }

    /// How long until the next tick is due at `now`.
    pub fn wait(&self, now: Duration) -> Duration {
        self.next.saturating_sub(now)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const STEP: Duration = Duration::from_millis(10);

    #[test]
    fn a_tick_is_due_each_step() {
        let mut pacer = Pacer::new(STEP);
        assert_eq!(pacer.due(Duration::ZERO), 1, "the first at once");
        assert_eq!(pacer.due(Duration::from_millis(5)), 0);
        assert_eq!(
            pacer.wait(Duration::from_millis(5)),
            Duration::from_millis(5)
        );
        assert_eq!(pacer.due(Duration::from_millis(10)), 1);
        assert_eq!(pacer.due(Duration::from_millis(35)), 2, "caught up");
    }

    #[test]
    fn a_long_stall_is_let_go_rather_than_replayed() {
        let mut pacer = Pacer::new(STEP);
        pacer.due(Duration::ZERO);
        assert_eq!(pacer.due(Duration::from_secs(5)), MAX_CATCH_UP);
        assert_eq!(pacer.due(Duration::from_secs(5)), 0, "the rest is dropped");
        assert_eq!(pacer.wait(Duration::from_secs(5)), STEP);
    }
}
