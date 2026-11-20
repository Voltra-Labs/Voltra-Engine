//! The loaded stores a sprite's geometry is resolved against.

use voltra_assets::{Atlas, Atlases, Fonts, Handle, TextureSizes};
use voltra_render::glam::{UVec2, Vec2};
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
    /// The faces and glyph atlases a [`Text`](crate::Text) draws from.
    /// Absent draws and picks no text at all.
    pub fonts: Option<&'a Fonts>,
    /// Where the camera the frame is drawn through sits, which is what a
    /// [`Parallax`](crate::Parallax) layer is displaced by. The origin —
    /// the default — draws every layer where its transform says.
    pub camera: Vec2,
}

impl<'a> Sheets<'a> {
    /// Both stores, as the editor and the draw path have them.
    pub fn new(atlases: &'a Atlases, textures: &'a dyn TextureSizes) -> Self {
        Self {
            atlases: Some(atlases),
            textures: Some(textures),
            fonts: None,
            camera: Vec2::ZERO,
        }
    }

    /// The same stores plus the fonts, as the draw path and the editor have
    /// them once text exists.
    pub fn with_fonts(mut self, fonts: &'a Fonts) -> Self {
        self.fonts = Some(fonts);
        self
    }

    /// The same stores, seen through a camera at `camera`.
    pub fn seen_from(mut self, camera: Vec2) -> Self {
        self.camera = camera;
        self
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
