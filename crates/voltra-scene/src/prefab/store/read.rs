//! The one place the store touches the disk.

use voltra_assets::AssetPath;

use super::super::Prefab;
use super::Prefabs;
use crate::format::{SceneError, SceneFile};

impl Prefabs {
    /// Reads and parses one file, refusing one past `limit` bytes (a
    /// prefab's [`MAX_PREFAB_BYTES`]; none for a scene's) or of another
    /// version. The only place this type touches the disk.
    pub(super) fn read(
        &self,
        path: &AssetPath,
        limit: Option<u64>,
    ) -> Result<SceneFile, SceneError> {
        let full = self.root.join(path.as_str());
        let too_large = |limit: u64| {
            SceneError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("larger than {limit} bytes, the most a prefab may be"),
            ))
        };
        if let Some(limit) = limit {
            if !voltra_assets::fs::is_packed(&full) {
                if let Ok(meta) = std::fs::metadata(&full) {
                    if meta.len() > limit {
                        return Err(too_large(limit));
                    }
                }
            }
        }
        let text = voltra_assets::fs::read_to_string(&full)?;
        if let Some(limit) = limit.filter(|&limit| text.len() as u64 > limit) {
            return Err(too_large(limit));
        }
        let file = crate::format::parse(&text)?;
        Ok(Prefab::new(file)?
            .file
            .expect("Prefab::new keeps the file it checked"))
    }
}
