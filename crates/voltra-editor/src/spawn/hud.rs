//! Putting new screen-space elements into the scene.

use voltra_core::UiFrame;
use voltra_ecs::Entity;
use voltra_scene::{Name, SceneId, Sprite, Text, UiRect};

/// Size a new HUD label's text is, in points.
const HUD_TEXT_SIZE: f32 = 24.0;

/// Drops a HUD label in the top-left corner, in the default font.
///
/// No `Transform`: a `UiRect` places it, and a transform would only offer
/// the gizmo something that moves nothing.
pub(crate) fn hud_text(frame: &mut UiFrame<'_>, name: &str) -> Entity {
    let entity = spawn(frame, name);
    let mut text = Text {
        size: HUD_TEXT_SIZE,
        ..Text::default()
    };
    text.set_font(None, frame.fonts);
    frame.world.insert(entity, text);
    entity
}

/// Drops a dark HUD panel in the top-left corner, behind labels on 0.
pub(crate) fn hud_panel(frame: &mut UiFrame<'_>, name: &str) -> Entity {
    let entity = spawn(frame, name);
    frame.world.insert(
        entity,
        Sprite::new([0.08, 0.09, 0.12, 1.0]).with_sort_order(-1),
    );
    entity
}

fn spawn(frame: &mut UiFrame<'_>, name: &str) -> Entity {
    let entity = frame.world.spawn();
    frame.world.insert(entity, SceneId::new());
    frame.world.insert(entity, Name::new(name));
    frame.world.insert(entity, UiRect::default());
    entity
}
