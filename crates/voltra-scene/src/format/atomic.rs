//! Replacing a file's contents with no window in which the file is truncated.
//!
//! `std::fs::write` truncates the destination before writing into it; between
//! those two steps the file on disk is shorter than both the old and new
//! contents, and a crash or I/O error in that window loses whatever was there.
//! `replace` writes the new bytes to a fresh file and swaps it in with a
//! rename, so a reader — or a crash — only ever sees one whole version or the
//! other, never a partial one.

use std::fs::File;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use uuid::Uuid;

/// Removes the temporary file on drop unless disarmed.
///
/// Every fallible step between creating the temporary and renaming it over
/// the destination returns through `?`, which unwinds through this guard —
/// that is what keeps a failed write from leaving a `.tmp` orphan next to the
/// scene it was trying to replace.
struct TempFile {
    path: PathBuf,
    armed: bool,
}

impl Drop for TempFile {
    fn drop(&mut self) {
        if self.armed {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

/// The sibling path `replace` writes to before renaming it over `path`.
fn temp_path(path: &Path) -> io::Result<PathBuf> {
    let name = path.file_name().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "{} has no file name to derive a temporary from",
                path.display()
            ),
        )
    })?;
    // Unique per call: two writers of the same destination then never share a
    // temporary, so neither can truncate the other's half-written file.
    let id = Uuid::now_v7().simple();
    // Sibling of the destination, not `std::env::temp_dir()`: `rename` is only
    // atomic within one volume, and a system temp directory can be another
    // one, which would degrade the rename below to a copy and reopen the
    // window this module exists to close.
    Ok(path.with_file_name(format!("{}.{id}.tmp", name.to_string_lossy())))
}

/// Replaces `path`'s contents with `bytes`, or leaves the file untouched.
///
/// `Ok(())` means the file at `path` holds exactly `bytes`. `Err(_)` means it
/// holds whatever it held before this call — unchanged, or still absent if it
/// was absent — and no temporary was left behind.
pub fn replace(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let temp = temp_path(path)?;
    let mut guard = TempFile {
        path: temp,
        armed: true,
    };

    let mut file = File::create(&guard.path)?;
    file.write_all(bytes)?;
    // Flushed before the rename, not after: otherwise the rename can reach
    // disk before the bytes do, and a power loss leaves the destination
    // renamed and empty rather than holding the old or the new contents.
    file.sync_all()?;
    drop(file);

    std::fs::rename(&guard.path, path)?;
    guard.armed = false;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch_dir() -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("voltra-scene-atomic-{}", Uuid::now_v7().simple()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        dir
    }

    #[test]
    fn replace_creates_a_file_that_did_not_exist() {
        let dir = scratch_dir();
        let path = dir.join("scene.ron");

        replace(&path, b"hello").expect("replace cannot fail here");

        assert_eq!(std::fs::read(&path).expect("written"), b"hello");
    }

    #[test]
    fn replace_over_an_existing_file_yields_the_new_contents() {
        let dir = scratch_dir();
        let path = dir.join("scene.ron");
        std::fs::write(&path, b"old").expect("seed file");

        replace(&path, b"new").expect("replace cannot fail here");

        assert_eq!(std::fs::read(&path).expect("written"), b"new");
    }

    #[test]
    fn a_failed_replace_leaves_the_original_intact_and_no_tmp_behind() {
        let dir = scratch_dir();
        let path = dir.join("scene.ron");
        std::fs::write(&path, b"original").expect("seed file");

        // A parent directory that does not exist fails `File::create` inside
        // `replace`, after the temporary path was derived but before anything
        // was written to disk.
        let doomed = dir.join("missing").join("scene.ron");
        let err = replace(&doomed, b"new").expect_err("missing parent must fail");
        assert_eq!(err.kind(), io::ErrorKind::NotFound);

        assert_eq!(
            std::fs::read(&path).expect("original untouched"),
            b"original"
        );

        let leftovers: Vec<_> = std::fs::read_dir(&dir)
            .expect("read dir")
            .map(|entry| entry.expect("dir entry").file_name())
            .filter(|name| name.to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(leftovers.is_empty(), "left behind: {leftovers:?}");
    }

    #[test]
    fn a_successful_replace_leaves_no_tmp_in_the_destination_directory() {
        let dir = scratch_dir();
        let path = dir.join("scene.ron");

        replace(&path, b"hello").expect("replace cannot fail here");

        let entries: Vec<_> = std::fs::read_dir(&dir)
            .expect("read dir")
            .map(|entry| entry.expect("dir entry"))
            .collect();
        assert_eq!(entries.len(), 1, "got {entries:?}");
    }

    #[test]
    fn two_temporaries_for_the_same_destination_differ() {
        let dir = scratch_dir();
        let path = dir.join("scene.ron");

        let a = temp_path(&path).expect("derivable");
        let b = temp_path(&path).expect("derivable");

        assert_ne!(a, b);
    }

    #[test]
    fn a_path_with_no_file_name_is_rejected() {
        // Must fail before creating anything — there is no destination or
        // temporary name to derive from a bare `..`.
        let err = replace(Path::new(".."), b"x").expect_err("no file name must fail");
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
    }

    #[test]
    fn empty_bytes_produce_an_empty_file() {
        let dir = scratch_dir();
        let path = dir.join("scene.ron");

        replace(&path, b"").expect("replace cannot fail here");

        assert_eq!(std::fs::read(&path).expect("written"), b"");
    }
}
