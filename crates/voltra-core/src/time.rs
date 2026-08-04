//! Frame timing: a monotonic clock and the per-frame delta it produces.

use std::time::{Duration, Instant};

/// A sane upper bound on a single frame's delta.
///
/// Without this, pausing at a breakpoint or dragging the window (which stalls
/// the event loop on most platforms) would produce a multi-second timestep on
/// the next `tick`. Anything driven by that delta — physics integration,
/// animation, gameplay timers — would then jump far ahead in a single step
/// instead of catching up smoothly. Clamping trades perfect accuracy after a
/// stall for stability, which is the right trade for a real-time loop.
const MAX_DELTA_SECS: f32 = 0.25;

/// The time elapsed between two consecutive frames.
///
/// Always non-negative and clamped to [`MAX_DELTA_SECS`] by [`Clock::tick`].
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Timestep(Duration);

impl Timestep {
    /// The delta as seconds, in `f32`. The unit most gameplay code wants.
    pub fn as_secs_f32(&self) -> f32 {
        self.0.as_secs_f32()
    }

    /// The delta as seconds, in `f64`, for callers that accumulate over many
    /// frames and need to avoid `f32` drift.
    pub fn as_secs_f64(&self) -> f64 {
        self.0.as_secs_f64()
    }

    /// The delta as milliseconds, in `f32`. Convenient for on-screen stats.
    pub fn as_millis_f32(&self) -> f32 {
        self.0.as_secs_f32() * 1000.0
    }
}

/// A monotonic frame clock.
///
/// Construct once at startup with [`Clock::new`] and call [`Clock::tick`]
/// exactly once per frame.
#[derive(Debug, Clone)]
pub struct Clock {
    start: Instant,
    last_tick: Instant,
    frame_count: u64,
}

impl Clock {
    /// Starts a new clock. `start` and the first `last_tick` are both "now".
    pub fn new() -> Self {
        let now = Instant::now();
        Self {
            start: now,
            last_tick: now,
            frame_count: 0,
        }
    }

    /// Advances the clock by one frame and returns the delta since the
    /// previous call (or since [`Clock::new`] for the first call).
    ///
    /// The returned delta is clamped to [`MAX_DELTA_SECS`]; see that
    /// constant's doc comment for why.
    pub fn tick(&mut self) -> Timestep {
        let now = Instant::now();
        let delta = now.saturating_duration_since(self.last_tick);
        self.last_tick = now;
        self.frame_count += 1;

        let clamp = Duration::from_secs_f32(MAX_DELTA_SECS);
        Timestep(delta.min(clamp))
    }

    /// Total time elapsed since this clock was created, unclamped.
    pub fn elapsed(&self) -> Duration {
        self.start.elapsed()
    }

    /// Number of times [`Clock::tick`] has been called.
    pub fn frame_count(&self) -> u64 {
        self.frame_count
    }
}

impl Default for Clock {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn fresh_clock_reports_zero_ish() {
        let clock = Clock::new();
        assert_eq!(clock.frame_count(), 0);
        // "Zero-ish": some wall-clock time passes between `new` and this
        // assertion, but it should be far below the clamp.
        assert!(clock.elapsed() < Duration::from_secs_f32(MAX_DELTA_SECS));
    }

    #[test]
    fn tick_advances_frame_count() {
        let mut clock = Clock::new();
        clock.tick();
        clock.tick();
        assert_eq!(clock.frame_count(), 2);
    }

    #[test]
    fn tick_reports_a_small_positive_delta() {
        let mut clock = Clock::new();
        thread::sleep(Duration::from_millis(5));
        let dt = clock.tick();
        assert!(dt.as_secs_f32() > 0.0);
        assert!(dt.as_secs_f32() < MAX_DELTA_SECS);
    }

    #[test]
    fn clamp_caps_a_large_delta() {
        let mut clock = Clock::new();
        // Simulate a stall (breakpoint, dragged window) by rewinding the
        // last-tick timestamp instead of actually sleeping for a long time.
        clock.last_tick -= Duration::from_secs(5);
        let dt = clock.tick();
        assert_eq!(dt.as_secs_f32(), MAX_DELTA_SECS);
    }

    #[test]
    fn timestep_conversions_agree() {
        let ts = Timestep(Duration::from_millis(250));
        assert_eq!(ts.as_secs_f32(), 0.25);
        assert_eq!(ts.as_secs_f64(), 0.25);
        assert_eq!(ts.as_millis_f32(), 250.0);
    }

    #[test]
    fn timestep_default_is_zero() {
        let ts = Timestep::default();
        assert_eq!(ts.as_secs_f32(), 0.0);
    }
}
