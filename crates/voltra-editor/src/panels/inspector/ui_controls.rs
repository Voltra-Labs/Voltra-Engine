//! The sections of the UI controls a screen-space element can carry: a
//! toggle, a slider or a part of one, a text field, a dropdown, a tooltip,
//! tabs, a scroll.

use voltra_core::egui::Ui;
use voltra_ecs::{Entity, World};
use voltra_scene::{UiDropdown, UiSlider, UiSliderPart, UiTextField, UiToggle, UiTooltip};

use super::mixed;
use super::{ui_dropdown, ui_scroll, ui_slider, ui_tabs, ui_text_field, ui_toggle, ui_tooltip};

use crate::multi_edit::MultiEdit;

/// Each control `entity` carries that is not one only some of the
/// selection have, with what differs across it.
pub(super) fn show(
    ui: &mut Ui,
    world: &mut World,
    entity: Entity,
    multi: &MultiEdit,
) -> Option<&'static str> {
    let mut claim = None;
    let open = |ui: &mut Ui, name: &str| {
        let shared = multi.editable(&[name]);
        if shared {
            mixed::marker(ui, multi, &[name]);
        }
        shared
    };
    if world.get::<UiToggle>(entity).is_some() && open(ui, UiToggle::NAME) {
        if let Some(toggle) = world.get_mut::<UiToggle>(entity) {
            claim = claim.or(ui_toggle::show(ui, toggle));
        }
    }
    if world.get::<UiSlider>(entity).is_some() && open(ui, UiSlider::NAME) {
        if let Some(slider) = world.get_mut::<UiSlider>(entity) {
            claim = claim.or(ui_slider::show(ui, slider));
        }
    }
    if world.get::<UiSliderPart>(entity).is_some() && open(ui, UiSliderPart::NAME) {
        if let Some(part) = world.get_mut::<UiSliderPart>(entity) {
            claim = claim.or(ui_slider::show_part(ui, part));
        }
    }
    if world.get::<UiTextField>(entity).is_some() && open(ui, UiTextField::NAME) {
        if let Some(field) = world.get_mut::<UiTextField>(entity) {
            claim = claim.or(ui_text_field::show(ui, field));
        }
    }
    if world.get::<UiDropdown>(entity).is_some() && open(ui, UiDropdown::NAME) {
        if let Some(dropdown) = world.get_mut::<UiDropdown>(entity) {
            claim = claim.or(ui_dropdown::show(ui, dropdown));
        }
    }
    if world.get::<UiTooltip>(entity).is_some() && open(ui, UiTooltip::NAME) {
        if let Some(tooltip) = world.get_mut::<UiTooltip>(entity) {
            claim = claim.or(ui_tooltip::show(ui, tooltip));
        }
    }
    if multi.editable(&[voltra_scene::UiTabs::NAME, voltra_scene::UiTab::NAME]) {
        mixed::marker(
            ui,
            multi,
            &[voltra_scene::UiTabs::NAME, voltra_scene::UiTab::NAME],
        );
        claim = claim.or(ui_tabs::show(ui, world, entity));
    }
    if multi.editable(&[voltra_scene::UiScroll::NAME]) {
        mixed::marker(ui, multi, &[voltra_scene::UiScroll::NAME]);
        claim = claim.or(ui_scroll::show(ui, world, entity));
    }
    claim
}
