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

/// How deep a newly created directory is scanned for files already in it.
///
/// A bound rather than a parameter: it only stops a pathological tree from
/// stalling a frame, and no asset layout comes near it.
const MAX_SCAN_DEPTH: usize = 16;

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
    has_extension(name, TEXTURE_EXTENSIONS)
}

/// File extensions that can become a [`Clip`](voltra_audio::Clip), lowercase,
/// without the dot.
///
/// Exactly the formats the workspace builds `symphonia` with. A name here
/// whose decoder is not compiled in would be watched, listed as a sound, and
/// then fail to decode every time.
pub const CLIP_EXTENSIONS: &[&str] = &["wav", "ogg"];

/// Whether a file name ends in an extension this engine can decode as a clip.
pub fn is_clip(name: &str) -> bool {
    has_extension(name, CLIP_EXTENSIONS)
}

/// File extensions that can become a [`Font`](crate::Font), lowercase,
/// without the dot. What `fontdue` parses: TrueType and OpenType outlines.
pub const FONT_EXTENSIONS: &[&str] = &["ttf", "otf"];

/// Whether a file name ends in an extension this engine can load as a font.
pub fn is_font(name: &str) -> bool {
    has_extension(name, FONT_EXTENSIONS)
}

/// Whether a file name is an input map: `*.input.ron`, the suffix that
/// keeps it out of the scene list the way `*.atlas.ron` keeps a slicing out.
pub fn is_input_map(name: &str) -> bool {
    name.to_ascii_lowercase().ends_with(".input.ron")
}

/// Whether a file name is a project's sorting layers: `*.layers.ron`.
pub fn is_sorting_layers(name: &str) -> bool {
    name.to_ascii_lowercase().ends_with(".layers.ron")
}

/// Whether a file name is a scene, which is also what a prefab is.
///
/// Any `.ron` that is not a slicing or an input map. The atomic save's
/// `demo.ron.tmp` does not match, so a save is seen once, as the rename
/// lands.
pub fn is_scene(name: &str) -> bool {
    has_extension(name, &["ron"])
        && !is_atlas(name)
        && !is_input_map(name)
        && !is_sorting_layers(name)
}

fn has_extension(name: &str, extensions: &[&str]) -> bool {
    let Some((_, extension)) = name.rsplit_once('.') else {
        return false;
    };
    extensions.contains(&extension.to_ascii_lowercase().as_str())
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
    /// Event *kinds* are deliberately not inspected, with one exception below. Which kind a save produces
    /// varies by platform and by the program doing the writing — an overwrite
    /// is `Create` on some, `Modify` on others, a rename pair on a third — and
    /// matching on them is what makes a watcher miss changes
    /// (bevyengine/bevy#10576). Any event on a path that could be an asset is
    /// a reload *attempt*; whether the file is readable is `Textures::reload`'s
    /// question, not this one's.
    ///
    /// The exception: a *created directory* is scanned for files already in
    /// it. inotify watches a new subdirectory only once it has seen it
    /// created, so a folder copied in — or made and filled in one go — has its
    /// files written before the watch exists, and they raise no event of their
    /// own. Windows and macOS watch the tree natively and never hit this; the
    /// scan is harmless there, since every path is deduplicated.
    pub fn drain(&mut self) -> Vec<AssetPath> {
        let mut seen = HashSet::new();
        let mut changed = Vec::new();

        loop {
            match self.events.try_recv() {
                Ok(Ok(events)) => {
                    for event in events {
                        for path in &event.paths {
                            let mut found = Vec::new();
                            // The one place the kind matters: a directory that
                            // was just created may already hold files written
                            // before the watch reached it.
                            if event.kind.is_create() {
                                files_under(path, &mut found);
                            }
                            found.push(path.clone());
                            for path in &found {
                                if let Some(asset) = self.to_asset_path(path) {
                                    if seen.insert(asset.clone()) {
                                        changed.push(asset);
                                    }
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
        if !is_texture(name)
            && !is_atlas(name)
            && !is_clip(name)
            && !is_scene(name)
            && !is_font(name)
            && !is_input_map(name)
            && !is_sorting_layers(name)
        {
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

/// Every file below `dir`, if it is a directory, appended to `found`.
///
/// Symbolic links are not followed, so a link back up the tree cannot loop;
/// [`MAX_SCAN_DEPTH`] bounds the rest.
fn files_under(dir: &Path, found: &mut Vec<PathBuf>) {
    fn walk(dir: &Path, depth: usize, found: &mut Vec<PathBuf>) {
        if depth > MAX_SCAN_DEPTH {
            return;
        }
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_dir() {
                walk(&entry.path(), depth + 1, found);
            } else if kind.is_file() {
                found.push(entry.path());
            }
        }
    }
    // `symlink_metadata`, so a created link to a directory is not walked.
    if std::fs::symlink_metadata(dir).is_ok_and(|meta| meta.is_dir()) {
        walk(dir, 0, found);
    }
}
