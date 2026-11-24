//! How much of a frame loads may take, and what they took.
//!
//! Load work on the frame is cut into units — one entity spawned, one
//! texture uploaded, one batch despawned, one task polled — and a unit is
//! started only while the frame's budget lasts. So a frame overshoots by at
//! most the unit it was in when the budget ran out: [`LoadStats`] reports
//! when the last unit started ([`LoadStats::last_start`], never past the
//! budget) and the longest one, and the tests hold the loader to both.
//! At least one unit runs a frame that has any, so a budget of zero still
//! makes progress.
//!
//! Wall time, as a frame is: on a machine with no spare core the workers
//! and the frame share one, and a frame preempted in the middle of a unit
//! reports that unit as long.

use std::time::Duration;

// `std::time::Instant` panics on the web; `web-time`'s is the same type
// natively and `performance.now()` there.
#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;
#[cfg(target_arch = "wasm32")]
use web_time::Instant;

/// The budget a frame gives loads unless the game says otherwise: a
/// quarter of a 60 Hz frame. Unity's `backgroundLoadingPriority` spans
/// 2–50 ms for the same choice, `BelowNormal` being 4.
pub const DEFAULT_LOAD_BUDGET: Duration = Duration::from_millis(4);

/// What the last frame's load work cost: a scene change's and a streamed
/// world's together.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LoadStats {
    /// The budget the frame had.
    pub budget: Duration,
    /// The time load work took on the frame, from its first unit to its
    /// last, bookkeeping included.
    pub work: Duration,
    /// How many units ran.
    pub units: u32,
    /// When the last unit started, from the frame's first load work: within
    /// the budget, but for a frame's first unit, which always runs.
    pub last_start: Duration,
    /// The longest of them: the most `work` can exceed `budget` by.
    pub longest_unit: Duration,
}

/// One frame's budget, being spent.
#[derive(Debug)]
pub(super) struct FrameBudget {
    start: Instant,
    limit: Duration,
    units: u32,
    longest: Duration,
    last_start: Duration,
}

/// A unit being timed.
pub(super) struct Unit(Instant);

impl FrameBudget {
    pub(super) fn new(limit: Duration) -> Self {
        Self {
            start: Instant::now(),
            limit,
            units: 0,
            longest: Duration::ZERO,
            last_start: Duration::ZERO,
        }
    }

    /// Whether another unit may start: the budget is not spent, or nothing
    /// has run yet this frame.
    pub(super) fn has_time(&self) -> bool {
        self.units == 0 || self.start.elapsed() < self.limit
    }

    /// Starts timing a unit.
    pub(super) fn begin(&mut self) -> Unit {
        let now = Instant::now();
        self.last_start = now.duration_since(self.start);
        Unit(now)
    }

    /// Ends the unit `begin` started.
    pub(super) fn end(&mut self, unit: Unit) {
        self.units += 1;
        self.longest = self.longest.max(unit.0.elapsed());
    }

    /// What the frame spent.
    pub(super) fn finish(&self) -> LoadStats {
        LoadStats {
            budget: self.limit,
            work: self.start.elapsed(),
            units: self.units,
            longest_unit: self.longest,
            last_start: self.last_start,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_unit_always_runs_and_then_the_budget_decides() {
        let mut budget = FrameBudget::new(Duration::ZERO);
        assert!(budget.has_time());
        let unit = budget.begin();
        budget.end(unit);
        assert!(!budget.has_time());
        assert_eq!(budget.finish().units, 1);
    }

    #[test]
    fn a_generous_budget_keeps_going() {
        let mut budget = FrameBudget::new(Duration::from_secs(60));
        for _ in 0..10 {
            assert!(budget.has_time());
            let unit = budget.begin();
            budget.end(unit);
        }
        assert_eq!(budget.finish().units, 10);
    }
}
