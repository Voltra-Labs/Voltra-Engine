use std::hint::black_box;
use std::time::Duration;

use super::*;
use crate::harness::{iter, measure, Config};
use crate::report::BenchResult;
use crate::stats::Stats;

fn stats(median: f64) -> Stats {
    Stats {
        median,
        mad: median * 0.01,
        min: median,
        max: median,
        samples: 5,
        iterations: 1,
    }
}

fn report(results: &[(&str, f64)]) -> Report {
    Report {
        results: results
            .iter()
            .map(|(name, median)| BenchResult::measured(*name, 10.0, stats(*median)))
            .collect(),
        ..Report::new()
    }
}

fn optimized(mut report: Report) -> Report {
    report.machine.optimized = true;
    report
}

fn verdict(comparison: &Comparison, name: &str) -> Verdict {
    comparison
        .rows
        .iter()
        .find(|row| row.name == name)
        .map(|row| row.verdict)
        .expect("the row is there")
}

#[test]
fn the_same_numbers_are_unchanged_and_pass() {
    let base = optimized(report(&[("a", 100.0), ("b", 5.0)]));
    let comparison = compare(&base, &base, None).expect("comparable");
    assert!(comparison.passed());
    assert_eq!(verdict(&comparison, "a"), Verdict::Unchanged);
}

#[test]
fn slower_past_the_threshold_regresses_and_within_it_does_not() {
    let base = optimized(report(&[("past", 100.0), ("within", 100.0)]));
    // Thresholds are 10%: 112 is past it, 109 is not.
    let head = optimized(report(&[("past", 112.0), ("within", 109.0)]));
    let comparison = compare(&base, &head, None).expect("comparable");
    assert_eq!(verdict(&comparison, "past"), Verdict::Regressed);
    assert_eq!(verdict(&comparison, "within"), Verdict::Unchanged);
    assert!(!comparison.passed());
    let failing: Vec<&str> = comparison.regressions().map(|r| r.name.as_str()).collect();
    assert_eq!(failing, ["past"]);
    assert!(comparison.table().contains("REGRESSED"));
}

#[test]
fn faster_past_the_threshold_is_an_improvement_not_a_failure() {
    let base = optimized(report(&[("a", 100.0)]));
    let head = optimized(report(&[("a", 50.0)]));
    let comparison = compare(&base, &head, None).expect("comparable");
    assert_eq!(verdict(&comparison, "a"), Verdict::Improved);
    assert!(comparison.passed());
}

#[test]
fn a_given_threshold_overrides_each_benchmarks_own() {
    let base = optimized(report(&[("a", 100.0)]));
    let head = optimized(report(&[("a", 112.0)]));
    assert!(compare(&base, &head, Some(20.0))
        .expect("comparable")
        .passed());
    assert!(!compare(&base, &head, Some(5.0))
        .expect("comparable")
        .passed());
}

#[test]
fn added_removed_and_skipped_benchmarks_are_listed_and_never_fail() {
    let mut base = optimized(report(&[("kept", 100.0), ("gone", 100.0)]));
    base.results
        .push(BenchResult::skipped("gpu", 10.0, "no adapter"));
    let mut head = optimized(report(&[("kept", 100.0), ("added", 1.0)]));
    head.results
        .push(BenchResult::measured("gpu", 10.0, stats(1e9)));
    let comparison = compare(&base, &head, None).expect("comparable");
    assert_eq!(verdict(&comparison, "added"), Verdict::New);
    assert_eq!(verdict(&comparison, "gone"), Verdict::Removed);
    assert_eq!(verdict(&comparison, "gpu"), Verdict::Skipped);
    assert!(comparison.passed());
}

#[test]
fn reports_of_different_machines_or_a_debug_build_are_refused() {
    let base = optimized(report(&[("a", 1.0)]));
    let mut other = base.clone();
    other.machine.arch = "elsewhere".to_owned();
    assert!(compare(&base, &other, None).is_err());

    let debug = report(&[("a", 1.0)]);
    let mut debug = debug;
    debug.machine.optimized = false;
    let mut debug_base = base.clone();
    debug_base.machine.optimized = false;
    assert!(compare(&debug_base, &debug, None).is_err());
}

/// The work both versions of the function do: a sum over a buffer, which
/// the optimizer cannot fold away through `black_box`.
fn work(values: &[u64]) -> u64 {
    values
        .iter()
        .fold(0u64, |sum, v| sum.wrapping_add(black_box(*v) * 3))
}

/// The function as it was.
fn original(values: &[u64]) -> u64 {
    work(values)
}

/// The same function, deliberately made slower: the same answer through
/// three times the work.
fn slowed(values: &[u64]) -> u64 {
    let answer = work(values);
    black_box(work(values));
    black_box(work(values));
    answer
}

#[test]
fn a_deliberately_slowed_function_trips_the_check() {
    // The real harness on a real function, not made-up medians: what CI
    // does to the suite, done to one routine and a slowed copy of it.
    let values: Vec<u64> = (0..4096).collect();
    assert_eq!(original(&values), slowed(&values));
    let config = Config {
        warm_up: Duration::from_millis(30),
        sample_time: Duration::from_millis(3),
        samples: 11,
    };
    let time = |f: fn(&[u64]) -> u64| {
        let stats = measure(&config, 1, iter(|| f(&values))).expect("measured");
        optimized(Report {
            results: vec![BenchResult::measured("sum/4k", 25.0, stats)],
            ..Report::new()
        })
    };
    let base = time(original);
    let head = time(slowed);
    let comparison = compare(&base, &head, None).expect("comparable");
    let row = &comparison.rows[0];
    assert_eq!(
        row.verdict,
        Verdict::Regressed,
        "three times the work must read as slower:\n{}",
        comparison.table()
    );
    assert!(!comparison.passed());
}
