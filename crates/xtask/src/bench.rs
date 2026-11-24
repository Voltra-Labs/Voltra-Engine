//! `cargo xtask bench`: the benchmark suite (`crates/voltra-bench`), built
//! in release, run, and compared with an earlier run.
//!
//! A front for the `voltra-bench` binary rather than a copy of it: the
//! suite, the JSON and the comparer are that crate's, and CI runs the
//! binary directly because it builds two of them, the base's and the
//! head's (docs/ARCHITECTURE.md, stage 107).

use std::path::{Path, PathBuf};
use std::process::Command;

/// Parses the arguments after `bench`, runs the suite, and compares.
pub fn run(mut args: impl Iterator<Item = String>, workspace: &Path) -> Result<(), String> {
    let mut save = None;
    let mut bases = Vec::new();
    let mut threshold = None;
    let mut passed: Vec<String> = Vec::new();
    while let Some(arg) = args.next() {
        let mut value = || args.next().ok_or(format!("`{arg}` needs a value"));
        match arg.as_str() {
            "--save" => save = Some(absolute(value()?)?),
            "--compare" => bases.push(absolute(value()?)?),
            "--threshold" => threshold = Some(value()?),
            "--filter" | "--runs" => {
                let value = value()?;
                passed.extend([arg, value]);
            }
            "--quick" => passed.push(arg),
            other => return Err(format!("unknown option `{other}`\n\n{}", crate::USAGE)),
        }
    }
    if threshold.is_some() && bases.is_empty() {
        return Err("`--threshold` is for a `--compare`".to_owned());
    }
    let target = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| workspace.join("target"));
    let save = save.unwrap_or_else(|| target.join("bench").join("latest.json"));

    let mut run = vec!["run".to_owned(), "--out".to_owned(), display(&save)];
    run.extend(passed);
    if !bench(workspace, &run)?.success() {
        return Err("the benchmark run failed".to_owned());
    }
    if bases.is_empty() {
        return Ok(());
    }

    let mut compare = vec!["compare".to_owned()];
    for base in &bases {
        compare.extend(["--base".to_owned(), display(base)]);
    }
    compare.extend(["--head".to_owned(), display(&save)]);
    if let Some(threshold) = threshold {
        compare.extend(["--threshold".to_owned(), threshold]);
    }
    let status = bench(workspace, &compare)?;
    match status.code() {
        Some(0) => Ok(()),
        Some(2) => Err("the runs could not be compared".to_owned()),
        _ => Err("benchmarks regressed".to_owned()),
    }
}

/// `voltra-bench` with `args`, built in release first — a debug build's
/// numbers are the compiler's settings, not the engine.
fn bench(workspace: &Path, args: &[String]) -> Result<std::process::ExitStatus, String> {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    Command::new(cargo)
        .arg("run")
        .arg("--manifest-path")
        .arg(workspace.join("Cargo.toml"))
        .args(["--release", "--quiet", "--package", "voltra-bench", "--"])
        .args(args)
        .status()
        .map_err(|e| format!("could not run cargo: {e}"))
}

/// `path` against the directory `cargo xtask` was typed in: the child runs
/// there too, but saying so leaves nothing to chance.
fn absolute(path: String) -> Result<PathBuf, String> {
    let path = PathBuf::from(path);
    if path.is_absolute() {
        return Ok(path);
    }
    std::env::current_dir()
        .map(|dir| dir.join(path))
        .map_err(|e| format!("no working directory: {e}"))
}

fn display(path: &Path) -> String {
    path.display().to_string()
}
