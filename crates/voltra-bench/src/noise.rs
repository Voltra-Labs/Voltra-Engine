//! How much a benchmark moves between runs of the same code: what its
//! regression threshold has to clear.
//!
//! Two numbers per benchmark. The **spread** is the slowest run's median
//! over the fastest's: what one run against one run would see. The
//! **A/A change** is what the CI check itself would see comparing the
//! code with itself — the best of `k` runs against the best of `k` others,
//! interleaved as CI interleaves the base's and the head's — the worst of
//! it over every window of `2k` consecutive runs. The threshold is set
//! from the A/A change when there are enough runs for one.

use std::fmt::Write as _;

use crate::report::Report;

/// The threshold no benchmark goes below, in percent: under it, a change
/// of compiler or a runner's image moves numbers past it for no reason in
/// the code.
pub const FLOOR_PERCENT: f64 = 15.0;

/// How a threshold is set from the noise: this many times the worst
/// change measured between runs of identical code, so a comparison as
/// unlucky as the worst one seen still passes with room to spare.
pub const MARGIN: f64 = 2.0;

/// One benchmark's noise across runs of identical code.
#[derive(Debug, Clone, PartialEq)]
pub struct NoiseRow {
    /// The benchmark.
    pub name: String,
    /// Each run's median, ns per iteration, in run order.
    pub medians: Vec<f64>,
    /// The slowest median over the fastest, minus one.
    pub spread: f64,
    /// The worst |best of `k` over best of `k` − 1| between interleaved
    /// halves of `2k` consecutive runs. `None` with fewer than `2k` runs.
    pub aa_change: Option<f64>,
    /// The largest within-run MAD as a share of its median.
    pub worst_relative_mad: f64,
    /// The threshold this noise asks for, in percent: [`MARGIN`] times the
    /// A/A change (the spread without one), at least [`FLOOR_PERCENT`],
    /// rounded up to 5.
    pub suggested_percent: f64,
    /// The threshold the suite sets now, in percent.
    pub current_percent: f64,
}

/// The noise of every benchmark measured in `runs`, with A/A comparisons
/// of the best of `best_of` runs a side.
pub fn noise(runs: &[Report], best_of: usize) -> Vec<NoiseRow> {
    let Some(first) = runs.first() else {
        return Vec::new();
    };
    let best_of = best_of.max(1);
    first
        .results
        .iter()
        .filter_map(|result| {
            let measured: Vec<_> = runs
                .iter()
                .filter_map(|run| run.get(&result.name)?.stats.clone())
                .collect();
            if measured.is_empty() {
                return None;
            }
            let medians: Vec<f64> = measured.iter().map(|s| s.median).collect();
            let spread = spread(&medians);
            let aa_change = aa_change(&medians, best_of);
            let worst_relative_mad = measured
                .iter()
                .map(|s| s.relative_mad())
                .fold(0.0, f64::max);
            Some(NoiseRow {
                name: result.name.clone(),
                medians,
                spread,
                aa_change,
                worst_relative_mad,
                suggested_percent: suggest(aa_change.unwrap_or(spread)),
                current_percent: result.threshold_percent,
            })
        })
        .collect()
}

/// Slowest over fastest, minus one.
fn spread(medians: &[f64]) -> f64 {
    let fastest = medians.iter().copied().fold(f64::INFINITY, f64::min);
    let slowest = medians.iter().copied().fold(0.0, f64::max);
    if fastest > 0.0 && fastest.is_finite() {
        slowest / fastest - 1.0
    } else {
        0.0
    }
}

/// The worst A/A change over every window of `2 × best_of` consecutive
/// medians, split alternately between the two sides.
fn aa_change(medians: &[f64], best_of: usize) -> Option<f64> {
    let window = 2 * best_of;
    medians
        .windows(window)
        .filter_map(|runs| {
            let side = |parity: usize| {
                runs.iter()
                    .skip(parity)
                    .step_by(2)
                    .copied()
                    .fold(f64::INFINITY, f64::min)
            };
            let (base, head) = (side(0), side(1));
            (base > 0.0 && base.is_finite() && head.is_finite()).then(|| (head / base - 1.0).abs())
        })
        .reduce(f64::max)
}

