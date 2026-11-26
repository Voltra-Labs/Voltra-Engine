//! The joint section's rows: an angle shown in degrees, a plain number, an
//! axis, an optional part, a breaking threshold.

use voltra_core::egui::{DragValue, RichText, Ui};
use voltra_render::glam::Vec2;

use super::super::active;
use super::super::light::optional;

/// A row for an angle stored in radians, shown in degrees.
pub(super) fn degrees(ui: &mut Ui, label: &str, radians: &mut f32) -> Option<&'static str> {
    ui.label(label);
    let mut shown = radians.to_degrees();
    let response = ui.add(DragValue::new(&mut shown).speed(0.5).suffix("°"));
    if response.changed() {
        *radians = shown.to_radians();
    }
    ui.end_row();
    active(&response, "Edit joint")
}

/// A row for a plain number.
pub(super) fn number(
    ui: &mut Ui,
    label: &str,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
) -> Option<&'static str> {
    ui.label(label);
    let response = ui.add(DragValue::new(value).speed(0.01).range(range));
    ui.end_row();
    active(&response, "Edit joint")
}

/// A row for a direction stored as a vector, shown as its angle in degrees
/// from the x axis — so the inspector can never make a zero one.
pub(super) fn axis(ui: &mut Ui, label: &str, hint: &str, axis: &mut Vec2) -> Option<&'static str> {
    ui.label(label).on_hover_text(hint);
    let mut shown = axis.y.atan2(axis.x).to_degrees();
    let response = ui.add(DragValue::new(&mut shown).speed(0.5).suffix("°"));
    if response.changed() {
        *axis = Vec2::from_angle(shown.to_radians());
    }
    ui.end_row();
    active(&response, "Edit joint")
}

/// A row switching an optional part on and off; `key` keeps what was
/// switched off, so switching it back brings it back.
pub(super) fn toggle<T>(
    ui: &mut Ui,
    label: &str,
    hint: &str,
    key: &str,
    value: &mut Option<T>,
) -> Option<&'static str>
where
    T: Clone + Default + Send + Sync + 'static,
{
    ui.label(label).on_hover_text(hint);
    let changed = optional(ui, key, value);
    ui.end_row();
    changed.then_some("Toggle joint part")
}

/// A row for a threshold that may be off: a box, and the number when on.
/// Switched on, it starts at `start` — not at zero, which would break the
/// joint on the next step.
pub(super) fn threshold(
    ui: &mut Ui,
    label: &str,
    hint: &str,
    value: &mut Option<f32>,
    start: f32,
) -> Option<&'static str> {
    ui.label(label).on_hover_text(hint);
    let mut claim = None;
    ui.horizontal(|ui| {
        let mut on = value.is_some();
        if ui.checkbox(&mut on, "").changed() {
            *value = on.then_some(start);
            claim = Some("Toggle joint breaking");
        }
        if let Some(limit) = value {
            claim = claim.or(active(
                &ui.add(DragValue::new(limit).speed(1.0).range(0.0..=1e9)),
                "Edit joint",
            ));
        } else {
            ui.label(RichText::new("never breaks").weak());
        }
    });
    ui.end_row();
    claim
}
