//! Putting new UI controls into the scene: each spawned whole — a slider
//! with its fill and handle, a toggle with its label — as one undo entry.

use voltra_core::UiFrame;
use voltra_ecs::Entity;
use voltra_render::glam::Vec2;
use voltra_scene::{
    hierarchy, Align, Sprite, Text, UiButton, UiRect, UiScroll, UiSlider, UiSliderPart, UiStack,
    UiTextField, UiToggle,
};

use super::hud::spawn;

/// Size a control's text is, in points.
const TEXT_SIZE: f32 = 18.0;

fn child(frame: &mut UiFrame<'_>, parent: Entity, entity: Entity) {
    if let Err(e) = hierarchy::set_parent(frame.world, entity, parent) {
        log::error!("could not put a control's part under it: {e}");
    }
}

fn text(frame: &mut UiFrame<'_>, content: &str, align: Align) -> Text {
    let mut text = Text {
        size: TEXT_SIZE,
        align,
        ..Text::new(content)
    };
    text.set_font(None, frame.fonts);
    text
}

/// A checkbox: a button that toggles, labelled.
pub(crate) fn hud_toggle(frame: &mut UiFrame<'_>, name: &str) -> (Vec<Entity>, Option<Entity>) {
    let toggle = spawn(frame, name);
    frame.world.insert(toggle, Sprite::default());
    frame.world.insert(toggle, UiButton::default());
    frame.world.insert(toggle, UiToggle::default());
    let label = spawn(frame, "Label");
    frame.world.insert(label, UiRect::stretched(6.0));
    let words = text(frame, "Toggle", Align::Center);
    frame.world.insert(label, words);
    child(frame, toggle, label);
    (vec![toggle, label], Some(toggle))
}

/// A slider: a dark track, a lit fill up to the value and a handle on it.
pub(crate) fn hud_slider(frame: &mut UiFrame<'_>, name: &str) -> (Vec<Entity>, Option<Entity>) {
    let slider = spawn(frame, name);
    frame.world.insert(
        slider,
        UiRect::pinned(Vec2::ZERO, Vec2::splat(16.0), Vec2::new(200.0, 16.0)),
    );
    frame
        .world
        .insert(slider, Sprite::new([0.12, 0.13, 0.17, 1.0]));
    frame.world.insert(slider, UiSlider::default());
    let fill = spawn(frame, "Fill");
    frame.world.insert(fill, UiRect::stretched(0.0));
    frame.world.insert(fill, Sprite::new([0.3, 0.5, 0.9, 1.0]));
    frame.world.insert(fill, UiSliderPart::Fill);
    child(frame, slider, fill);
    let handle = spawn(frame, "Handle");
    frame.world.insert(
        handle,
        UiRect {
            anchor_min: Vec2::new(0.5, 0.0),
            anchor_max: Vec2::new(0.5, 1.0),
            offset_min: Vec2::new(-6.0, -4.0),
            offset_max: Vec2::new(6.0, 4.0),
            slot: 0,
        },
    );
    frame
        .world
        .insert(handle, Sprite::new([0.9, 0.92, 1.0, 1.0]));
    frame.world.insert(handle, UiSliderPart::Handle);
    child(frame, slider, handle);
    (vec![slider, fill, handle], Some(slider))
}

/// A text field: a box, the default font, a placeholder.
pub(crate) fn hud_text_field(frame: &mut UiFrame<'_>, name: &str) -> Entity {
    let field = spawn(frame, name);
    frame.world.insert(
        field,
        UiRect::pinned(Vec2::ZERO, Vec2::splat(16.0), Vec2::new(220.0, 28.0)),
    );
    frame
        .world
        .insert(field, Sprite::new([0.08, 0.09, 0.12, 1.0]));
    let style = text(frame, "", Align::Left);
    frame.world.insert(field, style);
    frame.world.insert(
        field,
        UiTextField {
            placeholder: "Type here".to_owned(),
            ..UiTextField::default()
        },
    );
    field
}

/// A list that scrolls: a dark panel stacking its children in a column,
/// cut to its rect.
pub(crate) fn hud_scroll_list(frame: &mut UiFrame<'_>, name: &str) -> Entity {
    let list = super::hud::hud_panel(frame, name);
    frame.world.insert(
        list,
        UiRect::pinned(Vec2::ZERO, Vec2::splat(16.0), Vec2::new(240.0, 200.0)),
    );
    frame.world.insert(list, UiStack::default());
    frame.world.insert(list, UiScroll::default());
    list
}
