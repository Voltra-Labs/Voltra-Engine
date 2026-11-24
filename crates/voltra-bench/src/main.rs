//! `voltra-bench`: runs the suite, compares two runs, measures the noise.
//!
//! Usually reached through `cargo xtask bench`, which builds it in
//! release. CI runs the binary directly (`.github/workflows/ci.yml`, the
//! `bench` job), since it builds the base's and the head's.

use std::path::PathBuf;
use std::process::ExitCode;

use voltra_bench::{compare, noise, suite, Config, Report, ReportError};

const USAGE: &str = "\
Usage: voltra-bench <COMMAND>

Commands:
  run [--out FILE] [--filter TEXT] [--quick] [--runs N]
        Run the suite (or the benchmarks whose name contains TEXT) and
        write the results as JSON to FILE (default: bench.json). With
        --runs, run it N times and keep each benchmark's best run.
        --quick takes few, short samples: to see it runs, not to compare
  compare --base FILE [--base FILE…] --head FILE [--head FILE…]
          [--threshold P]
        Compare the head's results with the base's and fail (exit 1) if
        a benchmark's median grew past its threshold, or past P percent
        for all of them. Several files on a side are the best of them.
        Exit 2 when the reports cannot be compared (another format, or
        another machine)
  noise [--best-of K] FILE FILE…
        Reports of the same code run several times: each benchmark's
        run-to-run spread, the change CI's check would see comparing the
        code with itself (best of K runs a side, interleaved; default 3),
        and the threshold that suggests
  list
        The benchmarks' names and thresholds";

/// Runs a side CI's `bench` job takes the best of: the A/A comparison
/// `noise` makes by default is the one that job makes.
const CI_BEST_OF: usize = 3;

fn main() -> ExitCode {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();
    let mut args = std::env::args().skip(1);
    let result = match args.next().as_deref() {
        Some("run") => run(args),
        Some("compare") => compare_command(args),
        Some("noise") => noise_command(args),
        Some("list") => {
            for bench in suite::all() {
                println!("{:40} {:>4.0}%", bench.name, bench.threshold_percent);
            }
            Ok(ExitCode::SUCCESS)
        }
        Some("-h" | "--help") | None => {
            println!("{USAGE}");
            Ok(ExitCode::SUCCESS)
        }
        Some(other) => Err(format!("unknown command `{other}`\n\n{USAGE}")),
    };
    match result {
        Ok(code) => code,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(mut args: impl Iterator<Item = String>) -> Result<ExitCode, String> {
    let mut out = PathBuf::from("bench.json");
    let mut filter = None;
    let mut config = Config::DEFAULT;
    let mut runs = 1usize;
    while let Some(arg) = args.next() {
        let mut value = || args.next().ok_or(format!("`{arg}` needs a value"));
        match arg.as_str() {
            "--out" => out = PathBuf::from(value()?),
            "--filter" => filter = Some(value()?),
            "--quick" => config = Config::QUICK,
            "--runs" => {
                runs = value()?
                    .parse()
                    .ok()
                    .filter(|n| *n > 0)
                    .ok_or("`--runs` needs a whole number above zero")?
            }
            other => return Err(format!("unknown option `{other}`\n\n{USAGE}")),
        }
    }
    if cfg!(debug_assertions) {
        log::warn!("an unoptimized build: its numbers will not compare; build with --release");
    }
    let benches = suite::all();
    let mut reports = Vec::with_capacity(runs);
    for round in 1..=runs {
        if runs > 1 {
            println!("run {round} of {runs}");
        }
        let report = suite::run(&benches, &config, filter.as_deref(), |result| {
            match (&result.stats, &result.skipped) {
                (Some(stats), _) => println!(
                    "{:40} {:>12}  ± {:>4.1}%  ({} × {})",
                    result.name,
                    voltra_bench::stats::human(stats.median),
                    stats.relative_mad() * 100.0,
                    stats.samples,
                    stats.iterations,
                ),
                (None, Some(why)) => println!("{:40} skipped: {why}", result.name),
                (None, None) => println!("{:40} skipped", result.name),
            }
        });
        reports.push(report);
    }
    if reports.iter().all(|r| r.results.is_empty()) {
        return Err(format!("no benchmark matches {filter:?}"));
    }
    let report = Report::best_of(&reports).map_err(|e| e.to_string())?;
    report.write(&out).map_err(|e| e.to_string())?;
    println!("wrote {}", out.display());
    Ok(ExitCode::SUCCESS)
}

fn compare_command(mut args: impl Iterator<Item = String>) -> Result<ExitCode, String> {
    let mut base = Vec::new();
    let mut head = Vec::new();
    let mut threshold = None;
    while let Some(arg) = args.next() {
        let mut value = || args.next().ok_or(format!("`{arg}` needs a value"));
        match arg.as_str() {
            "--base" => base.push(PathBuf::from(value()?)),
            "--head" => head.push(PathBuf::from(value()?)),
            "--threshold" => threshold = Some(percent(&value()?)?),
            other => return Err(format!("unknown option `{other}`\n\n{USAGE}")),
        }
    }
    if base.is_empty() || head.is_empty() {
        return Err(format!("compare needs a --base and a --head\n\n{USAGE}"));
    }
    let side = |files: &[PathBuf]| -> Result<Report, ReportError> {
        let reports = files
            .iter()
            .map(|file| Report::read(file))
            .collect::<Result<Vec<_>, _>>()?;
        Report::best_of(&reports)
    };
    let compared = side(&base).and_then(|base| {
        let head = side(&head)?;
        compare(&base, &head, threshold)
    });
    let comparison = match compared {
        Ok(comparison) => comparison,
        Err(e @ (ReportError::Format { .. } | ReportError::Mismatch(_))) => {
            eprintln!("cannot compare: {e}");
            return Ok(ExitCode::from(2));
        }
        Err(e) => return Err(e.to_string()),
    };
    print!("{}", comparison.table());
    let regressions: Vec<&str> = comparison.regressions().map(|r| r.name.as_str()).collect();
    if regressions.is_empty() {
        println!("no regressions");
        Ok(ExitCode::SUCCESS)
    } else {
        println!("regressed: {}", regressions.join(", "));
        Ok(ExitCode::FAILURE)
    }
}

fn noise_command(mut args: impl Iterator<Item = String>) -> Result<ExitCode, String> {
    let mut best_of = CI_BEST_OF;
    let mut files = Vec::new();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--best-of" => {
                best_of = args
                    .next()
                    .and_then(|n| n.parse().ok())
                    .filter(|n| *n > 0)
                    .ok_or("`--best-of` needs a whole number above zero")?
            }
            other => files.push(PathBuf::from(other)),
        }
    }
    if files.len() < 2 {
        return Err(format!("noise needs two reports or more\n\n{USAGE}"));
    }
    let runs = files
        .iter()
        .map(|file| Report::read(file))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    print!("{}", noise::table(&noise::noise(&runs, best_of)));
    Ok(ExitCode::SUCCESS)
}

/// A percentage as typed: `15`, `15%` or `7.5`.
fn percent(text: &str) -> Result<f64, String> {
    text.trim_end_matches('%')
        .parse::<f64>()
        .ok()
        .filter(|p| p.is_finite() && *p >= 0.0)
        .ok_or(format!("`{text}` is not a percentage"))
}
