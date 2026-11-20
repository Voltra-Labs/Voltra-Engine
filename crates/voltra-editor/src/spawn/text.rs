//! Putting a new [`Text`] into the scene.

use voltra_assets::AssetPath;
use voltra_core::UiFrame;
use voltra_ecs::Entity;
use voltra_render::glam::Vec2;
use voltra_scene::{Name, SceneId, Text, Transform};

/// Drops a label at `at` in `font` — the default font when `None` — named
/// after the font file it was dropped from, or `name` otherwise.
///
/// Resolved on the spot, so the label is drawn this very frame.
pub(crate) fn text(
    frame: &mut UiFrame<'_>,
    name: &str,
    at: Vec2,
    font: Option<&AssetPath>,
) -> Entity {
    let entity = frame.world.spawn();
    frame.world.insert(entity, SceneId::new());
    let name = font.map_or_else(|| name.to_owned(), super::name_of);
    frame.world.insert(entity, Name::new(name));
    frame.world.insert(entity, Transform::from_translation(at));
    let mut text = Text::default();
    text.set_font(font.cloned(), frame.fonts);
    frame.world.insert(entity, text);
    entity
}
