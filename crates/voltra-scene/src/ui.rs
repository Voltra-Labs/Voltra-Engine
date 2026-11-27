//! Screen-space UI: elements placed by [`UiRect`] on the target rather than
//! in the world, drawn after it in the target's own pixels.
//!
//! A HUD, and since stage 35 buttons: [`UiButton`] takes clicks, hover and
//! keyboard/gamepad focus. An image is a `Sprite2D` on a `UiRect` — a
//! texture, a region or an atlas frame, tinted — drawn whole or, with a
//! [`UiSlice`], as a 9-slice; a [`UiStack`] lays its children out in a row
//! or a column. Stage 94 added [`UiDropdown`], [`UiTooltip`], [`UiTabs`]
//! and a [`UiTheme`] the controls take.

pub mod button;
pub mod controls;
pub mod dropdown;
pub mod edges;
pub mod hidden;
pub mod layout;
pub mod mesh;
pub mod overlay;
pub mod rect;
pub mod scroll;
pub mod slice;
pub mod slider;
pub mod stack;
pub mod tabs;
pub mod text_field;
pub mod theme;
pub mod toggle;
pub mod tooltip;

pub use button::{ButtonFrames, UiButton};
pub use controls::{Navigation, Pointer, UiControls, UiInput};
pub use dropdown::UiDropdown;
pub use edges::Edges;
pub use hidden::UiHidden;
pub use layout::UiLayout;
pub use rect::{Rect, UiRect};
pub use scroll::UiScroll;
pub use slice::UiSlice;
pub use slider::{UiSlider, UiSliderPart};
pub use stack::UiStack;
pub use tabs::{UiTab, UiTabs};
pub use text_field::{TextInput, UiTextField};
pub use theme::{UiOwnStyle, UiTheme};
pub use toggle::UiToggle;
pub use tooltip::UiTooltip;

use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;

/// Whether `entity` is drawn in screen space — and therefore not in the
/// world, by the world batch or the world pick.
pub fn is_ui(world: &World, entity: Entity) -> bool {
    world.get::<UiRect>(entity).is_some()
}

/// The screen-space element under `point`, in points from the target's
/// top-left: the topmost one with something to draw, or a control, by the
/// order [`mesh::batch`] draws in. A part a scroll masks off is not there.
pub fn element_at(world: &World, layout: &UiLayout, point: Vec2) -> Option<Entity> {
    world
        .query::<UiRect>()
        .filter_map(|(entity, _)| {
            // A control takes the pointer with nothing drawn: an invisible
            // hit area, a slider whose track is its parent's picture.
            let order = world
                .get::<crate::Text>(entity)
                .map(|text| text.sort_order)
                .into_iter()
                .chain(world.get::<crate::Sprite2D>(entity).map(|s| s.sort_order))
                .chain(controls::is_control(world, entity).then_some(0))
                .max()?;
            layout
                .visible(entity)
                .filter(|rect| rect.contains(point))
                .map(|_| (layout.draw_key(entity, order), entity))
        })
        .max_by_key(|(key, _)| *key)
        .map(|(_, entity)| entity)
}

#[cfg(test)]
mod tests;
