//! Reading a texture's file: the only place textures touch the disk.

use std::path::Path;

use voltra_render::texture::{decode_png, DecodedImage};

use crate::error::AssetError;
use crate::path::AssetPath;

/// Reads and decodes `path` under `root`: the only place textures touch the
/// disk, and needing no device, safe on any thread.
pub(crate) fn decode_file(root: &Path, path: &AssetPath) -> Result<DecodedImage, AssetError> {
    // Safe to join because `AssetPath` has already refused anything that
    // could climb out of the root. That check lives in the constructor
    // precisely so it cannot be forgotten here.
    let full = root.join(path.as_str());
    let bytes = crate::fs::read(&full).map_err(|source| AssetError::Read {
        path: full.clone(),
        source,
    })?;
    decode_png(&bytes).map_err(|source| AssetError::Decode { path: full, source })
}
