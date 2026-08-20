//! Turning filesystem events into the [`AssetPath`]s that changed.
//!
//! One concept, and deliberately not two: this knows nothing about textures,
//! handles or the GPU. [`Textures::reload`](crate::textures::Textures::reload)
//! consumes what this produces, and the caller — `voltra_core::App` — is what
//! joins them. That seam is where a second kind of asset would attach.
//!
//! There is no subscriber registry, because there is one consumer and the shape
//! of the second is not known.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, TryRecvError};
use std::time::Duration;

use notify_debouncer_full::notify::{RecommendedWatcher, RecursiveMode};
use notify_debouncer_full::{new_debouncer, DebounceEventResult, Debouncer, RecommendedCache};

use crate::atlas::is_atlas;
use crate::error::AssetError;
use crate::path::AssetPath;

/// How long events are held before being delivered.
///
/// An image editor writing a PNG emits a burst — truncate, several writes,
/// close — and reloading on each one would upload a half-written file several
/// times per save. 200 ms merges the burst and is still imperceptible.
///
/// Not a parameter: neither Bevy nor Unreal exposes one, and there is no second
/// caller here to disagree with the value.
const DEBOUNCE: Duration = Duration::from_millis(200);

/// File extensions that can become a texture, lowercase, without the dot.
///
/// The filter is by extension rather than by event kind, which is Unreal's
/// shape too. It also means the scene format's atomic save — `demo.ron.tmp`,
/// renamed over `demo.ron` — is ignored without a rule about our own temporary
/// files.
pub const TEXTURE_EXTENSIONS: &[&str] = &["png"];

/// Whether a file name ends in an extension this engine can load as a texture.
///
/// Here rather than beside the browser because this is where the list lives,
/// and a second copy of the match would drift from it.
pub fn is_texture(name: &str) -> bool {
    let Some((_, extension)) = name.rsplit_once('.') else {
        return false;
    };
    TEXTURE_EXTENSIONS.contains(&extension.to_ascii_lowercase().as_str())
}

/// Watches the asset root and reports which assets changed.
///
/// Dropping it stops the watch: the debouncer's own thread is joined by its
/// `Drop`.
pub struct AssetWatcher {
    /// Held for its `Drop`. Removing this field stops the watch immediately.
    _debouncer: Debouncer<RecommendedWatcher, RecommendedCache>,
    events: Receiver<DebounceEventResult>,
    /// The canonical form of the watched root.
    ///
    /// Canonical because the events arrive canonical, and on Windows that
    /// means an extended-length `\\?\` prefix that a root built from
    /// `CARGO_MANIFEST_DIR` does not have. Comparing the two forms is what
    /// crashed Bevy's watcher on Windows for a release (bevyengine/bevy#18342).
    root: PathBuf,
}

impl AssetWatcher {
    /// Starts a recursive watch on `root`.
    ///
    /// Recursive because an artist creates directories: a watch per known
    /// subdirectory would miss the folder of sprites added after startup.
    pub fn new(root: &Path) -> Result<Self, AssetError> {
        let canonical = root.canonicalize().map_err(|source| AssetError::Read {
            path: root.to_path_buf(),
            source,
        })?;

        let (tx, events) = channel();
        let mut debouncer =
            new_debouncer(DEBOUNCE, None, tx).map_err(|source| AssetError::Watch {
                root: canonical.clone(),
                source,
            })?;

        debouncer
            .watch(&canonical, RecursiveMode::Recursive)
            .map_err(|source| AssetError::Watch {
                root: canonical.clone(),
                source,
            })?;

        log::info!("watching {} for asset changes", canonical.display());

        Ok(Self {
            _debouncer: debouncer,
            events,
            root: canonical,
        })
    }

    /// Every asset path that changed since the last call, deduplicated.
    ///
    /// Never blocks: this runs once per frame, and a frame that waits on a
    /// filesystem notification is a dropped frame.
    ///
    /// Event *kinds* are deliberately not inspected. Which kind a save produces
    /// varies by platform and by the program doing the writing — an overwrite
    /// is `Create` on some, `Modify` on others, a rename pair on a third — and
    /// matching on them is what makes a watcher miss changes
    /// (bevyengine/bevy#10576). Any event on a path that could be an asset is
    /// a reload *attempt*; whether the file is readable is `Textures::reload`'s
    /// question, not this one's.
    pub fn drain(&mut self) -> Vec<AssetPath> {
        let mut seen = HashSet::new();
        let mut changed = Vec::new();

        loop {
            match self.events.try_recv() {
                Ok(Ok(events)) => {
                    for event in events {
                        for path in &event.paths {
                            if let Some(asset) = self.to_asset_path(path) {
                                if seen.insert(asset.clone()) {
                                    changed.push(asset);
                                }
                            }
                        }
                    }
                }
                Ok(Err(errors)) => {
                    for error in errors {
                        log::warn!("asset watcher: {error}");
                    }
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    // The debouncer thread is gone, so no further event will
                    // ever arrive. Worth one line, and nothing else: the editor
                    // keeps working, it just stops noticing file changes.
                    log::warn!("asset watcher stopped; file changes will no longer be noticed");
                    break;
                }
            }
        }

        changed
    }

    /// An absolute path from an event, relative to the root, if it is an asset.
    ///
    /// Returns `None` for anything that cannot be relativized rather than
    /// panicking. A path that will not relativize is an ordinary event, not a
    /// bug: a file deleted between the notification and this call, a junction,
    /// a path from a volume we do not own.
    fn to_asset_path(&self, path: &Path) -> Option<AssetPath> {
        let name = path.file_name()?.to_str()?;
        if !is_texture(name) && !is_atlas(name) {
            return None;
        }

        // Canonicalizing fails for a path that has just been deleted, so fall
        // back to the path as delivered: the prefix may still strip, and a
        // delete is exactly the case `Textures::reload` handles by keeping the
        // pixels it already has.
        let absolute = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());

        let relative = absolute.strip_prefix(&self.root).ok().or_else(|| {
            log::debug!(
                "ignoring {} — not under {}",
                absolute.display(),
                self.root.display()
            );
            None
        })?;

        AssetPath::new(&relative.to_string_lossy()).ok()
    }
}
