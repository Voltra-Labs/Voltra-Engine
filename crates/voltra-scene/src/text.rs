//! The text component: a string drawn in the world with a font.

pub mod shape;

use voltra_assets::{AssetPath, Font, Fonts, Handle};
use voltra_ecs::World;

/// Which end of each line sits on the entity's origin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum Align {
    /// Lines start at the origin and run right.
    #[default]
    Left,
    /// Each line is centred on the origin.
    Center,
    /// Lines end at the origin.
    Right,
}

impl Align {
    /// How far left of the origin a line of `width` starts, as a share of
    /// that width.
    pub fn share(self) -> f32 {
        match self {
            Self::Left => 0.0,
            Self::Center => 0.5,
            Self::Right => 1.0,
        }
    }
}

/// A string drawn in world space, through its entity's transform.
///
/// The origin is the first line's baseline at the aligned end, so a label
/// sits on integer coordinates the way a tile edge does; further lines run
/// downward. Drawn by the sprite pipeline, interleaved with sprites and
/// tilemap layers by [`sort_order`](Self::sort_order).
///
/// The colour's alpha is not drawn: the vertex carries RGB only, exactly as
/// it does for [`Sprite`](crate::Sprite).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Text {
    /// What is drawn. `\n` starts a new line.
    pub content: String,
    /// The face, by path relative to the asset root. `None` draws with
    /// [`DEFAULT_FONT`](voltra_assets::DEFAULT_FONT).
    #[serde(default)]
    pub font: Option<AssetPath>,
    /// The em height, in entity units: before the transform's scale, one
    /// line of capitals is a little under this tall.
    pub size: f32,
    /// The raster resolution, in pixels per em. Independent of `size` so a
    /// label meant to be zoomed into can be crisp without being large.
    pub pixels_per_em: u32,
    /// Multiplied with the glyphs' coverage, which is white.
    pub color: [f32; 4],
    #[serde(default)]
    pub align: Align,
    /// Draw order among sprites and tilemap layers. See
    /// [`Sprite::sort_order`](crate::Sprite::sort_order).
    #[serde(default)]
    pub sort_order: i32,
    /// The handle `font` resolves to — or the default font's. Never
    /// serialised: it addresses a slot in this session's [`Fonts`].
    #[serde(skip)]
    pub font_handle: Option<Handle<Font>>,
}

impl Default for Text {
    fn default() -> Self {
        Self {
            content: "Text".to_owned(),
            font: None,
            size: 1.0,
            pixels_per_em: Self::DEFAULT_PIXELS_PER_EM,
            color: [1.0, 1.0, 1.0, 1.0],
            align: Align::Left,
            sort_order: 0,
            font_handle: None,
        }
    }
}

impl Text {
    /// Raster pixels per em when nothing says otherwise: crisp at the sizes
    /// a label is usually seen at, and a few hundred glyphs per atlas.
    pub const DEFAULT_PIXELS_PER_EM: u32 = 48;

    /// The largest raster size accepted. Beyond it one glyph approaches the
    /// atlas's own limit.
    pub const MAX_PIXELS_PER_EM: u32 = 512;

    pub fn new(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            ..Default::default()
        }
    }

    /// The raster size actually used: [`pixels_per_em`](Self::pixels_per_em)
    /// kept within `1..=MAX_PIXELS_PER_EM`, so a zero typed into the
    /// inspector cannot divide by it.
    pub fn px(&self) -> u32 {
        self.pixels_per_em.clamp(1, Self::MAX_PIXELS_PER_EM)
    }

    /// Points the text at `path` (or the default font) and resolves it.
    pub fn set_font(&mut self, path: Option<AssetPath>, fonts: &mut Fonts) {
        self.font_handle = Some(match &path {
            Some(path) => fonts.load(path),
            None => fonts.default_font(),
        });
        self.font = path;
    }
}

/// Re-resolves every [`Text`]'s font handle from its path.
pub fn resolve_fonts(world: &mut World, fonts: &mut Fonts) {
    for (_, text) in world.query_mut::<Text>() {
        let path = text.font.clone();
        text.set_font(path, fonts);
    }
}

/// Rasterizes every glyph the world's texts will draw this frame.
///
/// Before the batch, never during it: the atlas may grow and repack as
/// glyphs are added, and a UV taken before the last insertion would point at
/// where a glyph used to be.
pub fn prepare(world: &World, fonts: &mut Fonts) {
    for (_, text) in world.query::<Text>() {
        if let Some(handle) = text.font_handle {
            fonts.prepare(handle, &text.content, text.px());
        }
    }
}

#[cfg(test)]
mod tests;
