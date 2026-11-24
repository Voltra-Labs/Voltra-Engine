//! The held control following the pointer.

use voltra_assets::Fonts;
use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;

use super::{Pointer, UiControls};
use crate::ui::layout::UiLayout;
use crate::ui::slider::UiSlider;
use crate::ui::text_field::{view, UiTextField};

impl UiControls {
    /// The held control following the pointer: a slider's value, a text
    /// field's caret — and its selection after the press.
    pub(super) fn drag(
        &mut self,
        world: &mut World,
        layout: &UiLayout,
        fonts: Option<&Fonts>,
        held: Entity,
        at: Vec2,
        pointer: Pointer,
    ) {
        if let Some(track) = layout.rect(held) {
            if let Some(slider) = world.get_mut::<UiSlider>(held) {
                if slider.set_at(track, at) {
                    self.changed.push(held);
                }
                return;
            }
        }
        if let Some(index) = view::index_at(world, held, layout, fonts, at) {
            if let Some(field) = world.get_mut::<UiTextField>(held) {
                field.place_caret(index, pointer.extend || !pointer.went_down);
            }
        }
    }
}
