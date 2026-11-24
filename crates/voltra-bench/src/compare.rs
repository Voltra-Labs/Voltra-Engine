//! Two reports side by side: which benchmarks got slower past their
//! threshold.

use std::fmt::Write as _;

use crate::report::{Report, ReportError};
use crate::stats::human;

/// What one benchmark did between the base and the head.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// Slower than the base by more than its threshold: fails the check.
    Regressed,
    /// Faster than the base by more than its threshold.
    Improved,
    /// Within its threshold of the base: noise, as far as can be told.
    Unchanged,
    /// Only in the head: a benchmark added, nothing to compare with.
    New,
    /// Only in the base: a benchmark removed or renamed.
    Removed,
    /// Skipped on one side or both (no GPU adapter, say).
    Skipped,
}

/// One benchmark's line of a [`Comparison`].
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    /// The benchmark's name.
    pub name: String,
    /// The base's median, ns per iteration.
    pub base: Option<f64>,
    /// The head's median, ns per iteration.
    pub head: Option<f64>,
    /// The threshold applied, in percent.
    pub threshold_percent: f64,
    /// What the change amounts to.
    pub verdict: Verdict,
}

impl Row {
    /// Head over base, when both were measured.
    pub fn ratio(&self) -> Option<f64> {
        match (self.base, self.head) {
            (Some(base), Some(head)) if base > 0.0 => Some(head / base),
            _ => None,
        }
    }
}

/// Every benchmark of two reports, compared.
#[derive(Debug, Clone, PartialEq)]
pub struct Comparison {
    /// In the head's order, then what only the base has.
    pub rows: Vec<Row>,
}

/// Compares `head` against `base`.
///
/// A benchmark regresses when its head median is more than its threshold
/// above the base's: `head > base × (1 + threshold / 100)`. The threshold
/// is the head's own for that benchmark (the suite sets each from its
/// measured noise), or `threshold_percent` for all of them when given.
/// It improves past the same margin the other way; anything between is
/// unchanged. Benchmarks on one side only, or skipped, are listed and
/// never fail the check.
///
/// Refuses reports of different machines or builds, and an unoptimized
/// one: their numbers differ for reasons that are not the change.
pub fn compare(
    base: &Report,
    head: &Report,
    threshold_percent: Option<f64>,
) -> Result<Comparison, ReportError> {
    if base.machine != head.machine {
        return Err(ReportError::Mismatch(format!(
            "base ran on {:?}, head on {:?}",
            base.machine, head.machine
        )));
    }
    if !head.machine.optimized {
        return Err(ReportError::Mismatch(
            "the reports are of an unoptimized build; benchmark in release".to_owned(),
        ));
    }
    let mut rows: Vec<Row> = head
        .results
        .iter()
        .map(|result| {
            let threshold = threshold_percent.unwrap_or(result.threshold_percent);
            let before = base.get(&result.name);
            let base_median = before.and_then(|b| b.median());
            let head_median = result.median();
            let verdict = match (before, base_median, head_median) {
                (None, _, _) => Verdict::New,
                (Some(_), Some(b), Some(h)) => judge(b, h, threshold),
                _ => Verdict::Skipped,
            };
            Row {
                name: result.name.clone(),
                base: base_median,
                head: head_median,
                threshold_percent: threshold,
                verdict,
            }
        })
        .collect();
    rows.extend(
        base.results
            .iter()
            .filter(|b| head.get(&b.name).is_none())
            .map(|b| Row {
                name: b.name.clone(),
                base: b.median(),
                head: None,
                threshold_percent: threshold_percent.unwrap_or(b.threshold_percent),
                verdict: Verdict::Removed,
            }),
    );
    Ok(Comparison { rows })
}

/// The verdict on a median going from `base` to `head` ns.
fn judge(base: f64, head: f64, threshold_percent: f64) -> Verdict {
    let margin = 1.0 + threshold_percent.max(0.0) / 100.0;
    if head > base * margin {
        Verdict::Regressed
    } else if head * margin < base {
        Verdict::Improved
    } else {
        Verdict::Unchanged
    }
}

impl Comparison {
    /// The rows that fail the check.
    pub fn regressions(&self) -> impl Iterator<Item = &Row> {
        self.rows
            .iter()
            .filter(|row| row.verdict == Verdict::Regressed)
    }

    /// Whether nothing regressed.
    pub fn passed(&self) -> bool {
        self.regressions().next().is_none()
    }

    /// A table for a terminal or a CI log: name, base, head, change,
    /// threshold, verdict.
    pub fn table(&self) -> String {
        let width = self
            .rows
            .iter()
            .map(|row| row.name.len())
            .max()
            .unwrap_or(0)
            .max("benchmark".len());
        let mut out = String::new();
        let _ = writeln!(
            out,
            "{:width$}  {:>12}  {:>12}  {:>8}  {:>6}  verdict",
            "benchmark", "base", "head", "change", "limit"
        );
        let time = |ns: Option<f64>| ns.map_or_else(|| "-".to_owned(), human);
        for row in &self.rows {
            let change = row
                .ratio()
                .map_or_else(|| "-".to_owned(), |r| format!("{:+.1}%", (r - 1.0) * 100.0));
            let verdict = match row.verdict {
                Verdict::Regressed => "REGRESSED",
                Verdict::Improved => "faster",
                Verdict::Unchanged => "ok",
                Verdict::New => "new",
                Verdict::Removed => "removed",
                Verdict::Skipped => "skipped",
            };
            let _ = writeln!(
                out,
                "{:width$}  {:>12}  {:>12}  {:>8}  {:>5.0}%  {verdict}",
                row.name,
                time(row.base),
                time(row.head),
                change,
                row.threshold_percent,
            );
        }
        out
    }
}

#[cfg(test)]
mod tests;
