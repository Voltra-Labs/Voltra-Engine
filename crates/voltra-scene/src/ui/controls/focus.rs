//! Which elements take the focus, and where the navigation moves it.

use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;

use crate::hierarchy;
use crate::ui::button::UiButton;
use crate::ui::slider::UiSlider;
use crate::ui::text_field::UiTextField;
use crate::ui::UiLayout;

/// Whether `entity` is a control: a button (a toggle is one), a slider or
/// a text field.
pub fn is_control(world: &World, entity: Entity) -> bool {
    world.get::<UiButton>(entity).is_some()
        || world.get::<UiSlider>(entity).is_some()
        || world.get::<UiTextField>(entity).is_some()
}

/// A control that takes input now: not a disabled button, and laid out.
pub(super) fn usable(world: &World, layout: &UiLayout, entity: Entity) -> bool {
    is_control(world, entity)
        && !world.get::<UiButton>(entity).is_some_and(|b| b.disabled)
        && layout.rect(entity).is_some()
}

/// The control `hit` is, or the nearest one above it — a click on a label
/// is a click on the control it labels, a click on a slider's handle a
/// click on the slider.
pub(super) fn control_owning(world: &World, hit: Entity) -> Option<Entity> {
    std::iter::once(hit)
        .chain(hierarchy::ancestors(world, hit))
        .find(|&e| is_control(world, e))
}

/// The usable control nearest `from` in `direction`, by Godot's rule of
/// thumb: ahead of it, scored by distance with sideways drift weighing
/// double. With no focus yet, the top-left-most. Controls scrolled out of
/// sight count: moving the focus to one scrolls it into view.
pub(super) fn next(
    world: &World,
    layout: &UiLayout,
    from: Option<Entity>,
    direction: Vec2,
) -> Option<Entity> {
    let controls: Vec<(Entity, Vec2)> = world
        .query::<crate::ui::UiRect>()
        .filter(|(e, _)| usable(world, layout, *e))
        .filter_map(|(e, _)| layout.rect(e).map(|r| (e, r.center())))
        .collect();
    let Some(from) = from.and_then(|f| controls.iter().find(|(e, _)| *e == f).copied()) else {
        return controls
            .iter()
            .min_by(|a, b| {
                (a.1.y, a.1.x)
                    .partial_cmp(&(b.1.y, b.1.x))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(e, _)| *e);
    };
    let direction = direction.normalize_or_zero();
    if direction == Vec2::ZERO {
        return Some(from.0);
    }
    controls
        .iter()
        .filter(|(e, _)| *e != from.0)
        .filter_map(|&(e, center)| {
            let offset = center - from.1;
            let ahead = offset.dot(direction);
            if ahead <= 0.0 {
                return None;
            }
            let sideways = (offset - direction * ahead).length();
            Some((e, ahead + 2.0 * sideways))
        })
        .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(e, _)| e)
        .or(Some(from.0))
}