/// The threshold, in percent, a measured `noise` (a share, 0.1 = 10%) asks
/// for.
pub fn suggest(noise: f64) -> f64 {
    let wanted = (noise * 100.0 * MARGIN).max(FLOOR_PERCENT);
    (wanted / 5.0).ceil() * 5.0
}

/// A table of `rows`: spread, A/A change, within-run noise, suggested and
/// current thresholds.
pub fn table(rows: &[NoiseRow]) -> String {
    let width = rows.iter().map(|r| r.name.len()).max().unwrap_or(0).max(9);
    let mut out = String::new();
    let _ = writeln!(
        out,
        "{:width$}  {:>4}  {:>7}  {:>7}  {:>8}  {:>9}  {:>7}",
        "benchmark", "runs", "spread", "A/A", "max MAD", "suggested", "current"
    );
    for row in rows {
        let aa = row
            .aa_change
            .map_or_else(|| "-".to_owned(), |c| format!("{:.1}%", c * 100.0));
        let _ = writeln!(
            out,
            "{:width$}  {:>4}  {:>6.1}%  {:>7}  {:>7.1}%  {:>8.0}%  {:>6.0}%",
            row.name,
            row.medians.len(),
            row.spread * 100.0,
            aa,
            row.worst_relative_mad * 100.0,
            row.suggested_percent,
            row.current_percent,
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::report::BenchResult;
    use crate::stats::Stats;

    fn run(median: f64) -> Report {
        let stats = Stats::of(&[median * 0.98, median, median * 1.04], 1).expect("samples");
        Report {
            results: vec![BenchResult::measured("a", 15.0, stats)],
            ..Report::new()
        }
    }

    #[test]
    fn the_spread_is_slowest_over_fastest() {
        let rows = noise(&[run(100.0), run(108.0), run(104.0)], 3);
        assert_eq!(rows.len(), 1);
        assert!((rows[0].spread - 0.08).abs() < 1e-9);
        assert!((rows[0].worst_relative_mad - 0.02).abs() < 1e-9);
        // Too few runs for an A/A: the spread sets it, 2 × 8% → 20%.
        assert_eq!(rows[0].aa_change, None);
        assert_eq!(rows[0].suggested_percent, 20.0);
        assert_eq!(rows[0].current_percent, 15.0);
    }

    #[test]
    fn the_aa_change_compares_interleaved_bests() {
        // Best of the odd runs 100, of the even 110: one window, 10%.
        let medians = [100.0, 130.0, 120.0, 110.0];
        let change = aa_change(&medians, 2).expect("a window");
        assert!((change - 0.1).abs() < 1e-9, "{change}");
        // Bests of one: the worst neighbouring pair, 130 over 100.
        let change = aa_change(&medians, 1).expect("windows");
        assert!((change - 0.3).abs() < 1e-9, "{change}");
        assert_eq!(aa_change(&medians, 3), None);
    }

    #[test]
    fn the_aa_change_sets_the_suggestion_when_there_is_one() {
        let runs: Vec<Report> = [100.0, 150.0, 104.0, 102.0].into_iter().map(run).collect();
        let rows = noise(&runs, 2);
        // Bests 100 and 102: 2%, under the floor — although one run was
        // 50% slow, which is what taking the best of several is for.
        assert_eq!(rows[0].suggested_percent, FLOOR_PERCENT);
        assert!(rows[0].spread > 0.4);
    }

    #[test]
    fn a_quiet_benchmark_still_gets_the_floor() {
        assert_eq!(suggest(0.0), FLOOR_PERCENT);
        assert_eq!(suggest(0.05), FLOOR_PERCENT);
        assert_eq!(suggest(0.13), 30.0);
    }

    #[test]
    fn no_runs_is_no_rows() {
        assert!(noise(&[], 3).is_empty());
    }
}
