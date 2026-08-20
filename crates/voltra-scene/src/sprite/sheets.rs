//! The loaded stores a sprite's geometry is resolved against.

use voltra_assets::{Atlas, Atlases, Handle, TextureSizes};
use voltra_render::glam::UVec2;
use voltra_render::Texture;

/// Where [`quad`](super::quad::quad) looks up the frame a sprite names and the
/// size of the sheet it is cut from.
///
/// One parameter rather than two, because the batch, the pick and the editor
/// all need exactly this pair to place a single sprite, and a click that used
/// one of them while the draw used both is the bug this whole module exists to
/// prevent.
///
/// Both stores are optional, and absent is not an error: a sprite with no
/// atlas and no pixels-per-unit is fully described without either, which is
/// every sprite written before sheets existed and every test that asserts on
/// their geometry headless.
#[derive(Clone, Copy, Default)]
pub struct Sheets<'a> {
    pub atlases: Option<&'a Atlases>,
    pub textures: Option<&'a dyn TextureSizes>,
}

impl<'a> Sheets<'a> {
    /// Both stores, as the editor and the draw path have them.
    pub fn new(atlases: &'a Atlases, textures: &'a dyn TextureSizes) -> Self {
        Self {
            atlases: Some(atlases),
            textures: Some(textures),
        }
    }

    /// The slicing `handle` names, if there is a store holding it.
    pub fn atlas(&self, handle: Option<Handle<Atlas>>) -> Option<&'a Atlas> {
        self.atlases?.try_get(handle?)
    }

    /// The size of `texture` in texels, if there is a store holding it.
    pub fn size(&self, texture: Option<Handle<Texture>>) -> Option<UVec2> {
        self.textures?.size(texture?)
    }

    /// The handle a sprite draws with when the frame it names is not there.
    pub fn placeholder(&self) -> Option<Handle<Texture>> {
        self.textures.map(|textures| textures.placeholder())
    }
}
