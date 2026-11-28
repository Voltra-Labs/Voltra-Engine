//! Finding the game library cargo built for this run, for a game's own
//! tests: what `tests/plugin.rs` loads as the editor would.
//!
//! `cargo build` copies a game's `cdylib` beside its binary — whose path
//! cargo hands integration tests (`CARGO_BIN_EXE_<name>`) — but `cargo
//! test` leaves it where it compiled it: under `deps/`, or under
//! `build/<package>/<hash>/out/` in cargo's newer layout. And there is one
//! such file per *feature set* cargo has built the game with: `cargo test
//! -p my-game` alone and `cargo test -p my-game -p voltra-scene` unify the
//! engine's features differently, so each leaves a library whose engine
//! types have other `TypeId`s. The newest file is not always this run's —
//! a run that found its own library fresh does not touch it — so the
//! newest one *this build's stamp accepts* is.

use std::fmt;
use std::path::{Path, PathBuf};

use super::library::{library_file, GameLibrary, LoadError};

/// Every copy of `package`'s game library cargo may have left beside
/// `binary` (a test's `CARGO_BIN_EXE_<name>`), newest first.
pub fn built_libraries(binary: &Path, package: &str) -> Vec<PathBuf> {
    let Some(profile) = binary.parent() else {
        return Vec::new();
    };
    let file = library_file(package);
    let mut found = vec![profile.join(&file), profile.join("deps").join(&file)];
    if let Ok(units) = std::fs::read_dir(profile.join("build").join(package)) {
        found.extend(
            units
                .flatten()
                .map(|unit| unit.path().join("out").join(&file)),
        );
    }
    let mut dated: Vec<_> = found
        .into_iter()
        .filter_map(|path| {
            let modified = std::fs::metadata(&path).ok()?.modified().ok()?;
            Some((modified, path))
        })
        .collect();
    dated.sort_by_key(|(modified, _)| std::cmp::Reverse(*modified));
    dated.into_iter().map(|(_, path)| path).collect()
}

/// The newest of [`built_libraries`] this build can run, each newer one it
/// refused logged with why.
pub fn open_built(binary: &Path, package: &str) -> Result<GameLibrary, NoBuiltLibrary> {
    let mut refused = Vec::new();
    for path in built_libraries(binary, package) {
        match GameLibrary::open(&path) {
            Ok(library) => return Ok(library),
            Err(error) => {
                log::info!("passing over {}: {error}", path.display());
                refused.push(error);
            }
        }
    }
    Err(NoBuiltLibrary {
        package: package.to_owned(),
        beside: binary.to_owned(),
        refused,
    })
}

/// No library of the game was found that this build can run.
#[derive(Debug)]
pub struct NoBuiltLibrary {
    /// The game's package.
    pub package: String,
    /// The binary it was looked for beside.
    pub beside: PathBuf,
    /// Why each one found was refused, newest first; empty when none was
    /// found at all.
    pub refused: Vec<LoadError>,
}

impl fmt::Display for NoBuiltLibrary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.refused.is_empty() {
            return write!(
                f,
                "cargo built no {} beside {}",
                library_file(&self.package),
                self.beside.display()
            );
        }
        write!(f, "no {} this build can run:", library_file(&self.package))?;
        for error in &self.refused {
            write!(f, "\n  {error}")?;
        }
        Ok(())
    }
}

impl std::error::Error for NoBuiltLibrary {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copies_come_newest_first_and_a_missing_folder_is_none() {
        let root = std::env::temp_dir().join("voltra-built-libraries");
        let _ = std::fs::remove_dir_all(&root);
        let file = library_file("my-game");
        let old = root.join("build/my-game/a/out");
        let new = root.join("build/my-game/b/out");
        std::fs::create_dir_all(&old).expect("a folder");
        std::fs::create_dir_all(&new).expect("a folder");
        std::fs::write(old.join(&file), b"old").expect("written");
        std::thread::sleep(std::time::Duration::from_millis(20));
        std::fs::write(new.join(&file), b"new").expect("written");
        let found = built_libraries(&root.join("my-game"), "my-game");
        assert_eq!(found, [new.join(&file), old.join(&file)]);
        assert!(built_libraries(&root.join("x/my-game"), "my-game").is_empty());
        // Neither is a library: both refused, the message naming each.
        let error = open_built(&root.join("my-game"), "my-game")
            .err()
            .expect("nothing loads");
        assert_eq!(error.refused.len(), 2);
        assert_eq!(error.to_string().lines().count(), 3, "{error}");
        let _ = std::fs::remove_dir_all(&root);
    }
}
