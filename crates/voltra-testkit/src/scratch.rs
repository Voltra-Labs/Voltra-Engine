//! Test scratch folders that do not outlive their test.
//!
//! Tests write projects, scenes, fonts and built games under the system
//! temp dir; left behind, a workspace's worth of runs fills a disk. So:
//!
//! - **A [`Scratch`] removes its folder when dropped** — at the end of the
//!   test that holds it, passed or failed. A failed test (one panicking
//!   while it holds the folder) keeps it instead when [`KEEP_SCRATCH_VAR`]
//!   is set, and says where on stderr.
//! - **What a guard cannot cover is swept**: every folder lives under
//!   `voltra-scratch/<process>/` in the temp dir, and each test process
//!   holds a lock on its own `<process>/.lock` until it exits. The first
//!   scratch folder a process makes removes the other processes' folders
//!   whose lock it can take — their process has ended, killed mid-test, or
//!   a helper handed a folder on with [`Scratch::keep`]. With
//!   [`KEEP_SCRATCH_VAR`] set nothing is swept.

use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;

/// The environment variable that keeps scratch folders (any value): a
/// failed test's is not removed when the test ends, and no earlier run's
/// is swept. Set it to look at what a failing test left behind.
pub const KEEP_SCRATCH_VAR: &str = "VOLTRA_KEEP_SCRATCH";

/// A test's own folder under the system temp dir, removed when dropped.
///
/// Dereferences to the [`Path`], so `root.join(..)` and `&root` read as
/// they would on a `PathBuf`. Hold it for as long as the folder is used:
/// `Store::new(&scratch_root())` removes the folder at the end of that
/// statement. A helper whose folder must outlive it (it hands back a store
/// reading from the folder) answers the [`Scratch`] beside the store, or
/// lets go of it with [`keep`](Self::keep).
#[derive(Debug)]
pub struct Scratch {
    path: PathBuf,
}

/// The folder this process's scratch folders go in, locked for its life.
struct ProcessDir {
    path: PathBuf,
    /// Held, never read: the lock lasts as long as the file is open, and a
    /// static is never dropped — the system lets go when the process ends.
    _lock: Option<File>,
}

static PROCESS: OnceLock<ProcessDir> = OnceLock::new();
static COUNT: AtomicU64 = AtomicU64::new(0);

fn keeping() -> bool {
    std::env::var_os(KEEP_SCRATCH_VAR).is_some()
}

fn parent() -> PathBuf {
    std::env::temp_dir().join("voltra-scratch")
}

fn process_dir() -> &'static Path {
    &PROCESS
        .get_or_init(|| {
            let parent = parent();
            if !keeping() {
                sweep(&parent);
            }
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("the clock is after 1970")
                .as_nanos();
            let path = parent.join(format!("{}-{nanos}", std::process::id()));
            std::fs::create_dir_all(&path).expect("the process's scratch dir");
            // A lock that will not take is no reason to fail a test: the
            // folder is then only removed by its guards, as before.
            let lock = File::create(path.join(".lock"))
                .ok()
                .filter(|file| file.try_lock().is_ok());
            ProcessDir { path, _lock: lock }
        })
        .path
}

/// Removes the process folders under `parent` no live process holds.
fn sweep(parent: &Path) {
    let Ok(entries) = std::fs::read_dir(parent) else {
        return;
    };
    for entry in entries.flatten() {
        let dir = entry.path();
        let free = match File::options().write(true).open(dir.join(".lock")) {
            Ok(file) => file.try_lock().is_ok(),
            // No lock file: a process that could not make one, or a folder
            // half removed. Old enough, it is no one's.
            Err(_) => entry
                .metadata()
                .and_then(|m| m.modified())
                .ok()
                .and_then(|at| at.elapsed().ok())
                .is_some_and(|age| age.as_secs() > 24 * 60 * 60),
        };
        if free {
            let _ = std::fs::remove_dir_all(&dir);
        }
    }
}

impl Scratch {
    /// A fresh folder named `<prefix>-<n>`, unique per call so tests running
    /// in parallel cannot see each other's files.
    pub fn new(prefix: &str) -> Self {
        let count = COUNT.fetch_add(1, Ordering::Relaxed);
        let path = process_dir().join(format!("{prefix}-{count}"));
        std::fs::create_dir_all(&path).expect("scratch dir");
        Self { path }
    }

    /// The folder.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Lets go of the guard, keeping the folder until this process has
    /// ended and a later one sweeps it: for a helper that hands the path to
    /// something outliving it.
    pub fn keep(self) -> PathBuf {
        let path = self.path.clone();
        std::mem::forget(self);
        path
    }
}

impl std::ops::Deref for Scratch {
    type Target = Path;

    fn deref(&self) -> &Path {
        &self.path
    }
}

impl AsRef<Path> for Scratch {
    fn as_ref(&self) -> &Path {
        &self.path
    }
}

impl AsRef<std::ffi::OsStr> for Scratch {
    fn as_ref(&self) -> &std::ffi::OsStr {
        self.path.as_os_str()
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        if std::thread::panicking() && keeping() {
            eprintln!(
                "kept the failed test's scratch folder {}",
                self.path.display()
            );
            return;
        }
        // Best effort: a file another thread still has open (Windows) is
        // left for a later process's sweep.
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// A fresh [`Scratch`] folder: removed when the test is done with it.
pub fn scratch_root() -> Scratch {
    Scratch::new("voltra")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_scratch_folder_goes_with_its_guard_and_a_kept_one_stays() {
        let scratch = scratch_root();
        std::fs::write(scratch.join("file"), "x").expect("writes");
        let path = scratch.to_path_buf();
        assert!(path.is_dir());
        drop(scratch);
        assert!(!path.exists(), "removed with the guard");

        let kept = scratch_root().keep();
        assert!(kept.is_dir(), "left for a later sweep");
        let _ = std::fs::remove_dir_all(kept);
    }

    #[test]
    fn a_dead_process_folder_is_swept_and_a_locked_one_is_not() {
        let parent = scratch_root();
        let dead = parent.join("1-1");
        std::fs::create_dir_all(&dead).expect("dir");
        File::create(dead.join(".lock")).expect("lock file");
        let live = parent.join("2-2");
        std::fs::create_dir_all(&live).expect("dir");
        let held = File::create(live.join(".lock")).expect("lock file");
        held.try_lock().expect("locks");

        sweep(&parent);
        assert!(!dead.exists(), "no one holds its lock");
        assert!(live.exists(), "a live process's");
    }
}
