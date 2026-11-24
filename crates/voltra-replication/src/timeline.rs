//! The client's interpolation clock: which server moment it draws, a
//! jitter buffer's depth behind the newest snapshot it can count on.
//!
//! Every snapshot's arrival is an *offset*: when it came, minus when the
//! server made it (its tick times the tick's length). The smallest offset
//! of the last three seconds is the path at its quickest — the latency
//! without the jitter. Then, every frame, the clock asks how *stale* the
//! newest snapshot in hand is: how far the quickest path says the server
//! has got, past the newest tick received. That staleness is what the
//! jitter costs — a late snapshot, a lost one, one overtaken and dropped by
//! the sequenced channel, the wait between two — measured as it is felt
//! rather than modelled. The most of it in the window, an interval on
//! top, is the buffer: drawn that far behind, the snapshot after the
//! drawn moment has always arrived, over the last three seconds at least.
//! Source's `cl_interp` fixes that depth by hand; Unity Netcode's
//! interpolation buffer and Overwatch size it from the link like this.
//!
//! The drawn moment does not jump to follow the measurement: it runs at
//! up to 10 % fast or slow until it is where it should be, so a change in
//! the network is a slight change of pace on screen, not a skip. Only a
//! gap of more than a quarter second (the first snapshot, a stall) snaps.

use std::collections::VecDeque;
use std::time::Duration;

use crate::wire::ServerTick;

/// How long arrivals and stalenesses are remembered for the measurement.
const WINDOW: f64 = 3.0;

/// The most of either remembered, whatever the rate.
const MAX_SAMPLES: usize = 512;

/// How far the pace may stray from the clock's to catch up.
const MAX_PACE: f64 = 0.10;

/// A gap past which the drawn moment snaps rather than catching up.
const SNAP: f64 = 0.25;

/// The interpolation clock.
#[derive(Debug, Clone)]
pub struct Timeline {
    /// A server tick's length, in seconds.
    step: f64,
    /// Seconds between snapshots.
    interval: f64,
    min_delay: f64,
    /// `(arrival, offset)`, oldest first.
    arrivals: VecDeque<(f64, f64)>,
    /// `(when, staleness)`, oldest first.
    staleness: VecDeque<(f64, f64)>,
    quickest: f64,
    newest: ServerTick,
    jitter: f64,
    /// The server moment being drawn, in seconds.
    drawn: Option<f64>,
}

/// Drops what is older than the window, keeping at least one.
fn trim(samples: &mut VecDeque<(f64, f64)>, now: f64) {
    while samples.len() > MAX_SAMPLES
        || (samples.len() > 1 && samples.front().is_some_and(|(at, _)| *at < now - WINDOW))
    {
        samples.pop_front();
    }
}

impl Timeline {
    /// A clock for snapshots `interval` ticks of `step` seconds apart,
    /// never drawing fewer than `min_delay_snapshots` intervals behind.
    pub fn new(step: f32, interval: u32, min_delay_snapshots: f32) -> Self {
        let step = f64::from(step.max(1e-4));
        let interval = step * f64::from(interval.max(1));
        Self {
            step,
            interval,
            min_delay: interval * f64::from(min_delay_snapshots.max(0.0)),
            arrivals: VecDeque::new(),
            staleness: VecDeque::new(),
            quickest: 0.0,
            newest: 0,
            jitter: 0.0,
            drawn: None,
        }
    }

    /// Notes snapshot `tick` arriving at `now`.
    pub fn arrived(&mut self, tick: ServerTick, now: Duration) {
        let now = now.as_secs_f64();
        self.newest = self.newest.max(tick);
        self.arrivals
            .push_back((now, now - f64::from(tick) * self.step));
        trim(&mut self.arrivals, now);
        self.quickest = self
            .arrivals
            .iter()
            .map(|(_, o)| *o)
            .fold(f64::INFINITY, f64::min);
    }

    /// The measured jitter: the most the newest snapshot in hand lagged the
    /// quickest path, over the window.
    pub fn jitter(&self) -> Duration {
        Duration::from_secs_f64(self.jitter.max(0.0))
    }

    /// How far behind the quickest path the clock draws.
    pub fn delay(&self) -> Duration {
        Duration::from_secs_f64(self.delay_secs())
    }

