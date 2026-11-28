//! The benchmarks: the engine's hot paths, each at the sizes a game reaches.
//!
//! Every benchmark builds its own world — generated, not read from a scene
//! file that someone may edit — so a change in its number is a change in
//! the code it times. Each group lives in a file of its own under
//! `suite/`; [`all`] is the list, in the order a report keeps.

use std::path::PathBuf;

use crate::harness::Config;
use crate::report::{BenchResult, Report};
use crate::stats::Stats;

mod batch;
mod draw3d;
mod frame;
mod interest;
mod keyframes;
mod lag;
mod physics;
mod prefab;
mod scene;
mod text;
mod tilemap;
mod ui;

/// What running a benchmark came to.
#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    /// It ran; the timing.
    Measured(Stats),
    /// It could not run here, and why — no GPU adapter, a missing asset.
    Skipped(String),
}

impl From<Option<Stats>> for Outcome {
    fn from(stats: Option<Stats>) -> Self {
        stats.map_or_else(
            || Self::Skipped("the clock read a broken value".to_owned()),
            Self::Measured,
        )
    }
}

/// The body of a benchmark: builds what it times, then measures it.
type Routine = Box<dyn Fn(&Config) -> Outcome>;

/// One benchmark of the suite.
pub struct Bench {
    /// `group/case/size`, unique in the suite.
    pub name: String,
    /// How far, in percent, its median may grow before the comparer calls
    /// it a regression. Set from the run-to-run noise measured with
    /// `voltra-bench noise` (docs/ARCHITECTURE.md, stage 107).
    pub threshold_percent: f64,
    run: Routine,
}

impl Bench {
    /// A benchmark called `name`, regressing past `threshold_percent`.
    pub fn new(
        name: impl Into<String>,
        threshold_percent: f64,
        run: impl Fn(&Config) -> Outcome + 'static,
    ) -> Self {
        Self {
            name: name.into(),
            threshold_percent,
            run: Box::new(run),
        }
    }

    /// Runs it under `config`.
    pub fn run(&self, config: &Config) -> BenchResult {
        match (self.run)(config) {
            Outcome::Measured(stats) => {
                BenchResult::measured(&self.name, self.threshold_percent, stats)
            }
            Outcome::Skipped(why) => BenchResult::skipped(&self.name, self.threshold_percent, why),
        }
    }
}

/// Every benchmark, in report order.
pub fn all() -> Vec<Bench> {
    let mut benches = Vec::new();
    benches.extend(batch::benches());
    benches.extend(tilemap::benches());
    benches.extend(draw3d::benches());
    benches.extend(physics::benches());
    benches.extend(ui::benches());
    benches.extend(text::benches());
    benches.extend(scene::benches());
    benches.extend(prefab::benches());
    benches.extend(keyframes::benches());
    benches.extend(interest::benches());
    benches.extend(lag::benches());
    benches.extend(frame::benches());
    benches
}

/// Runs each of `benches` whose name contains `filter` (all of them for
/// `None`), handing each result to `each` as it lands — a long suite
/// shows its progress — and answers the report.
pub fn run(
    benches: &[Bench],
    config: &Config,
    filter: Option<&str>,
    mut each: impl FnMut(&BenchResult),
) -> Report {
    let mut report = Report::new();
    for bench in benches
        .iter()
        .filter(|b| filter.is_none_or(|f| b.name.contains(f)))
    {
        let result = bench.run(config);
        each(&result);
        report.results.push(result);
    }
    report
}

/// The workspace's asset root, which the text and frame benchmarks read the
/// shipped font and scene from. Fixed at build time: the suite runs from
/// the checkout it was built in.
fn assets() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_unique_and_thresholds_positive() {
        let benches = all();
        let mut names: Vec<&str> = benches.iter().map(|b| b.name.as_str()).collect();
        let count = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), count, "a name is used twice");
        assert!(benches.iter().all(|b| b.threshold_percent > 0.0));
    }

    #[test]
    fn a_filter_runs_only_what_it_names() {
        let benches = vec![
            Bench::new("a/one", 10.0, |_| Outcome::Skipped("test".to_owned())),
            Bench::new("b/two", 10.0, |_| Outcome::Skipped("test".to_owned())),
        ];
        let mut seen = Vec::new();
        let report = run(&benches, &Config::QUICK, Some("b/"), |r| {
            seen.push(r.name.clone())
        });
        assert_eq!(seen, ["b/two"]);
        assert_eq!(report.results.len(), 1);
    }

    /// Every benchmark runs once, quickly, and measures: a broken
    /// routine is found here, not in CI's release run. The GPU ones may
    /// skip on a machine with no adapter.
    #[test]
    fn every_cpu_benchmark_runs() {
        let config = Config {
            warm_up: std::time::Duration::ZERO,
            sample_time: std::time::Duration::ZERO,
            samples: 1,
        };
        for bench in all() {
            let result = bench.run(&config);
            let gpu = bench.name.starts_with("frame/") || bench.name.contains("upload");
            assert!(
                result.stats.is_some() || gpu,
                "{} skipped: {:?}",
                bench.name,
                result.skipped
            );
        }
    }
}
