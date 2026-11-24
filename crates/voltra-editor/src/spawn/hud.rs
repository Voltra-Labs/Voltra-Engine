//! Putting new screen-space elements into the scene.

use voltra_core::UiFrame;
use voltra_ecs::Entity;
use voltra_render::glam::Vec2;
use voltra_scene::{
    hierarchy, Align, Name, ParticleEmitter, SceneId, Sprite, Text, Transform, UiButton, UiRect,
    UiStack,
};

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

/// Drops a dark HUD panel that stacks its UI children in a column — a
/// menu's frame: buttons parented under it line up by slot.
pub(crate) fn hud_column(frame: &mut UiFrame<'_>, name: &str) -> Entity {
    let entity = hud_panel(frame, name);
    frame.world.insert(
        entity,
        UiRect::pinned(Vec2::splat(0.5), Vec2::ZERO, Vec2::new(220.0, 260.0)),
    );
    frame.world.insert(entity, UiStack::default());
    entity
}

/// Drops a HUD button with a "Button" label as its child: the shape every
/// menu is made of. Answers both, so one undo takes both away, and the
/// button to select.
pub(crate) fn hud_button(frame: &mut UiFrame<'_>, name: &str) -> (Vec<Entity>, Option<Entity>) {
    let button = spawn(frame, name);
    frame.world.insert(button, Sprite::default());
    frame.world.insert(button, UiButton::default());
    let label = spawn(frame, "Label");
    frame.world.insert(label, UiRect::stretched(6.0));
    let mut text = Text {
        size: HUD_TEXT_SIZE * 0.75,
        align: Align::Center,
        ..Text::new("Button")
    };
    text.set_font(None, frame.fonts);
    frame.world.insert(label, text);
    if let Err(e) = hierarchy::set_parent(frame.world, label, button) {
        log::error!("could not label the button: {e}");
    }
    (vec![button, label], Some(button))
}

/// Drops a particle fountain at `at`: the default emitter, which plays once
/// the world simulates.
pub(crate) fn particles(frame: &mut UiFrame<'_>, name: &str, at: Vec2) -> Entity {
    let entity = frame.world.spawn();
    frame.world.insert(entity, SceneId::new());
    frame.world.insert(entity, Name::new(name));
    frame.world.insert(entity, Transform::from_translation(at));
    frame.world.insert(entity, ParticleEmitter::default());
    entity
}

pub(super) fn spawn(frame: &mut UiFrame<'_>, name: &str) -> Entity {
    let entity = frame.world.spawn();
    frame.world.insert(entity, SceneId::new());
    frame.world.insert(entity, Name::new(name));
    frame.world.insert(entity, UiRect::default());
    entity
}
