//! Noticing that a game crate's sources changed.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver};
use std::time::Duration;

use notify_debouncer_full::notify::{RecommendedWatcher, RecursiveMode};
use notify_debouncer_full::{new_debouncer, DebounceEventResult, Debouncer, RecommendedCache};

/// A save in an editor is a burst of events; a quarter second merges it.
const DEBOUNCE: Duration = Duration::from_millis(250);

/// Watches a crate's folder for changes to what builds it: its Rust
/// sources, its manifest, its build script.
pub struct SourceWatcher {
    _debouncer: Debouncer<RecommendedWatcher, RecommendedCache>,
    events: Receiver<DebounceEventResult>,
    dir: PathBuf,
}

impl SourceWatcher {
    /// Starts watching the crate at `dir`.
    pub fn new(dir: &Path) -> Result<Self, String> {
        let dir = dir
            .canonicalize()
            .map_err(|e| format!("cannot watch {}: {e}", dir.display()))?;
        let (tx, events) = channel();
        let mut debouncer = new_debouncer(DEBOUNCE, None, tx)
            .map_err(|e| format!("cannot watch {}: {e}", dir.display()))?;
        debouncer
            .watch(&dir, RecursiveMode::Recursive)
            .map_err(|e| format!("cannot watch {}: {e}", dir.display()))?;
        log::info!("watching {} for changes to the game", dir.display());
        Ok(Self {
            _debouncer: debouncer,
            events,
            dir,
        })
    }

    /// Whether anything that builds the crate changed since the last call.
    /// Never waits.
    pub fn changed(&mut self) -> bool {
        let mut changed = false;
        while let Ok(result) = self.events.try_recv() {
            let Ok(events) = result else {
                continue;
            };
            for event in events {
                // A read is no change: see the asset watcher.
                if event.kind.is_access() {
                    continue;
                }
                changed |= event.paths.iter().any(|path| self.builds(path));
            }
        }
        changed
    }

    /// Whether `path` is something cargo builds the crate from: a `.rs`
    /// file or the manifest, and not under its own `target`.
    fn builds(&self, path: &Path) -> bool {
        let Ok(inside) = path.strip_prefix(&self.dir) else {
            return false;
        };
        if inside.components().any(|c| c.as_os_str() == "target") {
            return false;
        }
        let is_rust = path.extension().is_some_and(|ext| ext == "rs");
        let is_manifest = path.file_name().is_some_and(|name| name == "Cargo.toml");
        is_rust || is_manifest
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    fn waits_for_change(watcher: &mut SourceWatcher, within: Duration) -> bool {
        let start = Instant::now();
        while start.elapsed() < within {
            if watcher.changed() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        false
    }

    #[test]
    fn an_edited_source_is_a_change_and_anything_else_is_not() {
        let dir = voltra_testkit::scratch_root();
        std::fs::create_dir_all(dir.join("src")).expect("dir");
        std::fs::write(dir.join("src/lib.rs"), "const SPEED: f32 = 1.0;").expect("write");
        let mut watcher = SourceWatcher::new(&dir).expect("watching");
        waits_for_change(&mut watcher, Duration::from_millis(600));

        std::fs::write(dir.join("notes.txt"), "not code").expect("write");
        std::fs::create_dir_all(dir.join("target")).expect("dir");
        std::fs::write(dir.join("target/out.rs"), "built").expect("write");
        assert!(
            !waits_for_change(&mut watcher, Duration::from_millis(900)),
            "neither notes nor build output build the crate"
        );

        std::fs::write(dir.join("src/lib.rs"), "const SPEED: f32 = 2.0;").expect("write");
        assert!(waits_for_change(&mut watcher, Duration::from_secs(10)));
    }
}
