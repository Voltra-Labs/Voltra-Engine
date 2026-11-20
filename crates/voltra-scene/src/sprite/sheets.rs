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
    /// The world-space rectangle the camera sees, `(min, max)`. Geometry
    /// wholly outside it is left out of the batch. `None` — the default —
    /// keeps everything, which is what a pick, a test and a caller with no
    /// camera want.
    pub visible: Option<(Vec2, Vec2)>,
    /// Seconds on the clock the frame is drawn at, which animated tiles
    /// pick their frame by.
    pub time: f32,
}

impl<'a> Sheets<'a> {
    /// Both stores, as the editor and the draw path have them.
    pub fn new(atlases: &'a Atlases, textures: &'a dyn TextureSizes) -> Self {
        Self {
            atlases: Some(atlases),
            textures: Some(textures),
            fonts: None,
            camera: Vec2::ZERO,
            visible: None,
            time: 0.0,
        }
    }

    /// The same stores plus the fonts, as the draw path and the editor have
    /// them once text exists.
    pub fn with_fonts(mut self, fonts: &'a Fonts) -> Self {
        self.fonts = Some(fonts);
        self
    }

    /// The same stores, drawing only what falls inside `min`..`max`.
    pub fn visible_in(mut self, min: Vec2, max: Vec2) -> Self {
        self.visible = Some((min.min(max), min.max(max)));
        self
    }

    /// Whether a world-space box `min`..`max` can be seen. Edges touching
    /// count as seen; anything non-finite is kept rather than guessed away.
    pub fn sees(&self, min: Vec2, max: Vec2) -> bool {
        let Some((view_min, view_max)) = self.visible else {
            return true;
        };
        if !(min.is_finite() && max.is_finite()) {
            return true;
        }
        min.x <= view_max.x && max.x >= view_min.x && min.y <= view_max.y && max.y >= view_min.y
    }

    /// The same stores, drawn `time` seconds into the clock.
    pub fn at_time(mut self, time: f32) -> Self {
        self.time = time;
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
