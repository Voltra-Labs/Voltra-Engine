//! Handles for what a UI theme wrote into elements: a skin's texture, a
//! font, named by path and left for the loop to load.

use voltra_scene::{Sprite, Text, UiRect};

use super::App;

impl App {
    /// Loads every UI sprite's texture and every UI text's font that has a
    /// path but no handle — what [`UiTheme`](voltra_scene::UiTheme) leaves
    /// when it skins or re-fonts an element. Every frame, after the
    /// controls apply the theme; a query when nothing is waiting.
    pub(super) fn resolve_restyled(&mut self) {
        let waiting: Vec<_> = self
            .world
            .query::<UiRect>()
            .map(|(e, _)| e)
            .filter(|&e| {
                self.world
                    .get::<Sprite>(e)
                    .is_some_and(|s| s.texture.is_some() && s.texture_handle.is_none())
                    || self
                        .world
                        .get::<Text>(e)
                        .is_some_and(|t| t.font_handle.is_none())
            })
            .collect();
        if waiting.is_empty() {
            return;
        }
        if let Some(fonts) = self.fonts.as_mut() {
            for &entity in &waiting {
                if let Some(text) = self.world.get_mut::<Text>(entity) {
                    if text.font_handle.is_none() {
                        let path = text.font.clone();
                        text.set_font(path, fonts);
                    }
                }
            }
        }
        let context = self.renderer.as_ref().map(|renderer| renderer.context());
        if let (Some(textures), Some(context)) = (self.textures.as_mut(), context) {
            for &entity in &waiting {
                if let Some(sprite) = self.world.get_mut::<Sprite>(entity) {
                    if sprite.texture_handle.is_none() {
                        let path = sprite.texture.clone();
                        sprite.set_texture(path, textures, context.device(), context.queue());
                    }
                }
            }
        }
    }
}
