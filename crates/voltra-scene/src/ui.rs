//! Screen-space UI: elements placed by [`UiRect`] on the target rather than
//! in the world, drawn after it in the target's own pixels.
//!
//! A HUD, and since stage 35 buttons: [`UiButton`] takes clicks, hover and
//! keyboard/gamepad focus.

pub mod button;
pub mod layout;
pub mod mesh;
pub mod rect;

pub use button::{Navigation, Pointer, UiButton, UiButtons};
pub use layout::UiLayout;
pub use rect::{Rect, UiRect};

use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;

/// Whether `entity` is drawn in screen space — and therefore not in the
/// world, by the world batch or the world pick.
pub fn is_ui(world: &World, entity: Entity) -> bool {
    world.get::<UiRect>(entity).is_some()
}

/// The screen-space element under `point`, in points from the target's
/// top-left: the topmost one with something to draw, by the order
/// [`mesh::batch`] draws in.
pub fn element_at(world: &World, layout: &UiLayout, point: Vec2) -> Option<Entity> {
    world
        .query::<UiRect>()
        .filter_map(|(entity, _)| {
            let order = world
                .get::<crate::Text>(entity)
                .map(|text| text.sort_order)
                .into_iter()
                .chain(world.get::<crate::Sprite>(entity).map(|s| s.sort_order))
                .max()?;
            layout
                .rect(entity)
                .filter(|rect| rect.contains(point))
                .map(|_| (layout.draw_key(entity, order), entity))
        })
        .max_by_key(|(key, _)| *key)
        .map(|(_, entity)| entity)
}

#[cfg(test)]
mod tests;
