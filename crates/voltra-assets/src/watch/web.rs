//! The asset watcher in a browser: there is nothing to watch.
//!
//! A web build's assets are a pack fetched once; nothing changes under it
//! while the page runs. The type exists so the loop that holds one compiles
//! unchanged, and [`AssetWatcher::new`] refuses — which the loop already
//! treats as "hot reload off" — rather than pretending to watch.

use std::io;
use std::path::Path;

use crate::error::AssetError;
use crate::path::AssetPath;

/// Never constructed: [`AssetWatcher::new`] always refuses on the web.
pub struct AssetWatcher {
    _never: (),
}

impl AssetWatcher {
    pub fn new(root: &Path) -> Result<Self, AssetError> {
        Err(AssetError::Read {
            path: root.to_path_buf(),
            source: io::Error::new(io::ErrorKind::Unsupported, "the web has no files to watch"),
        })
    }

    /// Nothing ever changed.
    pub fn drain(&mut self) -> Vec<AssetPath> {
        Vec::new()
    }
}
