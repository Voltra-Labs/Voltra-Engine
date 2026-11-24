//! Building, testing and packaging the project: cargo, with arguments the
//! tool chooses — never a command line the client wrote.

use std::path::Path;
use std::process::Command;
use std::time::Instant;

use serde_json::json;

use super::{ToolError, ToolResult};
use crate::Project;

/// The most output lines a cargo run hands back; the last are kept, which
/// is where errors and the summary are.
const MAX_LINES: usize = 150;

/// What to ask cargo for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CargoJob {
    /// `cargo build`, of one package or the workspace.
    Build {
        /// The package to build; absent builds the workspace.
        package: Option<String>,
    },
    /// `cargo test`, of one package or the workspace, filtered by name.
    Test {
        /// The package to test; absent tests the workspace.
        package: Option<String>,
        /// Only the tests whose name contains this.
        filter: Option<String>,
    },
    /// `cargo xtask dist`, with the build manifest named or `build.ron`.
    Dist {
        /// The manifest to package; absent uses `build.ron`.
        manifest: Option<String>,
    },
}

/// Whether `name` is a cargo package name: letters, digits, `-` and `_`,
/// and not an option.
fn package_name(name: &str) -> Result<&str, ToolError> {
    let ok = !name.is_empty()
        && !name.starts_with('-')
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if ok {
        Ok(name)
    } else {
        Err(ToolError(format!("{name:?} is not a package name")))
    }
}

/// The arguments cargo is run with for `job` in `project`: built here, from
/// checked pieces, so nothing a client sends becomes an option or a shell
/// word.
pub fn cargo_args(project: &Project, job: &CargoJob) -> Result<Vec<String>, ToolError> {
    let mut args = Vec::new();
    match job {
        CargoJob::Build { package } => {
            args.push("build".to_owned());
            match package {
                Some(name) => args.extend(["-p".to_owned(), package_name(name)?.to_owned()]),
                None => args.push("--workspace".to_owned()),
            }
        }
        CargoJob::Test { package, filter } => {
            args.push("test".to_owned());
            match package {
                Some(name) => args.extend(["-p".to_owned(), package_name(name)?.to_owned()]),
                None => args.push("--workspace".to_owned()),
            }
            if let Some(filter) = filter.as_deref().filter(|f| !f.is_empty()) {
                if filter.starts_with('-') {
                    return Err(ToolError(format!("{filter:?} is not a test name filter")));
                }
                args.extend(["--".to_owned(), filter.to_owned()]);
            }
        }
        CargoJob::Dist { manifest } => {
            args.extend(["xtask".to_owned(), "dist".to_owned()]);
            if let Some(manifest) = manifest {
                let relative = Path::new(manifest);
                let inside = relative.is_relative()
                    && !relative
                        .components()
                        .any(|c| matches!(c, std::path::Component::ParentDir));
                if !inside || !project.root().join(relative).is_file() {
                    return Err(ToolError(format!(
                        "{manifest} is not a manifest file in the project"
                    )));
                }
                args.extend(["--manifest".to_owned(), manifest.clone()]);
            }
        }
    }
    Ok(args)
}

/// Runs cargo for `job` in the project's root and answers whether it
/// succeeded, how long it took, and the end of what it printed. Blocking:
/// call it off the async runtime.
pub fn run_cargo(project: &Project, job: &CargoJob) -> ToolResult {
    let args = cargo_args(project, job)?;
    let started = Instant::now();
    let output = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
        .args(&args)
        .current_dir(project.root())
        .output()
        .map_err(|e| ToolError(format!("could not run cargo: {e}")))?;
    let mut lines: Vec<String> = String::from_utf8_lossy(&output.stderr)
        .lines()
        .chain(String::from_utf8_lossy(&output.stdout).lines())
        .map(str::to_owned)
        .collect();
    if lines.len() > MAX_LINES {
        lines.drain(..lines.len() - MAX_LINES);
    }
    Ok(json!({
        "command": format!("cargo {}", args.join(" ")),
        "success": output.status.success(),
        "exit_code": output.status.code(),
        "seconds": started.elapsed().as_secs_f32(),
        "output": lines,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arguments_are_built_from_checked_pieces() {
        let project = Project::new(std::env::temp_dir(), None);
        let build = cargo_args(
            &project,
            &CargoJob::Build {
                package: Some("voltra-scene".into()),
            },
        )
        .expect("fine");
        assert_eq!(build, ["build", "-p", "voltra-scene"]);
        let all = cargo_args(
            &project,
            &CargoJob::Test {
                package: None,
                filter: Some("headless".into()),
            },
        )
        .expect("fine");
        assert_eq!(all, ["test", "--workspace", "--", "headless"]);
        for bad in ["--config=x", "a b", "", "x;rm"] {
            assert!(
                cargo_args(
                    &project,
                    &CargoJob::Build {
                        package: Some(bad.into())
                    }
                )
                .is_err(),
                "{bad:?}"
            );
        }
        assert!(cargo_args(
            &project,
            &CargoJob::Test {
                package: None,
                filter: Some("--nocapture".into())
            }
        )
        .is_err());
        assert!(cargo_args(
            &project,
            &CargoJob::Dist {
                manifest: Some("../build.ron".into())
            }
        )
        .is_err());
    }
}
