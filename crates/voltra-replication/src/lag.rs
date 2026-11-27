//! Lag compensation's network half: the moment a client says it was
//! looking at, and how far back a server lets that take a shot.
//!
//! A client draws remote entities a jitter buffer behind the newest
//! snapshot ([`Timeline`](crate::Timeline)), so what its player aims at
//! is where they were, not where the server has them. Each input it sends
//! carries that moment — the interpolation clock's drawn tick when the
//! input was made, a [`ViewTime`] — and the server tests the input's shot
//! against its colliders put back to then (`voltra-physics`' `rewind`).
//!
//! The moment is the client's word, and so is bounded:
//! [`LagCompensation::target`] takes it no later than now (a client
//! claiming the future gains nothing) and no earlier than
//! [`max_rewind`](LagCompensation::max_rewind) before now — a client
//! slowed on purpose, or with a terrible link, cannot shoot further into
//! the past than every other player has agreed to be shot in. Valve's
//! Source keeps a second of history and rewinds by the client's own
//! `cl_interp` plus its latency, capped at `sv_maxunlag` (one second);
//! Overwatch favours the shooter only within a bound, then makes the
//! shooter lead their target. The default here, 200 ms, covers a
//! jitter buffer and a round trip on any playable link, and is what a
//! target can feel: shot behind cover a fifth of a second after reaching
//! it, never more.

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::wire::ServerTick;

/// A moment of the server's past, in ticks: the whole tick and 256ths of
/// the next. What a client was drawing remote entities at when it made an
/// input, sent with it; a 256th of a tick is a few hundredths of a
/// millisecond, far finer than anything drawn moves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ViewTime {
    /// The server tick at or before the moment.
    pub tick: ServerTick,
    /// How far on toward the next, in 256ths.
    pub fraction: u8,
}

impl ViewTime {
    /// The moment `ticks` (fractional) — `None` for one before the first
    /// tick, past the last a tick can count, or not a number.
    pub fn from_ticks(ticks: f64) -> Option<Self> {
        if !(0.0..f64::from(ServerTick::MAX)).contains(&ticks) {
            return None;
        }
        let scaled = (ticks * 256.0).round();
        let tick = (scaled / 256.0).floor();
        Some(Self {
            tick: tick as ServerTick,
            fraction: (scaled - tick * 256.0) as u8,
        })
    }

    /// The moment, in ticks.
    pub fn ticks(&self) -> f64 {
        f64::from(self.tick) + f64::from(self.fraction) / 256.0
    }
}

/// How the server compensates its clients' lag. On by default
/// ([`ReplicationConfig::lag_compensation`](crate::ReplicationConfig)):
/// what is remembered is only what a game marks `LagCompensated`, so it
/// costs nothing until something is. Only the server's is read.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LagCompensation {
    /// How much of the past the server keeps — a pose a tick of every
    /// lag-compensated collider. One second by default, Source's; it need
    /// be no longer than [`max_rewind`](Self::max_rewind), and is kept
    /// longer so a game can rewind further for its own reasons (a replay,
    /// a kill cam) without the session's bound changing.
    pub history: Duration,
    /// The furthest back a client's shot is rewound, whatever it claims:
    /// 200 ms by default. A rewind is bounded by the history kept too.
    pub max_rewind: Duration,
}

impl Default for LagCompensation {
    fn default() -> Self {
        Self {
            history: Duration::from_secs(1),
            max_rewind: Duration::from_millis(200),
        }
    }
}

impl LagCompensation {
    /// The moment to rewind to for an input that `reported` it was made
    /// at, with `now` the newest tick the server holds and a tick `step`
    /// seconds long: the report, no later than `now` and no earlier than
    /// [`max_rewind`](Self::max_rewind) before it. `now` without a report
    /// — an input before the client had a snapshot, or none this step.
    pub fn target(&self, reported: Option<ViewTime>, now: f64, step: f32) -> f64 {
        let Some(reported) = reported else {
            return now;
        };
        let bound = self.max_rewind.as_secs_f64() / f64::from(step.max(1e-4));
        reported.ticks().clamp(now - bound, now)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const STEP: f32 = 1.0 / 60.0;

    fn at(ticks: f64) -> Option<ViewTime> {
        ViewTime::from_ticks(ticks)
    }

    #[test]
    fn a_view_time_keeps_256ths_of_a_tick() {
        for ticks in [0.0, 1.5, 99.25, 1234.99, 70_000.003_9] {
            let view = at(ticks).expect("in range");
            assert!((view.ticks() - ticks).abs() <= 1.0 / 512.0, "{ticks}");
        }
        // Rounded up into the next tick, not to a fraction of 256.
        assert_eq!(
            at(4.999),
            Some(ViewTime {
                tick: 5,
                fraction: 0
            })
        );
        for nonsense in [-1.0, f64::NAN, f64::INFINITY, 5e9] {
            assert_eq!(at(nonsense), None, "{nonsense}");
        }
    }

    #[test]
    fn a_report_within_the_bound_is_taken_as_it_is() {
        let lag = LagCompensation::default();
        assert_eq!(lag.target(at(95.5), 100.0, STEP), 95.5);
    }

    #[test]
    fn the_far_past_is_clamped_at_max_rewind() {
        let lag = LagCompensation::default();
        // 200 ms at 60 a second is 12 ticks.
        let target = lag.target(at(10.0), 100.0, STEP);
        assert!((target - 88.0).abs() < 1e-5, "{target}");
    }

    #[test]
    fn the_future_is_now() {
        let lag = LagCompensation::default();
        assert_eq!(lag.target(at(150.25), 100.0, STEP), 100.0);
        assert_eq!(
            lag.target(
                Some(ViewTime {
                    tick: ServerTick::MAX,
                    fraction: 255
                }),
                100.0,
                STEP
            ),
            100.0
        );
        assert_eq!(lag.target(None, 100.0, STEP), 100.0);
    }
}