    fn delay_secs(&self) -> f64 {
        // An interval on top: what the window has not seen yet — one more
        // snapshot lost, one more late — is covered once before the clock
        // has to hold.
        (self.jitter + self.interval).max(self.min_delay)
    }

    /// Moves the drawn moment on by a frame of `delta` seconds at client
    /// time `now`, and answers it in server ticks (fractional) — `None`
    /// before the first snapshot.
    pub fn advance(&mut self, now: Duration, delta: f32) -> Option<f64> {
        if self.arrivals.is_empty() {
            return None;
        }
        let now = now.as_secs_f64();
        let reached = now - self.quickest;
        let stale = (reached - f64::from(self.newest) * self.step).max(0.0);
        self.staleness.push_back((now, stale));
        trim(&mut self.staleness, now);
        self.jitter = self.staleness.iter().map(|(_, s)| *s).fold(0.0, f64::max);

        let target = reached - self.delay_secs();
        let delta = f64::from(delta.max(0.0));
        let drawn = match self.drawn {
            // Where a frame at the clock's own pace would land, against
            // where it should: the difference sets this frame's pace.
            Some(drawn) if (target - (drawn + delta)).abs() <= SNAP => {
                let behind = target - (drawn + delta);
                drawn + delta * (1.0 + (behind / SNAP).clamp(-MAX_PACE, MAX_PACE))
            }
            _ => target,
        };
        self.drawn = Some(drawn);
        Some(drawn / self.step)
    }

    /// The moment last drawn, in server ticks.
    pub fn drawn_tick(&self) -> Option<f64> {
        self.drawn.map(|d| d / self.step)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const STEP: f32 = 1.0 / 60.0;

    /// Snapshots every tick, `late(tick)` seconds after 50 ms; a frame
    /// every tick reads what has arrived by then, oldest first, and
    /// advances the clock. Answers the timeline.
    fn run(late: impl Fn(u32) -> f64) -> Timeline {
        let mut timeline = Timeline::new(STEP, 1, 1.0);
        let mut arrivals: Vec<(f64, u32)> = (1..240u32)
            .map(|tick| (f64::from(tick) / 60.0 + 0.05 + late(tick), tick))
            .collect();
        arrivals.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut next = 0;
        let mut newest = 0;
        for frame in 1..240u32 {
            let now = f64::from(frame) / 60.0;
            while next < arrivals.len() && arrivals[next].0 <= now {
                // A sequenced channel drops what a newer one overtook.
                if arrivals[next].1 > newest {
                    newest = arrivals[next].1;
                    timeline.arrived(newest, Duration::from_secs_f64(arrivals[next].0));
                }
                next += 1;
            }
            timeline.advance(Duration::from_secs_f64(now), STEP);
        }
        timeline
    }

    #[test]
    fn the_delay_grows_with_the_jitter() {
        let steady = run(|_| 0.0);
        // Up to 40 ms more, spread evenly.
        let jittery = run(|tick| f64::from(tick % 9) * 0.005);
        assert!(steady.jitter() < Duration::from_millis(45));
        assert!(jittery.jitter() > steady.jitter() + Duration::from_millis(30));
        assert!(jittery.delay() > steady.delay() + Duration::from_millis(30));
    }

    #[test]
    fn the_drawn_moment_catches_up_without_jumping() {
        let mut timeline = Timeline::new(STEP, 1, 1.0);
        let mut now = 1.0;
        for tick in 1..=60u32 {
            timeline.arrived(tick, Duration::from_secs_f64(now));
            timeline.advance(Duration::from_secs_f64(now), STEP);
            now += 1.0 / 60.0;
        }
        let mut last = timeline
            .advance(Duration::from_secs_f64(now), STEP)
            .expect("snapshots in");
        // The link gets 100 ms slower: drawn ticks keep coming at most 10 %
        // faster or slower than real time.
        for tick in 61..=360u32 {
            timeline.arrived(tick, Duration::from_secs_f64(now + 0.1));
            now += 1.0 / 60.0;
            let drawn = timeline
                .advance(Duration::from_secs_f64(now), STEP)
                .expect("snapshots in");
            let pace = drawn - last;
            assert!((0.89..=1.11).contains(&pace), "pace {pace} at tick {tick}");
            last = drawn;
        }
    }
}
