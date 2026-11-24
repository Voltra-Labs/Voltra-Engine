//! A sheet cut into frames.

use voltra_render::glam::UVec2;

use crate::path::AssetPath;

mod file;
mod frame;
mod grid;
mod meta;

pub use file::{AtlasError, AtlasFile, FrameEntry};
pub use frame::Frame;
pub use grid::Grid;
pub use meta::{FrameMeta, Trim};

/// The suffix that marks a RON file as a slicing: `hero.atlas.ron`.
///
/// A suffix rather than an extension, because the extension is `ron` and the
/// scene format owns that already. Matching the whole tail is what keeps a
/// scene save out of the atlas reload path without a rule about either.
pub const ATLAS_SUFFIX: &str = ".atlas.ron";

/// Whether a file name is an atlas file. Case-insensitive, as the texture
/// extensions are: an artist on Windows types `Coin.Atlas.RON` and means it.
pub fn is_atlas(name: &str) -> bool {
    name.to_ascii_lowercase().ends_with(ATLAS_SUFFIX)
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
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Atlas {
    sheet: UVec2,
    frames: Vec<Frame>,
    /// One per frame, always: the constructors and [`Atlas::push`] keep
    /// the two the same length.
    meta: Vec<FrameMeta>,
    texture: Option<AssetPath>,
}

impl Atlas {
    /// A slicing of a `sheet`-sized image into `frames`.
    pub fn new(sheet: UVec2, frames: Vec<Frame>) -> Self {
        let meta = vec![FrameMeta::default(); frames.len()];
        Self {
            sheet,
            frames,
            meta,
            texture: None,
        }
    }

    /// Adds `frame`, described by `meta`, after the last one. Returns its
    /// index.
    pub fn push(&mut self, frame: Frame, meta: FrameMeta) -> u32 {
        self.frames.push(frame);
        self.meta.push(meta);
        (self.frames.len() - 1) as u32
    }

    /// What frame `index` carries besides its rectangle, or `None` past the
    /// last one.
    pub fn meta(&self, index: u32) -> Option<&FrameMeta> {
        self.meta.get(index as usize)
    }

    /// The name of frame `index`, if it has one.
    pub fn name(&self, index: u32) -> Option<&str> {
        self.meta(index)?.name.as_deref()
    }

    /// The index of the first frame called `name`: how a game or a tool
    /// that knows a frame by name finds the index a sprite draws by.
    pub fn index_of(&self, name: &str) -> Option<u32> {
        self.meta
            .iter()
            .position(|meta| meta.name.as_deref() == Some(name))
            .map(|index| index as u32)
    }

    /// A slicing of the sheet `grid` describes.
    pub fn from_grid(grid: Grid) -> Self {
        Self::new(grid.sheet(), grid.frames())
    }

    /// The slicing with `texture` as the sheet it was cut from.
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

    /// Every frame, in index order.
    pub fn frames(&self) -> &[Frame] {
        &self.frames
    }

    /// How many frames there are.
    pub fn len(&self) -> usize {
        self.frames.len()
    }

    /// Whether there are no frames.
    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }
}

#[cfg(test)]
mod tests;
