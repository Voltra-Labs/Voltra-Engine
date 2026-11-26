//! The joint section's rows: an angle shown in degrees, a plain number.

use voltra_core::egui::{DragValue, Ui};

use super::super::active;

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
