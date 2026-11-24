//! Where a streamed clip's bytes are.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use symphonia::core::io::MediaSourceStream;

use crate::error::AudioError;

/// The bytes a streamed clip is decoded from, each time a voice plays it.
///
/// A file on disk is opened afresh per voice and read a block at a time, so
/// nothing of it is resident beyond `symphonia`'s read buffer. Bytes already
/// in memory — an asset pack, which is the only storage a browser has — are
/// shared by every voice and read through a cursor; they are the compressed
/// file, not its samples, so they are resident either way.
#[derive(Debug, Clone)]
pub enum StreamSource {
    /// A file to open by path.
    File(PathBuf),
    /// A file already in memory, named by `path` in errors and format hints.
    Bytes {
        /// What the file is called.
        path: PathBuf,
        /// Its contents.
        bytes: Arc<[u8]>,
    },
}

impl StreamSource {
    /// The path the bytes are known by.
    pub fn path(&self) -> &Path {
        match self {
            Self::File(path) | Self::Bytes { path, .. } => path,
        }
    }

    /// A fresh reader over the bytes, positioned at the start.
    pub(crate) fn open(&self) -> Result<MediaSourceStream<'static>, AudioError> {
        let source: Box<dyn symphonia::core::io::MediaSource> = match self {
            Self::File(path) => {
                Box::new(
                    std::fs::File::open(path).map_err(|source| AudioError::Read {
                        path: path.clone(),
                        source,
                    })?,
                )
            }
            Self::Bytes { bytes, .. } => Box::new(std::io::Cursor::new(Arc::clone(bytes))),
        };
        Ok(MediaSourceStream::new(source, Default::default()))
    }
}

/// The shape of a streamed clip, read from its header at load.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StreamInfo {
    /// Channels per frame in the file.
    pub channels: u16,
    /// Samples per second per channel in the file.
    pub rate: u32,
    /// How many frames long it is, when the container says.
    pub frames: Option<u64>,
}
