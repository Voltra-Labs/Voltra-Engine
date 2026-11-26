//! What an `.atlas.ron` file holds, and why one is refused.

use serde::{Deserialize, Serialize};
use voltra_render::glam::{UVec2, Vec2};

use super::{Atlas, Frame, FrameMeta, Grid, Trim};
use crate::path::AssetPath;

/// What an `.atlas.ron` file holds.
///
/// Split from [`Atlas`] the way `voltra-scene`'s `SceneFile` is split from a
/// world: the file carries a version and may describe its frames as a grid,
/// while the runtime type carries the frames themselves. A reader that had to
/// understand both forms would spread the grid arithmetic over every caller.
///
/// **Version 2** (stage 95) adds a name, a trim and a pivot to each explicit
/// frame, all optional; a version 1 file reads unchanged, since it simply
/// has none of them. The packer and the atlas editor write version 2.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grid: Option<Grid>,
    /// Explicit rectangles, appended after the grid's cells.
    #[serde(default)]
    pub frames: Vec<FrameEntry>,
}

/// One explicit frame as a file writes it: the rectangle, then what it
/// carries besides — each left out when it has none, so a plain frame is
/// written as it was in version 1.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct FrameEntry {
    /// Left edge, in texels from the sheet's left.
    pub x: u32,
    /// Top edge, in texels from the sheet's top.
    pub y: u32,
    /// Width, in texels.
    pub width: u32,
    /// Height, in texels.
    pub height: u32,
    /// See [`FrameMeta::name`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// See [`FrameMeta::trim`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trim: Option<Trim>,
    /// See [`FrameMeta::pivot`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pivot: Option<Vec2>,
}

impl FrameEntry {
    /// The rectangle.
    pub fn frame(&self) -> Frame {
        Frame::new(self.x, self.y, self.width, self.height)
    }

    /// The entry for `frame` described by `meta`.
    pub fn new(frame: Frame, meta: &FrameMeta) -> Self {
        Self {
            x: frame.x,
            y: frame.y,
            width: frame.width,
            height: frame.height,
            name: meta.name.clone(),
            trim: meta.trim,
            pivot: meta.pivot,
        }
    }

    fn meta(&self) -> FrameMeta {
        FrameMeta {
            name: self.name.clone(),
            // A trim the frame does not fit in places it nowhere sensible;
            // dropped with a word, so the frame still draws, untrimmed.
            trim: self.trim.filter(|trim| {
                let fits = trim.holds(self.frame().size());
                if !fits {
                    log::warn!(
                        "atlas frame {:?} at ({}, {}) has a trim it does not fit in; drawn untrimmed",
                        self.name.as_deref().unwrap_or(""),
                        self.x,
                        self.y
                    );
                }
                fits
            }),
            pivot: self.pivot,
        }
    }
}

impl From<Frame> for FrameEntry {
    fn from(frame: Frame) -> Self {
        Self::new(frame, &FrameMeta::default())
    }
}

impl AtlasFile {
    /// The version this engine writes. It reads this and every earlier one.
    pub const VERSION: u32 = 2;

    /// The file for `atlas`, every frame written out explicitly with what
    /// it carries: what the packer and the atlas editor save.
    pub fn from_atlas(atlas: &Atlas) -> Self {
        Self {
            version: Self::VERSION,
            sheet: Some(atlas.sheet()),
            texture: atlas.texture().map(|path| path.as_str().to_owned()),
            grid: None,
            frames: atlas
                .frames
                .iter()
                .zip(&atlas.meta)
                .map(|(frame, meta)| FrameEntry::new(*frame, meta))
                .collect(),
        }
    }

    /// The file as RON, laid out one frame to a line, for a pull request to
    /// read and a diff to show the frame that changed.
    pub fn to_ron(&self) -> Result<String, ron::Error> {
        let config = crate::pretty_ron()
            .depth_limit(2)
            .struct_names(false)
            .compact_arrays(false);
        ron::ser::to_string_pretty(self, config).map(|text| {
            text + "
"
        })
    }

    /// The runtime slicing, or why the file cannot become one.
    pub fn into_atlas(self) -> Result<Atlas, AtlasError> {
        if self.version == 0 || self.version > Self::VERSION {
            return Err(AtlasError::Version(self.version));
        }

        let mut frames = match self.grid {
            Some(grid) => grid.frames(),
            None => Vec::new(),
        };
        let mut meta = vec![FrameMeta::default(); frames.len()];
        for entry in &self.frames {
            frames.push(entry.frame());
            meta.push(entry.meta());
        }
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
            meta,
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
                "atlas version {found}, but this engine reads versions 1 to {}",
                AtlasFile::VERSION
            ),
            Self::Empty => write!(f, "the atlas has no frames"),
            Self::NoSheet => write!(f, "the atlas has no sheet size and no grid to derive one"),
            Self::Texture(source) => write!(f, "the atlas names a texture it cannot use: {source}"),
        }
    }
}

impl std::error::Error for AtlasError {}
