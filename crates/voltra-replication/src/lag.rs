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
//! The moment is the client's word, and so is bounded twice
//! ([`LagCompensation::target`]): never later than now (a client claiming
//! the future gains nothing), and never further back than the lag the
//! *server* measured for that client allows ([`LagCompensation::allowed`]
//! of a [`MeasuredLag`]) — its round trip, how much that varies, the
//! snapshot interval, and how long its input waited — capped by an
//! absolute [`max_rewind`](LagCompensation::max_rewind). A client's view
//! lags the server by its round trip, plus its jitter buffer (stage 115:
//! the worst staleness it saw, which is up to a snapshot interval of
//! waiting and the jitter, plus an interval on top), plus the input's wait
//! in the server's queue; each of those the server can bound from what it
//! sees itself. So a player on a good link cannot claim the cap and shoot
//! a quarter second into the past, and a player on a slow one is rewound
//! as far as its link really is, up to the cap.
//!
//! Valve's Source rewinds by the client's own `cl_interp` plus the latency
//! it measures, capped at `sv_maxunlag` (one second); Overwatch favours the
//! shooter within a bound, then makes the shooter lead. The cap here, half
//! a second, is a 300 ms round trip — past what an action game is played
//! over — with a 100 ms jitter buffer and room to spare, and it is what a
//! target can feel at worst: shot behind cover half a second after reaching
//! it, and only by someone whose link really is that slow.

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
    /// The furthest back any shot is rewound, whatever the client claims
    /// and however slow its link: 500 ms by default. Each client is held to
    /// less — what its measured lag allows ([`allowed`](Self::allowed)). A
    /// rewind is bounded by the history kept too.
    pub max_rewind: Duration,
    /// What a client's measured lag is allowed on top, for what the server
    /// cannot see of it: the frame the client's input waited for its fixed
    /// step, a little jitter its buffer measured and the round trip did
    /// not. 50 ms (three ticks at 60 a second) by default.
    pub slack: Duration,
}

impl Default for LagCompensation {
    fn default() -> Self {
        Self {
            history: Duration::from_secs(1),
            max_rewind: Duration::from_millis(500),
            slack: Duration::from_millis(50),
        }
    }
}

/// What a server measured of one client's lag, none of it the client's
/// word ([`ReplicationServer::measured_lag`](crate::ReplicationServer::measured_lag)).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct MeasuredLag {
    /// The connection's smoothed round trip.
    pub rtt: Duration,
    /// How much the round trip varies (RFC 6298's `RTTVAR`).
    pub rtt_variation: Duration,
    /// How often the server sends this client a snapshot.
    pub snapshot_interval: Duration,
    /// How long the input being simulated waited in the server's queue.
    pub input_wait: Duration,
}

impl LagCompensation {
    /// How far back a client with `lag` may be rewound: its round trip at
    /// its plausible worst (`rtt + 4 × rtt_variation`, TCP's retransmission
    /// bound), its jitter buffer (two snapshot intervals: the wait for the
    /// next snapshot it measures as staleness, and the interval it draws
    /// behind on top — the jitter itself is in the round trip's variation),
    /// its input's wait in the queue, and [`slack`](Self::slack); at most
    /// [`max_rewind`](Self::max_rewind).
    pub fn allowed(&self, lag: &MeasuredLag) -> Duration {
        let link = lag.rtt + lag.rtt_variation * 4 + lag.snapshot_interval * 2 + lag.input_wait;
        (link + self.slack).min(self.max_rewind)
    }

    /// The moment to rewind to for an input that `reported` it was made
    /// at, with `now` the newest tick the server holds, a tick `step`
    /// seconds long and the client `allowed` this far back
    /// ([`allowed`](Self::allowed)): the report, no later than `now` and no
    /// earlier than the lesser of `allowed` and
    /// [`max_rewind`](Self::max_rewind) before it. `now` without a report
    /// — an input before the client had a snapshot, or none this step.
    pub fn target(
        &self,
        reported: Option<ViewTime>,
        now: f64,
        step: f32,
        allowed: Duration,
    ) -> f64 {
        let Some(reported) = reported else {
            return now;
        };
        let bound = allowed.min(self.max_rewind).as_secs_f64() / f64::from(step.max(1e-4));
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

    /// A client on a link of `rtt`, its snapshots every tick, its input
    /// straight through.
    fn link(rtt: u64) -> MeasuredLag {
        MeasuredLag {
            rtt: Duration::from_millis(rtt),
            snapshot_interval: Duration::from_secs_f32(STEP),
            ..MeasuredLag::default()
        }
    }

    #[test]
    fn a_report_within_the_bound_is_taken_as_it_is() {
        let lag = LagCompensation::default();
        let allowed = lag.allowed(&link(100));
        assert_eq!(lag.target(at(95.5), 100.0, STEP, allowed), 95.5);
    }

    #[test]
    fn the_far_past_is_clamped_at_max_rewind() {
        let lag = LagCompensation::default();
        // However slow the link, 500 ms at 60 a second is 30 ticks.
        let allowed = lag.allowed(&link(2000));
        assert_eq!(allowed, lag.max_rewind);
        let target = lag.target(at(10.0), 100.0, STEP, allowed);
        assert!((target - 70.0).abs() < 1e-5, "{target}");
    }

    #[test]
    fn a_quick_link_cannot_claim_the_cap() {
        let lag = LagCompensation::default();
        // 20 ms round trip, two intervals, 50 ms of slack: 103 ms, about
        // six ticks — not the 30 the cap would give.
        let allowed = lag.allowed(&link(20));
        assert!(
            (Duration::from_millis(100)..Duration::from_millis(110)).contains(&allowed),
            "{allowed:?}"
        );
        let target = lag.target(at(70.0), 100.0, STEP, allowed);
        assert!((93.0..95.0).contains(&target), "{target}");
        // What the server measured counts: a varying link, a slow
        // snapshot rate and a queued input each widen it.
        let slow = MeasuredLag {
            rtt_variation: Duration::from_millis(10),
            snapshot_interval: Duration::from_millis(50),
            input_wait: Duration::from_millis(30),
            ..link(20)
        };
        assert!(lag.allowed(&slow) > allowed + Duration::from_millis(130));
    }

    #[test]
    fn the_future_is_now() {
        let lag = LagCompensation::default();
        let allowed = lag.allowed(&link(100));
        assert_eq!(lag.target(at(150.25), 100.0, STEP, allowed), 100.0);
        assert_eq!(
            lag.target(
                Some(ViewTime {
                    tick: ServerTick::MAX,
                    fraction: 255
                }),
                100.0,
                STEP,
                allowed
            ),
            100.0
        );
        assert_eq!(lag.target(None, 100.0, STEP, allowed), 100.0);
    }
}
