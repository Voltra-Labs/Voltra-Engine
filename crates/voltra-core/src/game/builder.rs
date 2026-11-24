//! Building a game's library with cargo, off the frame.
//!
//! A build takes seconds to minutes; the editor keeps drawing, and the old
//! code keeps running, until it is done. What it is told is the outcome:
//! built, or the compiler's own account of why not.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

/// How a build ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildOutcome {
    /// It built.
    Built,
    /// It did not build: cargo's output, from which [`summary`] picks the
    /// errors.
    Failed(String),
}

/// A cargo build running in the background.
pub struct GameBuild {
    child: Child,
    /// What cargo writes to stderr, read by its own thread so a chatty build
    /// cannot fill the pipe and stall.
    output: Arc<Mutex<String>>,
    reader: Option<JoinHandle<()>>,
}

/// The cargo to run: the one running this process when there is one
/// (`cargo run` sets `CARGO`), otherwise whichever is on the path.
pub fn cargo() -> PathBuf {
    std::env::var_os("CARGO").map_or_else(|| PathBuf::from("cargo"), PathBuf::from)
}

impl GameBuild {
    /// Starts `cargo <args>` in `dir`.
    pub fn start(dir: &Path, args: &[String]) -> std::io::Result<Self> {
        let mut child = Command::new(cargo())
            .args(args)
            .current_dir(dir)
            .env("CARGO_TERM_COLOR", "never")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()?;
        let output = Arc::new(Mutex::new(String::new()));
        let reader = child.stderr.take().map(|mut stderr| {
            let output = Arc::clone(&output);
            std::thread::spawn(move || {
                let mut bytes = Vec::new();
                let _ = stderr.read_to_end(&mut bytes);
                if let Ok(mut text) = output.lock() {
                    text.push_str(&String::from_utf8_lossy(&bytes));
                }
            })
        });
        Ok(Self {
            child,
            output,
            reader,
        })
    }

    /// The outcome once the build is over; `None` while it runs. Never
    /// waits.
    pub fn poll(&mut self) -> Option<BuildOutcome> {
        let status = match self.child.try_wait() {
            Ok(Some(status)) => status,
            Ok(None) => return None,
            Err(e) => {
                return Some(BuildOutcome::Failed(format!(
                    "could not watch the build: {e}"
                )))
            }
        };
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
        if status.success() {
            return Some(BuildOutcome::Built);
        }
        let output = self
            .output
            .lock()
            .map(|text| text.clone())
            .unwrap_or_default();
        Some(BuildOutcome::Failed(output))
    }
}

impl Drop for GameBuild {
    /// A build no one is waiting for any more is stopped.
    fn drop(&mut self) {
        if matches!(self.child.try_wait(), Ok(None)) {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

/// The compiler's errors out of `output`: each `error` line with the
/// `-->` line that places it, at most `limit` of them — what the editor
/// shows. All of it when there is no such line, cut to the last lines.
pub fn summary(output: &str, limit: usize) -> String {
    let lines: Vec<&str> = output.lines().collect();
    let mut errors = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim_start();
        let is_error = trimmed.starts_with("error[") || trimmed.starts_with("error:");
        if !is_error || trimmed.starts_with("error: could not compile") {
            continue;
        }
        let mut entry = trimmed.to_owned();
        if let Some(place) = lines.get(i + 1).map(|l| l.trim_start()) {
            if place.starts_with("-->") {
                entry.push('\n');
                entry.push_str(place);
            }
        }
        errors.push(entry);
        if errors.len() == limit {
            break;
        }
    }
    if errors.is_empty() {
        let tail: Vec<&str> = lines.iter().rev().take(limit * 2).rev().copied().collect();
        return tail.join("\n");
    }
    errors.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn finish(mut build: GameBuild) -> BuildOutcome {
        let start = Instant::now();
        loop {
            if let Some(outcome) = build.poll() {
                return outcome;
            }
            assert!(
                start.elapsed() < Duration::from_secs(120),
                "the build never ended"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    #[test]
    fn a_command_that_succeeds_is_built() {
        let build =
            GameBuild::start(Path::new("."), &["--version".to_owned()]).expect("cargo runs");
        assert_eq!(finish(build), BuildOutcome::Built);
    }

    #[test]
    fn a_build_that_fails_answers_cargos_own_words() {
        let args = ["build", "--package", "no-such-voltra-game"].map(str::to_owned);
        let build =
            GameBuild::start(Path::new(env!("CARGO_MANIFEST_DIR")), &args).expect("cargo runs");
        let BuildOutcome::Failed(output) = finish(build) else {
            panic!("there is no such package");
        };
        assert!(output.contains("no-such-voltra-game"), "{output}");
        assert!(summary(&output, 3).contains("no-such-voltra-game"));
    }

    #[test]
    fn the_summary_is_the_errors_and_where_they_are() {
        let output = "   Compiling game v0.1.0\nerror[E0308]: mismatched types\n  --> src/lib.rs:4:5\n   |\nwarning: unused\nerror: could not compile `game`\n";
        assert_eq!(
            summary(output, 5),
            "error[E0308]: mismatched types\n--> src/lib.rs:4:5"
        );
    }
}
