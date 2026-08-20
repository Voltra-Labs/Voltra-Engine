//! A sheet cut into frames.

use serde::{Deserialize, Serialize};
use voltra_render::glam::UVec2;

use crate::path::AssetPath;

/// One rectangle of a sheet, in texels.
///
/// Texels rather than UVs because texels are what a sheet is authored in: a
/// slicer, an artist and Aseprite's exporter all speak in pixels, and `0.0625`
/// is nobody's idea of a sixteen-pixel cell. The conversion happens once, at
/// batch time, against [`Atlas::sheet`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Frame {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl Frame {
    pub const fn new(x: u32, y: u32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub const fn size(&self) -> UVec2 {
        UVec2::new(self.width, self.height)
    }

    /// Whether the frame covers any texels at all.
    pub const fn is_empty(&self) -> bool {
        self.width == 0 || self.height == 0
    }

    /// The frame as `(min, max)` in UV space, given the sheet it is cut from.
    ///
    /// `None` for an empty frame or an empty sheet: a zero-area quad is not a
    /// picture of anything, and dividing by a zero sheet is how a `NaN` reaches
    /// the vertex buffer.
    pub fn uv(&self, sheet: UVec2) -> Option<([f32; 2], [f32; 2])> {
        if self.is_empty() || sheet.x == 0 || sheet.y == 0 {
            return None;
        }
        let sheet = sheet.as_vec2();
        let min = [self.x as f32 / sheet.x, self.y as f32 / sheet.y];
        let max = [
            (self.x + self.width) as f32 / sheet.x,
            (self.y + self.height) as f32 / sheet.y,
        ];
        Some((min, max))
    }
}

/// A uniform grid of cells, which is what most sheets are.
///
/// `margin` is the border before the first cell; `padding` is the gap between
/// cells. Both exist because the sheets that are *not* a plain grid are
/// usually a grid with bleed guards, and re-cutting one by hand into explicit
/// rectangles is exactly the work this avoids.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Grid {
    pub cell: UVec2,
    pub columns: u32,
    pub rows: u32,
    #[serde(default)]
    pub margin: UVec2,
    #[serde(default)]
    pub padding: UVec2,
}

impl Grid {
    /// `columns` × `rows` cells of `cell` texels, with no margin or padding.
    pub const fn new(cell: UVec2, columns: u32, rows: u32) -> Self {
        Self {
            cell,
            columns,
            rows,
            margin: UVec2::ZERO,
            padding: UVec2::ZERO,
        }
    }

    pub const fn with_margin(mut self, margin: UVec2) -> Self {
        self.margin = margin;
        self
    }

    pub const fn with_padding(mut self, padding: UVec2) -> Self {
        self.padding = padding;
        self
    }

    /// The cells, left to right and then top to bottom — reading order, which
    /// is the order a sheet is drawn in and therefore the order frame indices
    /// have to follow.
    ///
    /// A grid with no cells, no columns or no rows cuts nothing rather than
    /// producing empty frames nobody can draw.
    pub fn frames(&self) -> Vec<Frame> {
        if self.cell.x == 0 || self.cell.y == 0 {
            return Vec::new();
        }
        let mut frames = Vec::with_capacity((self.columns * self.rows) as usize);
        for row in 0..self.rows {
            for column in 0..self.columns {
                frames.push(Frame::new(
                    self.margin.x + column * (self.cell.x + self.padding.x),
                    self.margin.y + row * (self.cell.y + self.padding.y),
                    self.cell.x,
                    self.cell.y,
                ));
            }
        }
        frames
    }

    /// The sheet a full grid needs, in texels: the margin on both sides, every
    /// cell, and the gaps between them.
    pub fn sheet(&self) -> UVec2 {
        if self.columns == 0 || self.rows == 0 {
            return UVec2::ZERO;
        }
        let cells = UVec2::new(self.columns, self.rows);
        self.margin * 2 + cells * self.cell + (cells - UVec2::ONE) * self.padding
    }
}

