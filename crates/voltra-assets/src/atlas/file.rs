//! What an `.atlas.ron` file holds, and why one is refused.

use serde::{Deserialize, Serialize};
use voltra_render::glam::UVec2;

use super::{Atlas, Frame, Grid};
use crate::path::AssetPath;

/// What an `.atlas.ron` file holds.
///
/// Split from [`Atlas`] the way `voltra-scene`'s `SceneFile` is split from a
/// world: the file carries a version and may describe its frames as a grid,
/// while the runtime type carries the frames themselves. A reader that had to
/// understand both forms would spread the grid arithmetic over every caller.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AtlasFile {
    /// The file format version.
    pub version: u32,
    /// The sheet this slicing is cut against, in texels. Read from the grid
    /// when there is one, so a grid file does not have to repeat itself.
    #[serde(default)]
    pub sheet: Option<UVec2>,
    /// The sheet this was cut from, for the editor to assign alongside.
    #[serde(default)]
    pub texture: Option<String>,
    /// A uniform slicing. Cut before `frames`, which are appended after it, so
    /// a sheet that is a grid plus two odd rectangles is one file.
    #[serde(default)]
    pub grid: Option<Grid>,
    /// Explicit rectangles, appended after the grid's cells.
    #[serde(default)]
    pub frames: Vec<Frame>,
}

impl AtlasFile {
    /// The version this engine writes and reads.
    pub const VERSION: u32 = 1;

    /// The runtime slicing, or why the file cannot become one.
    pub fn into_atlas(self) -> Result<Atlas, AtlasError> {
        if self.version != Self::VERSION {
            return Err(AtlasError::Version(self.version));
        }

        let mut frames = match self.grid {
            Some(grid) => grid.frames(),
            None => Vec::new(),
        };
        frames.extend(self.frames);
        if frames.is_empty() {
            return Err(AtlasError::Empty);
        }

        // The grid knows the sheet it needs; an explicit `sheet` overrides it,
        // because a grid cut out of the corner of a bigger sheet is ordinary.
        let sheet = match (self.sheet, self.grid) {
            (Some(sheet), _) => sheet,
            (None, Some(grid)) => grid.sheet(),
            (None, None) => return Err(AtlasError::NoSheet),
        };
        if sheet.x == 0 || sheet.y == 0 {
            return Err(AtlasError::NoSheet);
        }

        let texture = match self.texture {
            Some(raw) => Some(AssetPath::new(&raw).map_err(|e| AtlasError::Texture(Box::new(e)))?),
            None => None,
        };

        Ok(Atlas {
            sheet,
            frames,
            texture,
        })
    }
}

/// Why a file is not a slicing.
#[derive(Debug)]
pub enum AtlasError {
    /// Written by a different version of the engine.
    Version(u32),
    /// No grid and no frames: nothing to draw.
    Empty,
    /// No sheet size, and no grid to work one out from. Without it every UV
    /// would be a guess.
    NoSheet,
    /// The texture hint is not a usable asset path.
    ///
    /// Boxed because [`AssetError`](crate::error::AssetError) carries this
    /// type in turn — a file that will not parse is an asset error, and a path
    /// that will not validate is an atlas error — and two enums holding each
    /// other by value have no size.
    Texture(Box<crate::error::AssetError>),
}

impl std::fmt::Display for AtlasError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Version(found) => write!(
                f,
                "atlas version {found}, but this engine reads version {}",
                AtlasFile::VERSION
            ),
            Self::Empty => write!(f, "the atlas has no frames"),
            Self::NoSheet => write!(f, "the atlas has no sheet size and no grid to derive one"),
            Self::Texture(source) => write!(f, "the atlas names a texture it cannot use: {source}"),
        }
    }
}

impl std::error::Error for AtlasError {}
