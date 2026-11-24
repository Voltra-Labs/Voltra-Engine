//! A run of the suite as JSON: what CI keeps and the comparer reads.

use std::fmt;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::stats::Stats;

/// The version of the JSON layout. A report of another version is refused
/// by [`Report::from_json`] rather than half-read: comparing fields that
/// changed meaning would be a verdict about nothing.
pub const FORMAT: u32 = 1;

/// One run of the suite — or the best of several, from [`Report::best_of`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Report {
    /// [`FORMAT`] when written.
    pub format: u32,
    /// What it ran on.
    pub machine: Machine,
    /// Every benchmark the run was asked for, in suite order.
    pub results: Vec<BenchResult>,
}

/// The machine and build a report was measured with: numbers from two
/// machines, or from an unoptimized build, do not compare.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Machine {
    /// `std::env::consts::OS`.
    pub os: String,
    /// `std::env::consts::ARCH`.
    pub arch: String,
    /// Logical CPUs available to the process.
    pub cpus: usize,
    /// Whether the suite was built with optimizations (release).
    pub optimized: bool,
}

impl Machine {
    /// The machine this process runs on, and how it was built.
    pub fn current() -> Self {
        Self {
            os: std::env::consts::OS.to_owned(),
            arch: std::env::consts::ARCH.to_owned(),
            cpus: std::thread::available_parallelism().map_or(1, usize::from),
            optimized: !cfg!(debug_assertions),
        }
    }
}

/// One benchmark's outcome.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BenchResult {
    /// `group/case/size`, unique in the suite: what two reports are
    /// matched by.
    pub name: String,
    /// How far, in percent, its median may grow before the comparer calls
    /// it a regression: set per benchmark from its measured noise.
    pub threshold_percent: f64,
    /// The timing, ns per iteration. `None` when it was skipped.
    pub stats: Option<Stats>,
    /// Why it was not measured — no GPU adapter, say. `None` when it was.
    pub skipped: Option<String>,
    /// How many runs this result is the best of.
    pub runs: usize,
    /// Across those runs, the slowest median over the fastest, minus one:
    /// the run-to-run noise the threshold has to clear. `None` for one run.
    pub run_spread: Option<f64>,
}

impl BenchResult {
    /// A measured result of one run.
    pub fn measured(name: impl Into<String>, threshold_percent: f64, stats: Stats) -> Self {
        Self {
            name: name.into(),
            threshold_percent,
            stats: Some(stats),
            skipped: None,
            runs: 1,
            run_spread: None,
        }
    }

    /// A result that could not be measured, and why.
    pub fn skipped(
        name: impl Into<String>,
        threshold_percent: f64,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            threshold_percent,
            stats: None,
            skipped: Some(reason.into()),
            runs: 1,
            run_spread: None,
        }
    }

    /// The median, ns per iteration, when it was measured.
    pub fn median(&self) -> Option<f64> {
        self.stats.as_ref().map(|s| s.median)
    }
}

/// Why a report could not be read, written or combined.
#[derive(Debug)]
pub enum ReportError {
    /// The file would not read or write.
    Io(String, std::io::Error),
    /// The text is not a report.
    Json(serde_json::Error),
    /// A report of another layout version.
    Format {
        /// The version in the file.
        found: u32,
    },
    /// Reports from different machines or builds, which do not combine.
    Mismatch(String),
    /// Nothing to combine.
    Empty,
}

impl fmt::Display for ReportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(path, e) => write!(f, "{path}: {e}"),
            Self::Json(e) => write!(f, "not a benchmark report: {e}"),
            Self::Format { found } => write!(
                f,
                "a report of format {found}; this build reads format {FORMAT}"
            ),
            Self::Mismatch(why) => write!(f, "reports do not combine: {why}"),
            Self::Empty => write!(f, "no reports to combine"),
        }
    }
}

impl std::error::Error for ReportError {}

impl Report {
    /// An empty report of this machine.
    pub fn new() -> Self {
        Self {
            format: FORMAT,
            machine: Machine::current(),
            results: Vec::new(),
        }
    }