/// How a sheet is cut up, and how big the sheet is.
///
/// Deliberately *not* a texture. Bevy's `TextureAtlasLayout` made the same
/// split and it is the right one: a slicing is geometry, so one of them serves
/// every sheet cut the same way — four palette swaps of a character share a
/// file instead of copying it four times. Godot's `AtlasTexture` binds the two
/// together and needs one resource per region as a result.
///
/// [`Atlas::texture`] is a *hint* for authoring — the sheet this slicing was
/// cut from, so the editor can assign both in one drag. Nothing at draw time
/// reads it; the sprite names its own texture, as it always has.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Atlas {
    sheet: UVec2,
    frames: Vec<Frame>,
    texture: Option<AssetPath>,
}

impl Atlas {
    /// A slicing of a `sheet`-sized image into `frames`.
    pub fn new(sheet: UVec2, frames: Vec<Frame>) -> Self {
        Self {
            sheet,
            frames,
            texture: None,
        }
    }

    /// A slicing of the sheet `grid` describes.
    pub fn from_grid(grid: Grid) -> Self {
        Self::new(grid.sheet(), grid.frames())
    }

    pub fn with_texture(mut self, texture: AssetPath) -> Self {
        self.texture = Some(texture);
        self
    }

    /// The size of the image this slicing was cut against, in texels.
    pub fn sheet(&self) -> UVec2 {
        self.sheet
    }

    /// The sheet this slicing was cut from, if the file named one.
    pub fn texture(&self) -> Option<&AssetPath> {
        self.texture.as_ref()
    }

    /// Frame `index`, or `None` past the last one.
    ///
    /// `None` rather than the last frame: an index past the end is an
    /// authoring mistake, and clamping it hides the mistake in the one place
    /// it would have been seen.
    pub fn frame(&self, index: u32) -> Option<Frame> {
        self.frames.get(index as usize).copied()
    }

    pub fn frames(&self) -> &[Frame] {
        &self.frames
    }

    pub fn len(&self) -> usize {
        self.frames.len()
    }

    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }
}

