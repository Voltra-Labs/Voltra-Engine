//! Reading a texture's file: the only place textures touch the disk.

use std::path::Path;

use voltra_render::texture::{decode_png, DecodedImage};

use crate::error::AssetError;
use crate::path::AssetPath;

/// Reads and decodes `path` under `root`: the only place textures touch the
/// disk, and needing no device, safe on any thread. A render texture is
/// [`AssetError::MadeNotRead`], unread: every caller — the store, a
/// preload, a decode for later, a silhouette — is told so by type.
pub(crate) fn decode_file(root: &Path, path: &AssetPath) -> Result<DecodedImage, AssetError> {
    // Safe to join because `AssetPath` has already refused anything that
    // could climb out of the root. That check lives in the constructor
    // precisely so it cannot be forgotten here.
    let full = root.join(path.as_str());
    // A render texture's file is its size, not its pixels: never a PNG.
    if super::is_render_texture(path.as_str()) {
        return Err(AssetError::MadeNotRead(full));
    }
    let bytes = crate::fs::read(&full).map_err(|source| AssetError::Read {
        path: full.clone(),
        source,
    })?;
    decode_png(&bytes).map_err(|source| AssetError::Decode { path: full, source })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_render_texture_is_made_not_read_and_its_sprites_trace_nothing() {
        let root = voltra_testkit::scratch_root();
        std::fs::write(
            root.join("map.rendertexture.ron"),
            "(version: 1, width: 8, height: 8)",
        )
        .expect("writes");
        let path = AssetPath::new("map.rendertexture.ron").expect("a path");
        assert!(matches!(
            decode_file(&root, &path),
            Err(AssetError::MadeNotRead(_))
        ));
        let decoded = crate::DecodedTexture::decode(&root, path.clone());
        assert_eq!(decoded.path(), &path);
        let mut silhouettes = crate::Silhouettes::new(&*root);
        let rect = [0, 0, 8, 8];
        silhouettes.ensure(&path, rect, 128);
        assert_eq!(silhouettes.get(&path, rect, 128).map(<[_]>::len), Some(0));
    }
}
