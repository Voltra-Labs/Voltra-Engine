//! One loaded font: its face, the glyphs rasterized from it, and the texture
//! they were uploaded into.

use std::fmt;

use voltra_render::Texture;

use super::atlas::{GlyphAtlas, GlyphKey, GlyphMetrics};
use super::layout::{layout, TextLayout, Wrap};
use crate::handle::Handle;

/// A face plus its glyph atlas.
///
/// The face is optional: a path that would not parse, in a project whose
/// default font is missing too, still gets a slot — so a later fix to the
/// file can fill it under the handle every `Text` already holds — but it
/// draws nothing until then.
pub struct Font {
    face: Option<fontdue::Font>,
    atlas: GlyphAtlas,
    /// The GPU copy of `atlas`, once [`Fonts::upload`](crate::Fonts::upload)
    /// has made one. Stable across growths and reloads, for the same reason
    /// every other handle in this crate is.
    texture: Option<Handle<Texture>>,
}

impl Font {
    pub fn new(face: Option<fontdue::Font>) -> Self {
        Self {
            face,
            atlas: GlyphAtlas::default(),
            texture: None,
        }
    }

    pub fn face(&self) -> Option<&fontdue::Font> {
        self.face.as_ref()
    }

    pub fn atlas(&self) -> &GlyphAtlas {
        &self.atlas
    }

    pub(crate) fn atlas_mut(&mut self) -> &mut GlyphAtlas {
        &mut self.atlas
    }

    /// The texture the atlas is drawn from, or `None` before its first
    /// upload.
    pub fn texture(&self) -> Option<Handle<Texture>> {
        self.texture
    }

    pub(crate) fn set_texture(&mut self, texture: Handle<Texture>) {
        self.texture = Some(texture);
    }

    /// `content` laid out at `px` pixels per em, broken as `wrap` says, or
    /// `None` with no face.
    pub fn layout(&self, content: &str, px: u32, wrap: Wrap) -> Option<TextLayout> {
        Some(layout(self.face.as_ref()?, content, px, wrap))
    }

    /// Rasterizes every glyph of `content` at `px` the atlas does not hold.
    ///
    /// Separate from drawing because the atlas may grow and repack while
    /// glyphs are added, which moves every glyph already in it: UVs computed
    /// before the last insertion of the frame would be wrong.
    pub fn prepare(&mut self, content: &str, px: u32) {
        let Some(face) = self.face.as_ref() else {
            return;
        };
        for character in content.chars().filter(|c| !c.is_control()) {
            let glyph = face.lookup_glyph_index(character);
            let key = GlyphKey { glyph, px };
            if self.atlas.contains(key) {
                continue;
            }
            let (metrics, coverage) = face.rasterize_indexed(glyph, px as f32);
            let metrics = GlyphMetrics {
                xmin: metrics.xmin,
                ymin: metrics.ymin,
                width: metrics.width as u32,
                height: metrics.height as u32,
            };
            self.atlas.insert(key, metrics, coverage);
        }
    }

    /// Swaps in a re-read face. Every glyph index may now mean another
    /// shape, so the atlas starts over; the texture handle is kept and
    /// refilled on the next upload.
    pub(crate) fn replace_face(&mut self, face: fontdue::Font) {
        self.face = Some(face);
        self.atlas = GlyphAtlas::default();
    }
}

impl fmt::Debug for Font {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Font")
            .field("face", &self.face.as_ref().and_then(|face| face.name()))
            .field("glyphs", &self.atlas.len())
            .field("texture", &self.texture)
            .finish()
    }
}
