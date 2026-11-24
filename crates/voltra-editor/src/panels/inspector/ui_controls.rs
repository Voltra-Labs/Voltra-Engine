//! The sections of the UI controls a screen-space element can carry: a
//! toggle, a slider or a part of one, a text field, a dropdown, a tooltip,
//! tabs, a scroll.

use voltra_core::egui::Ui;
use voltra_ecs::{Entity, World};
use voltra_scene::{UiDropdown, UiSlider, UiSliderPart, UiTextField, UiToggle, UiTooltip};

use super::{ui_dropdown, ui_scroll, ui_slider, ui_tabs, ui_text_field, ui_toggle, ui_tooltip};

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
    if let Some(dropdown) = world.get_mut::<UiDropdown>(entity) {
        claim = claim.or(ui_dropdown::show(ui, dropdown));
    }
    if let Some(tooltip) = world.get_mut::<UiTooltip>(entity) {
        claim = claim.or(ui_tooltip::show(ui, tooltip));
    }
    claim = claim.or(ui_tabs::show(ui, world, entity));
    claim.or(ui_scroll::show(ui, world, entity))
}
