//! What a benchmark's samples come to: a median and its spread.

use serde::{Deserialize, Serialize};

/// The summary of one benchmark's samples, in nanoseconds per iteration.
///
/// The median and the median absolute deviation rather than the mean and
/// the standard deviation: noise on a shared machine is one-sided (a
/// preempted sample is slow, never fast) and comes in bursts, and one
/// sample a hundred times too slow moves a mean and a standard deviation as
/// far as it likes. It moves the median by at most one rank.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Stats {
    /// The middle sample, ns per iteration.
    pub median: f64,
    /// The median of every sample's distance from `median`, ns. Unscaled:
    /// multiply by 1.4826 for a standard deviation's equivalent under
    /// normal noise.
    pub mad: f64,
    /// The fastest sample, ns per iteration.
    pub min: f64,
    /// The slowest sample, ns per iteration.
    pub max: f64,
    /// How many samples were taken.
    pub samples: usize,
    /// How many iterations each sample timed.
    pub iterations: u64,
}

impl Stats {
    /// The summary of `samples` (ns per iteration, each over `iterations`
    /// iterations), or `None` when there are none or any is not a finite,
    /// non-negative number — a clock that went backwards is a broken run,
    /// not a fast one.
    pub fn of(samples: &[f64], iterations: u64) -> Option<Self> {
        if samples.is_empty() || samples.iter().any(|s| !s.is_finite() || *s < 0.0) {
            return None;
        }
        let mut sorted = samples.to_vec();
        sorted.sort_by(f64::total_cmp);
        let median = median_of_sorted(&sorted);
        let mut deviations: Vec<f64> = sorted.iter().map(|s| (s - median).abs()).collect();
        deviations.sort_by(f64::total_cmp);
        Some(Self {
            median,
            mad: median_of_sorted(&deviations),
            min: sorted[0],
            max: sorted[sorted.len() - 1],
            samples: sorted.len(),
            iterations,
        })
    }

    /// `mad` as a share of `median`: how noisy the samples were. Zero for a
    /// median of zero.
    pub fn relative_mad(&self) -> f64 {
        if self.median > 0.0 {
            self.mad / self.median
        } else {
            0.0
        }
    }
}

/// The middle of `sorted`, the mean of the two middle values for an even
/// count. `sorted` is not empty.
fn median_of_sorted(sorted: &[f64]) -> f64 {
    let middle = sorted.len() / 2;
    if sorted.len() % 2 == 1 {
        sorted[middle]
    } else {
        (sorted[middle - 1] + sorted[middle]) / 2.0
    }
}

/// Formats `ns` with the unit that keeps it between 1 and 1000.
pub fn human(ns: f64) -> String {
    if ns >= 1e9 {
        format!("{:.3} s", ns / 1e9)
    } else if ns >= 1e6 {
        format!("{:.3} ms", ns / 1e6)
    } else if ns >= 1e3 {
        format!("{:.3} µs", ns / 1e3)
    } else {
        format!("{ns:.1} ns")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_odd_count_has_its_middle_sample_as_median() {
        let stats = Stats::of(&[5.0, 1.0, 3.0], 1).expect("samples");
        assert_eq!(stats.median, 3.0);
        assert_eq!((stats.min, stats.max), (1.0, 5.0));
        // Deviations 2, 2, 0 → median 2.
        assert_eq!(stats.mad, 2.0);
    }

    #[test]
    fn an_even_count_averages_the_two_middle_samples() {
        let stats = Stats::of(&[4.0, 1.0, 3.0, 2.0], 8).expect("samples");
        assert_eq!(stats.median, 2.5);
        assert_eq!(stats.iterations, 8);
        assert_eq!(stats.samples, 4);
    }

    #[test]
    fn one_wild_sample_barely_moves_the_median_or_the_mad() {
        let calm = Stats::of(&[10.0, 10.0, 11.0, 9.0, 10.0], 1).expect("samples");
        let wild = Stats::of(&[10.0, 10.0, 11.0, 9.0, 1000.0], 1).expect("samples");
        assert_eq!(calm.median, wild.median);
        // Deviations 0, 0, 0, 1, 1 then 0, 0, 1, 1, 990: up one rank.
        assert_eq!((calm.mad, wild.mad), (0.0, 1.0));
        assert_eq!(wild.max, 1000.0);
    }

    #[test]
    fn no_samples_or_a_broken_one_is_no_summary() {
        assert!(Stats::of(&[], 1).is_none());
        assert!(Stats::of(&[1.0, f64::NAN], 1).is_none());
        assert!(Stats::of(&[1.0, -1.0], 1).is_none());
    }

    #[test]
    fn units_scale_with_the_value() {
        assert_eq!(human(12.0), "12.0 ns");
        assert_eq!(human(1_500.0), "1.500 µs");
        assert_eq!(human(2_000_000.0), "2.000 ms");
        assert_eq!(human(3e9), "3.000 s");
    }
}
