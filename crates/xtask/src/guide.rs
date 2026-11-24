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
        let status = Command::new("bash")
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

#[cfg(test)]
mod tests {
    use super::*;

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