    /// The result named `name`.
    pub fn get(&self, name: &str) -> Option<&BenchResult> {
        self.results.iter().find(|r| r.name == name)
    }

    /// Pretty JSON, a result per block: diffable when kept as an artifact.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("a report is plain data and always serializes")
    }

    /// Reads a report back, refusing one of another [`FORMAT`].
    pub fn from_json(text: &str) -> Result<Self, ReportError> {
        // The version first, on its own: a future layout may not parse as
        // this one, and "format 2" says more than a missing-field error.
        #[derive(Deserialize)]
        struct Version {
            format: u32,
        }
        let version: Version = serde_json::from_str(text).map_err(ReportError::Json)?;
        if version.format != FORMAT {
            return Err(ReportError::Format {
                found: version.format,
            });
        }
        serde_json::from_str(text).map_err(ReportError::Json)
    }

    /// Reads the report at `path`.
    pub fn read(path: &Path) -> Result<Self, ReportError> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| ReportError::Io(path.display().to_string(), e))?;
        Self::from_json(&text)
    }

    /// Writes the report to `path`, making its directory.
    pub fn write(&self, path: &Path) -> Result<(), ReportError> {
        let io = |e| ReportError::Io(path.display().to_string(), e);
        if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(dir).map_err(io)?;
        }
        std::fs::write(path, self.to_json()).map_err(io)
    }

    /// The fastest median of each benchmark across `reports`, with how
    /// far apart the runs were.
    ///
    /// The best rather than the mean of the runs: what slows a run on a
    /// shared machine — another process, a migrated thread, a throttled
    /// clock — only ever adds time, so the fastest run is the closest to
    /// what the code costs. A benchmark measured in some runs and skipped
    /// in others keeps the measured ones; one skipped in all stays skipped.
    pub fn best_of(reports: &[Report]) -> Result<Report, ReportError> {
        let first = reports.first().ok_or(ReportError::Empty)?;
        if let Some(other) = reports.iter().find(|r| r.machine != first.machine) {
            return Err(ReportError::Mismatch(format!(
                "{:?} and {:?}",
                first.machine, other.machine
            )));
        }
        let mut names: Vec<&str> = Vec::new();
        for report in reports {
            for result in &report.results {
                if !names.contains(&result.name.as_str()) {
                    names.push(&result.name);
                }
            }
        }
        let results = names
            .into_iter()
            .map(|name| {
                let all: Vec<&BenchResult> = reports.iter().filter_map(|r| r.get(name)).collect();
                combine(&all)
            })
            .collect();
        Ok(Report {
            format: FORMAT,
            machine: first.machine.clone(),
            results,
        })
    }
}

impl Default for Report {
    fn default() -> Self {
        Self::new()
    }
}

/// One benchmark's results across runs, combined as [`Report::best_of`]
/// says. `all` is not empty.
fn combine(all: &[&BenchResult]) -> BenchResult {
    let measured: Vec<&BenchResult> = all.iter().copied().filter(|r| r.stats.is_some()).collect();
    let runs = all.iter().map(|r| r.runs).sum();
    let Some(best) = measured.iter().copied().min_by(|a, b| {
        a.median()
            .unwrap_or(0.0)
            .total_cmp(&b.median().unwrap_or(0.0))
    }) else {
        return BenchResult {
            runs,
            ..all[0].clone()
        };
    };
    let medians = measured.iter().filter_map(|r| r.median());
    let fastest = medians.clone().fold(f64::INFINITY, f64::min);
    let slowest = medians.fold(0.0, f64::max);
    // A spread recorded inside an input (already best of several) is
    // kept if it was wider than what is seen across the inputs.
    let inner = measured
        .iter()
        .filter_map(|r| r.run_spread)
        .fold(0.0, f64::max);
    let across = if fastest > 0.0 {
        slowest / fastest - 1.0
    } else {
        0.0
    };
    BenchResult {
        runs,
        run_spread: (runs > 1).then_some(across.max(inner)),
        ..best.clone()
    }
}

#[cfg(test)]
mod tests;