/// What an `.atlas.ron` file holds.
///
/// Split from [`Atlas`] the way `voltra-scene`'s `SceneFile` is split from a
/// world: the file carries a version and may describe its frames as a grid,
/// while the runtime type carries the frames themselves. A reader that had to
/// understand both forms would spread the grid arithmetic over every caller.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AtlasFile {
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
            Some(raw) => Some(AssetPath::new(&raw).map_err(AtlasError::Texture)?),
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
    Texture(crate::error::AssetError),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_grid_cuts_in_reading_order() {
        let grid = Grid::new(UVec2::new(16, 16), 3, 2);

        let frames = grid.frames();

        assert_eq!(frames.len(), 6);
        assert_eq!(frames[0], Frame::new(0, 0, 16, 16));
        assert_eq!(frames[2], Frame::new(32, 0, 16, 16));
        assert_eq!(frames[3], Frame::new(0, 16, 16, 16), "the second row");
        assert_eq!(grid.sheet(), UVec2::new(48, 32));
    }

    #[test]
    fn a_margin_shifts_every_cell_and_a_padding_only_the_gaps() {
        let grid = Grid::new(UVec2::splat(8), 2, 1)
            .with_margin(UVec2::new(2, 3))
            .with_padding(UVec2::new(4, 0));

        let frames = grid.frames();

        assert_eq!(frames[0], Frame::new(2, 3, 8, 8));
        assert_eq!(frames[1], Frame::new(14, 3, 8, 8));
        assert_eq!(grid.sheet(), UVec2::new(2 * 2 + 16 + 4, 3 * 2 + 8));
    }

    #[test]
    fn a_grid_with_no_size_cuts_nothing() {
        assert!(Grid::new(UVec2::ZERO, 4, 4).frames().is_empty());
        assert!(Grid::new(UVec2::splat(8), 0, 4).frames().is_empty());
        assert!(Grid::new(UVec2::splat(8), 4, 0).frames().is_empty());
        assert_eq!(Grid::new(UVec2::splat(8), 0, 4).sheet(), UVec2::ZERO);
    }

    #[test]
    fn a_frame_becomes_the_uvs_it_covers() {
        let frame = Frame::new(16, 0, 16, 32);

        let (min, max) = frame.uv(UVec2::new(64, 32)).expect("it has area");

        assert_eq!(min, [0.25, 0.0]);
        assert_eq!(max, [0.5, 1.0]);
    }

    #[test]
    fn an_empty_frame_or_sheet_has_no_uvs() {
        assert!(Frame::new(0, 0, 0, 8).uv(UVec2::splat(16)).is_none());
        assert!(Frame::new(0, 0, 8, 8).uv(UVec2::ZERO).is_none());
    }

    #[test]
    fn a_frame_past_the_end_is_not_a_frame() {
        let atlas = Atlas::from_grid(Grid::new(UVec2::splat(8), 2, 1));

        assert_eq!(atlas.len(), 2);
        assert_eq!(atlas.frame(1), Some(Frame::new(8, 0, 8, 8)));
        assert_eq!(atlas.frame(2), None, "and not the last one again");
    }

    #[test]
    fn a_file_of_a_grid_needs_no_sheet_of_its_own() {
        let file = AtlasFile {
            version: 1,
            sheet: None,
            texture: None,
            grid: Some(Grid::new(UVec2::splat(16), 4, 1)),
            frames: Vec::new(),
        };

        let atlas = file.into_atlas().expect("a grid is a slicing");

        assert_eq!(atlas.len(), 4);
        assert_eq!(atlas.sheet(), UVec2::new(64, 16));
    }

    #[test]
    fn explicit_frames_are_appended_to_the_grid() {
        let file = AtlasFile {
            version: 1,
            sheet: Some(UVec2::new(64, 32)),
            texture: None,
            grid: Some(Grid::new(UVec2::splat(16), 2, 1)),
            frames: vec![Frame::new(0, 16, 64, 16)],
        };

        let atlas = file.into_atlas().expect("both forms in one file");

        assert_eq!(atlas.len(), 3);
        assert_eq!(atlas.frame(2), Some(Frame::new(0, 16, 64, 16)));
        assert_eq!(atlas.sheet(), UVec2::new(64, 32), "the explicit sheet wins");
    }

    #[test]
    fn a_file_with_nothing_in_it_is_refused() {
        let file = AtlasFile {
            version: 1,
            sheet: Some(UVec2::splat(16)),
            texture: None,
            grid: None,
            frames: Vec::new(),
        };

        assert!(matches!(file.into_atlas(), Err(AtlasError::Empty)));
    }

    #[test]
    fn explicit_frames_without_a_sheet_are_refused() {
        // Every UV would be a guess, and a guess that draws *something* is
        // worse than a refusal that says why.
        let file = AtlasFile {
            version: 1,
            sheet: None,
            texture: None,
            grid: None,
            frames: vec![Frame::new(0, 0, 8, 8)],
        };

        assert!(matches!(file.into_atlas(), Err(AtlasError::NoSheet)));
    }

    #[test]
    fn a_wrong_version_is_refused() {
        let file = AtlasFile {
            version: 99,
            sheet: Some(UVec2::splat(16)),
            texture: None,
            grid: Some(Grid::new(UVec2::splat(16), 1, 1)),
            frames: Vec::new(),
        };

        assert!(matches!(file.into_atlas(), Err(AtlasError::Version(99))));
    }

    #[test]
    fn a_file_round_trips_through_ron() {
        let text = r#"(
            version: 1,
            texture: Some("sprites/coin.png"),
            grid: Some((cell: (16, 16), columns: 4, rows: 1)),
        )"#;

        let file: AtlasFile = ron::from_str(text).expect("the fixture is valid RON");
        let atlas = file.into_atlas().expect("a grid slicing");

        assert_eq!(atlas.len(), 4);
        assert_eq!(
            atlas.texture().map(AssetPath::as_str),
            Some("sprites/coin.png")
        );
        assert_eq!(atlas.frame(3), Some(Frame::new(48, 0, 16, 16)));
    }

    #[test]
    fn an_escaping_texture_hint_is_refused() {
        let file = AtlasFile {
            version: 1,
            sheet: None,
            texture: Some("../secrets.png".into()),
            grid: Some(Grid::new(UVec2::splat(8), 1, 1)),
            frames: Vec::new(),
        };

        assert!(matches!(file.into_atlas(), Err(AtlasError::Texture(_))));
    }
}
