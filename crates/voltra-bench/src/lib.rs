//! Benchmarks of the engine's hot paths, written as JSON, and the check
//! that compares two runs for regressions.
//!
//! In-house rather than `criterion`: CI needs a machine-readable result
//! and a comparer that fails a job, and `criterion` offers neither in a
//! form to rely on — its JSON is an undocumented directory tree under
//! `target/criterion`, its baselines live there too, and its change test
//! answers "probably changed" without a threshold per benchmark. What is
//! needed is small: warm up, size a sample, take N of them, keep the median
//! and the median absolute deviation, write JSON ([`harness`], [`stats`],
//! [`report`]); then match two reports by name and fail past each
//! benchmark's threshold ([`compare`]). docs/ARCHITECTURE.md, stage 107,
//! has the reasoning and the noise the thresholds come from.
//!
//! [`suite`] holds the benchmarks. `cargo xtask bench` builds this crate in
//! release and runs it; the binary's `--help` lists what it does.

#![warn(missing_docs)]

pub mod compare;
pub mod harness;
pub mod noise;
pub mod report;
pub mod stats;
pub mod suite;

pub use compare::{compare, Comparison, Row, Verdict};
pub use harness::Config;
pub use report::{BenchResult, Report, ReportError};
pub use stats::Stats;
