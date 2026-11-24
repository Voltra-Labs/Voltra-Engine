//! Timing a routine: warm it up, size a sample, take the samples.
//!
//! A routine is handed to [`measure`] as a closure that runs it a given
//! number of times and answers how long the timed part took — [`iter`] for
//! one that needs nothing fresh, [`iter_batched`] for one that consumes
//! what a setup made (a world stepped forward cannot be stepped from the
//! same place twice). Setup and dropping the outputs stay outside the clock.

use std::hint::black_box;
use std::time::{Duration, Instant};

use crate::stats::Stats;

/// How long a measurement warms up, how long each sample runs and how many
/// it takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Config {
    /// Time spent running the routine before any sample counts: caches,
    /// branch predictors, the allocator's free lists and the CPU's clock
    /// all settle while it runs. Also what sizes a sample.
    pub warm_up: Duration,
    /// How long one sample should take, its setup included. A routine
    /// faster than this runs
    /// several times per sample, so the clock's own cost and resolution
    /// (tens of nanoseconds) are a small share of what it reads.
    pub sample_time: Duration,
    /// Samples per benchmark. Odd, so the median is one of them.
    pub samples: usize,
}

impl Config {
    /// What `cargo xtask bench` and CI run.
    pub const DEFAULT: Self = Self {
        warm_up: Duration::from_millis(300),
        sample_time: Duration::from_millis(25),
        samples: 21,
    };

    /// A quick pass: to see the suite runs, not to compare its numbers.
    pub const QUICK: Self = Self {
        warm_up: Duration::from_millis(20),
        sample_time: Duration::from_millis(2),
        samples: 5,
    };
}

impl Default for Config {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Measures `run`, which runs the routine the number of times it is given
/// and answers how long that took, in ns per iteration divided by `per`
/// (a routine that does ten physics steps reports a step with `per` = 10).
///
/// `None` if the clock read a broken value, which [`Stats::of`] refuses.
pub fn measure(config: &Config, per: u32, mut run: impl FnMut(u64) -> Duration) -> Option<Stats> {
    let per = f64::from(per.max(1));
    let started = Instant::now();
    let mut iterations = 1u64;
    // Warm up, doubling the count while one run is shorter than a sample:
    // the last count's time is the estimate a sample is sized by. Sized by
    // the wall clock, setup included, not by the timed part alone: a
    // nanosecond routine behind a millisecond setup would otherwise ask
    // for millions of setups per sample.
    let per_iteration = loop {
        let wall = Instant::now();
        run(iterations);
        let wall = wall.elapsed();
        let estimate = wall.as_secs_f64() / iterations as f64;
        if started.elapsed() >= config.warm_up {
            break estimate;
        }
        if wall < config.sample_time {
            iterations = iterations.saturating_mul(2);
        }
    };
    let iterations = if per_iteration > 0.0 {
        (config.sample_time.as_secs_f64() / per_iteration).clamp(1.0, 1e9) as u64
    } else {
        1_000_000
    };
    let samples: Vec<f64> = (0..config.samples.max(1))
        .map(|_| run(iterations).as_nanos() as f64 / iterations as f64 / per)
        .collect();
    Stats::of(&samples, iterations)
}

/// A routine that needs nothing fresh per iteration. Its output is dropped
/// inside the timing, as a caller's would be.
pub fn iter<O>(mut routine: impl FnMut() -> O) -> impl FnMut(u64) -> Duration {
    move |iterations| {
        let start = Instant::now();
        for _ in 0..iterations {
            black_box(routine());
        }
        start.elapsed()
    }
}

/// A routine that consumes what `setup` makes. Every iteration's input is
/// made before the clock starts and every output dropped after it stops,
/// so neither is timed.
pub fn iter_batched<I, O>(
    mut setup: impl FnMut() -> I,
    mut routine: impl FnMut(I) -> O,
) -> impl FnMut(u64) -> Duration {
    move |iterations| {
        let inputs: Vec<I> = (0..iterations).map(|_| setup()).collect();
        let mut outputs = Vec::with_capacity(inputs.len());
        let start = Instant::now();
        for input in inputs {
            outputs.push(black_box(routine(input)));
        }
        let took = start.elapsed();
        drop(outputs);
        took
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fast_routine_runs_many_times_per_sample() {
        let stats = measure(&Config::QUICK, 1, iter(|| black_box(1u64) + 1)).expect("measured");
        assert!(stats.iterations > 1, "{stats:?}");
        assert_eq!(stats.samples, Config::QUICK.samples);
    }

    #[test]
    fn setup_is_not_timed() {
        // A setup far slower than the routine: if it were timed, a
        // sample would read at least its millisecond.
        let stats = measure(
            &Config::QUICK,
            1,
            iter_batched(
                || std::thread::sleep(Duration::from_millis(1)),
                |()| black_box(2u64) * 3,
            ),
        )
        .expect("measured");
        assert!(stats.median < 500_000.0, "{stats:?}");
    }

    #[test]
    fn per_divides_the_reported_time() {
        let config = Config {
            warm_up: Duration::ZERO,
            sample_time: Duration::ZERO,
            samples: 3,
        };
        let fixed = |_| Duration::from_micros(10);
        let whole = measure(&config, 1, fixed).expect("measured");
        let tenth = measure(&config, 10, fixed).expect("measured");
        assert_eq!(whole.median, 10_000.0);
        assert_eq!(tenth.median, 1_000.0);
    }
}
