//! The wheel, the focus and the content keeping each scroll's offset.

use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;

use crate::hierarchy;
use crate::ui::{UiLayout, UiScroll};

/// The wheel scrolls the innermost scroll under the pointer — the nearest
/// scroll above the element there, or one whose own rect holds it.
pub(super) fn wheel(world: &mut World, layout: &UiLayout, at: Vec2, lines: Vec2) {
    let hit = crate::ui::element_at(world, layout, at);
    let from_hit = hit.and_then(|e| {
        std::iter::once(e)
            .chain(hierarchy::ancestors(world, e))
            .find(|&a| world.get::<UiScroll>(a).is_some())
    });
    let scroll = from_hit.or_else(|| {
        world
            .query::<UiScroll>()
            .filter(|(e, _)| layout.visible(*e).is_some_and(|r| r.contains(at)))
            .max_by_key(|(e, _)| layout.depth(*e))
            .map(|(e, _)| e)
    });
    if let Some(scroll) = scroll.and_then(|e| world.get_mut::<UiScroll>(e)) {
        scroll.wheel(lines);
    }
}

/// Scrolls every scroll above `control` just far enough to show it: the
/// focus moving down a list moves the list.
pub(super) fn reveal(world: &mut World, layout: &UiLayout, control: Entity) {
    let Some(mut target) = layout.rect(control) else {
        return;
    };
    let mut inner = layout.scroll_of(control);
    while let Some(scroll) = inner {
        let Some(view) = layout.rect(scroll) else {
            break;
        };
        if let Some(component) = world.get_mut::<UiScroll>(scroll) {
            let before = component.shift();
            component.reveal(view, target);
            // Seen from further out, the target moved with this scroll.
            target = target.translated(before - component.shift());
        }
        inner = layout.scroll_of(scroll);
    }
}

/// How far each scroll's content reaches past its view: the most its
/// offset may be. Read from the layout, so before any offset changes.
pub(super) fn reaches(world: &World, layout: &UiLayout) -> Vec<(Entity, Vec2)> {
    world
        .query::<UiScroll>()
        .filter_map(|(entity, scroll)| {
            let view = layout.rect(entity)?;
            let padding = world
                .get::<crate::ui::UiStack>(entity)
                .map_or(Vec2::ZERO, |stack| stack.padding.max());
            let reach = match layout.content(entity) {
                // The content as laid out, moved back by the offset it was
                // laid out with.
                Some(content) => content.max + scroll.shift() + padding - view.max,
                None => Vec2::ZERO,
            };
            Some((entity, reach))
        })
        .collect()
}
