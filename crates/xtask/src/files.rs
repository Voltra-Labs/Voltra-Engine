//! Copying an asset tree, and emptying a build folder without emptying
//! anything else.

use std::io;
use std::path::{Path, PathBuf};

/// Copies every file under `from` to the same place under `to`, creating
/// directories as needed. Returns how many files were copied.
///
/// Which files: [`tree_files`]'s.
pub fn copy_tree(from: &Path, to: &Path) -> io::Result<usize> {
    std::fs::create_dir_all(to)?;
    let files = tree_files(from)?;
    for relative in &files {
        let target = to.join(relative);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(from.join(relative), target)?;
    }
    Ok(files.len())
}

/// Every file under `root` that is an asset, relative to `root`, sorted.
///
/// Hidden files and directories (a leading `.`) and temporaries (`*.tmp`, the
/// atomic save's half-written file) are left out: neither is an asset.
/// Symbolic links are not followed, so a link out of the tree cannot copy a
/// stranger's files into a build.
pub fn tree_files(root: &Path) -> io::Result<Vec<PathBuf>> {
    let mut found = Vec::new();
    walk(root, Path::new(""), &mut found)?;
    found.sort();
    Ok(found)
}

fn walk(root: &Path, relative: &Path, found: &mut Vec<PathBuf>) -> io::Result<()> {
    for entry in std::fs::read_dir(root.join(relative))? {
        let entry = entry?;
        let name = entry.file_name();
        let shown = name.to_string_lossy();
        if shown.starts_with('.') || shown.ends_with(".tmp") {
            continue;
        }
        let kind = entry.file_type()?;
        let path = relative.join(&name);
        if kind.is_dir() {
            walk(root, &path, found)?;
        } else if kind.is_file() {
            found.push(path);
        }
    }
    Ok(())
}

/// Readies `dir` to receive a build: creates it, or empties one that a
/// previous build made.
///
/// A directory is only emptied if it is empty already or holds a build
/// manifest — the mark a previous `dist` left. Anything else is refused: a
/// mistyped `--out` must not delete someone's documents.
pub fn prepare_build_dir(dir: &Path, marker: &str) -> io::Result<()> {
    if !dir.exists() {
        return std::fs::create_dir_all(dir);
    }
    let is_empty = std::fs::read_dir(dir)?.next().is_none();
    if !is_empty && !dir.join(marker).is_file() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!(
                "{} exists and is not a previous build (no {marker} in it); \
                 refusing to empty it",
                dir.display()
            ),
        ));
    }
    std::fs::remove_dir_all(dir)?;
    std::fs::create_dir_all(dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch() -> voltra_testkit::Scratch {
        voltra_testkit::Scratch::new("voltra-xtask")
    }

    #[test]
    fn a_tree_is_copied_with_its_folders() {
        let root = scratch();
        let from = root.join("assets");
        std::fs::create_dir_all(from.join("sprites")).expect("dir");
        std::fs::write(from.join("sprites/a.png"), "a").expect("file");
        std::fs::write(from.join("b.ron"), "b").expect("file");

        let copied = copy_tree(&from, &root.join("out")).expect("copies");

        assert_eq!(copied, 2);
        assert_eq!(
            std::fs::read_to_string(root.join("out/sprites/a.png")).expect("there"),
            "a"
        );
    }

    #[test]
    fn hidden_files_and_temporaries_are_left_behind() {
        let root = scratch();
        let from = root.join("assets");
        std::fs::create_dir_all(from.join(".cache")).expect("dir");
        std::fs::write(from.join(".cache/x"), "x").expect("file");
        std::fs::write(from.join("scene.ron.tmp"), "half").expect("file");
        std::fs::write(from.join("scene.ron"), "whole").expect("file");

        assert_eq!(copy_tree(&from, &root.join("out")).expect("copies"), 1);
        assert!(!root.join("out/.cache").exists());
    }

    #[test]
    fn a_new_build_folder_is_created() {
        let scratch = scratch();
        let dir = scratch.join("dist").join("Game");
        prepare_build_dir(&dir, "build.ron").expect("creates");
        assert!(dir.is_dir());
    }

    #[test]
    fn a_previous_build_is_emptied() {
        let scratch = scratch();
        let dir = scratch.join("Game");
        std::fs::create_dir_all(&dir).expect("dir");
        std::fs::write(dir.join("build.ron"), "old").expect("file");
        std::fs::write(dir.join("stale.png"), "old").expect("file");

        prepare_build_dir(&dir, "build.ron").expect("empties");

        assert!(dir.is_dir());
        assert!(!dir.join("stale.png").exists());
    }

    #[test]
    fn a_folder_that_is_not_a_build_is_never_emptied() {
        let scratch = scratch();
        let dir = scratch.join("Documents");
        std::fs::create_dir_all(&dir).expect("dir");
        std::fs::write(dir.join("thesis.docx"), "years of work").expect("file");

        assert!(prepare_build_dir(&dir, "build.ron").is_err());
        assert!(dir.join("thesis.docx").exists(), "untouched");
    }
}
