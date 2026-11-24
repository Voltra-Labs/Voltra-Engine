//! Textures being decoded on the pool for loads in flight, each once.
//!
//! Shared by every load: the regions of a world mostly draw from the same
//! few sheets, and nine regions loading at once must not inflate one PNG
//! nine times. A load waits for every texture it names to leave this list
//! — uploaded by whichever load polled it — so its entities find their
//! textures in the store and nothing decodes on the frame.

use std::path::Path;

use voltra_assets::{AssetPath, DecodedTexture};
use voltra_jobs::{JobPool, Task};

#[derive(Debug, Default)]
pub(super) struct Decodes {
    pending: Vec<(AssetPath, Task<DecodedTexture>)>,
}

impl Decodes {
    /// Starts decoding `path` from `root`, unless it already is.
    pub(super) fn request(&mut self, jobs: &JobPool, root: &Path, path: &AssetPath) {
        if self.is_pending(path) {
            return;
        }
        let root = root.to_path_buf();
        let task_path = path.clone();
        let task = jobs.submit(move || DecodedTexture::decode(&root, task_path));
        self.pending.push((path.clone(), task));
    }

    pub(super) fn is_pending(&self, path: &AssetPath) -> bool {
        self.pending.iter().any(|(pending, _)| pending == path)
    }

    /// `path`'s pixels if they are ready — taken off the list — or `None`.
    /// Where there are no workers this is the decode (see `Task::poll`).
    pub(super) fn poll(&mut self, path: &AssetPath) -> Option<DecodedTexture> {
        let index = self
            .pending
            .iter()
            .position(|(pending, _)| pending == path)?;
        let decoded = self.pending[index].1.poll()?;
        self.pending.swap_remove(index);
        Some(decoded)
    }
}
