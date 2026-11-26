//! `cargo xtask guide`: the user guide's commands, run in order, as someone
//! following it from a clean checkout would type them.
//!
//! A shell block in `docs/guide/*.md` preceded by the line `<!-- ci -->` is
//! one the check runs; the rest — launching the editor, playing a game in a
//! window — are for a person at a desk. The files run in name order, and
//! each block's lines in one `bash -e`, so a step that fails stops the
//! check with the guide's own words around it.

use std::path::{Path, PathBuf};
use std::process::Command;

/// The line that marks the next shell block as run by the check.
pub const MARK: &str = "<!-- ci -->";

/// One block the check runs: where it is, and its commands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    pub file: PathBuf,
    pub line: usize,
    pub script: String,
}

/// The marked blocks of `text`, in order.
pub fn steps_in(file: &Path, text: &str) -> Vec<Step> {
    let mut steps = Vec::new();
    let mut armed = false;
    let mut block: Option<(usize, Vec<&str>)> = None;
    for (number, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if let Some((start, lines)) = block.as_mut() {
            if trimmed.starts_with("```") {
                steps.push(Step {
                    file: file.to_path_buf(),
                    line: *start,
                    script: lines.join("\n"),
                });
                block = None;
            } else {
                lines.push(line);
            }
            continue;
        }
        if trimmed == MARK {
            armed = true;
        } else if armed && (trimmed == "```sh" || trimmed == "```bash") {
            block = Some((number + 2, Vec::new()));
            armed = false;
        } else if !trimmed.is_empty() {
            armed = false;
        }
    }
    steps
}

/// Every marked block of the guide under `guide`, its files in name order.
pub fn steps(guide: &Path) -> Result<Vec<Step>, String> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(guide)
        .map_err(|e| format!("cannot read {}: {e}", guide.display()))?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "md"))
        .collect();
    files.sort();
    let mut all = Vec::new();
    for file in files {
        let text = std::fs::read_to_string(&file)
            .map_err(|e| format!("cannot read {}: {e}", file.display()))?;
        all.extend(steps_in(&file, &text));
    }
    Ok(all)
}

/// Runs every marked block from `workspace`, stopping at the first that
/// fails.
pub fn run(workspace: &Path) -> Result<(), String> {
    let steps = steps(&workspace.join("docs/guide"))?;
    if steps.is_empty() {
        return Err("the guide marks no commands to check".into());
    }
    for step in &steps {
        let at = format!("{}:{}", step.file.display(), step.line);
        println!("== {at}\n{}", step.script);
        let status = Command::new(bash()?)
            .args(["-e", "-c", &step.script])
            .current_dir(workspace)
            .status()
            .map_err(|e| format!("cannot run bash for {at}: {e}"))?;
        if !status.success() {
            return Err(format!("the guide's commands at {at} failed ({status})"));
        }
    }
    println!("all {} of the guide's checked blocks ran", steps.len());
    Ok(())
}

/// The bash the blocks run in.
///
/// On Windows `Command::new("bash")` looks in System32 before `PATH` and
/// finds WSL's launcher, whose Linux has none of this machine's tools: the
/// guide failed with `cargo: command not found`. The first `bash.exe` on
/// `PATH` outside Windows' own folders is Git for Windows' — the shell a
/// developer and CI's runner both have.
#[cfg(windows)]
fn bash() -> Result<PathBuf, String> {
    let path = std::env::var_os("PATH").unwrap_or_default();
    let system = std::env::var_os("SystemRoot").map(PathBuf::from);
    pick_bash(
        std::env::split_paths(&path),
        system.as_deref(),
        Path::is_file,
    )
    .ok_or_else(|| {
        "no bash.exe on PATH outside Windows' own folders (WSL's has no cargo): \
         install Git for Windows"
            .to_owned()
    })
}

/// Elsewhere `bash` on `PATH` is the one.
#[cfg(not(windows))]
fn bash() -> Result<PathBuf, String> {
    Ok(PathBuf::from("bash"))
}

/// The first `bash.exe` in `dirs` that is neither under `system` (WSL's
/// `System32\bash.exe`) nor an app-execution alias in `WindowsApps`.
#[cfg_attr(not(windows), allow(dead_code))]
fn pick_bash(
    dirs: impl IntoIterator<Item = PathBuf>,
    system: Option<&Path>,
    is_file: impl Fn(&Path) -> bool,
) -> Option<PathBuf> {
    let lower = |p: &Path| p.to_string_lossy().to_ascii_lowercase();
    let system = system.map(lower);
    dirs.into_iter()
        .filter(|dir| {
            let dir = lower(dir);
            !system.as_ref().is_some_and(|s| dir.starts_with(s.as_str()))
                && !dir.contains("windowsapps")
        })
        .map(|dir| dir.join("bash.exe"))
        .find(|candidate| is_file(candidate))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bash_picked_skips_wsl_s_launchers() {
        let dirs = [
            r"C:\WINDOWS\system32",
            r"C:\Users\me\AppData\Local\Microsoft\WindowsApps",
            r"C:\Tools\no-bash-here",
            r"D:\Git\usr\bin",
            r"E:\later\bin",
        ]
        .map(PathBuf::from);
        let has_bash = |p: &Path| !p.starts_with(r"C:\Tools");
        let picked = pick_bash(dirs, Some(Path::new(r"C:\Windows")), has_bash);
        assert_eq!(
            picked,
            Some(PathBuf::from(r"D:\Git\usr\bin").join("bash.exe"))
        );
    }

    #[test]
    fn no_bash_outside_windows_folders_is_none() {
        let dirs = [PathBuf::from(r"C:\Windows\System32")];
        assert_eq!(
            pick_bash(dirs, Some(Path::new(r"C:\Windows")), |_| true),
            None
        );
    }

    #[test]
    fn only_marked_blocks_are_steps() {
        let text = "# Title\n\n```sh\ncargo run\n```\n\n<!-- ci -->\n```sh\ncargo xtask new-game a\ncargo test -p a\n```\n\n<!-- ci -->\nSome words.\n```sh\nnot run\n```\n";
        let steps = steps_in(Path::new("g.md"), text);
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].script, "cargo xtask new-game a\ncargo test -p a");
        assert_eq!(steps[0].line, 9);
    }

    #[test]
    fn the_shipped_guide_checks_a_game_built_for_the_web() {
        let guide = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/guide");
        let steps = steps(&guide).expect("the guide reads");
        let all: String = steps
            .iter()
            .map(|s| s.script.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(all.contains("cargo xtask new-game"), "{all}");
        assert!(all.contains("dist --web"), "{all}");
    }
}
