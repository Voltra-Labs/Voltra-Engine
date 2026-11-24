//! The sections of the UI controls a screen-space element can carry: a
//! toggle, a slider or a part of one, a text field, a scroll.

use voltra_core::egui::Ui;
use voltra_ecs::{Entity, World};
use voltra_scene::{UiSlider, UiSliderPart, UiTextField, UiToggle};

use super::{ui_scroll, ui_slider, ui_text_field, ui_toggle};

pub(super) fn show(ui: &mut Ui, world: &mut World, entity: Entity) -> Option<&'static str> {
    let mut claim = None;
    if let Some(toggle) = world.get_mut::<UiToggle>(entity) {
        claim = claim.or(ui_toggle::show(ui, toggle));
    }
    if let Some(slider) = world.get_mut::<UiSlider>(entity) {
        claim = claim.or(ui_slider::show(ui, slider));
    }
    if let Some(part) = world.get_mut::<UiSliderPart>(entity) {
        claim = claim.or(ui_slider::show_part(ui, part));
    }
    if let Some(field) = world.get_mut::<UiTextField>(entity) {
        claim = claim.or(ui_text_field::show(ui, field));
    }
    claim.or(ui_scroll::show(ui, world, entity))
}
