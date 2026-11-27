//! Where an entity sits, which way it faces, and how big it is.

use voltra_core::egui::{self, DragValue, RichText, Ui};
use voltra_scene::format::serial_tree::Step;
use voltra_scene::Transform;

use super::active;
use super::mixed::{self, Fields};

/// The three rows; an axis that differs across the selection reads `—`.
pub(super) fn show(
    ui: &mut Ui,
    transform: &mut Transform,
    mixed: Fields<'_>,
) -> Option<&'static str> {
    ui.label(RichText::new("Transform").strong());

    let mut claim = None;
    egui::Grid::new("transform").num_columns(2).show(ui, |ui| {
        ui.label("position");
        ui.horizontal(|ui| {
            for (axis, value) in [&mut transform.translation.x, &mut transform.translation.y]
                .into_iter()
                .enumerate()
            {
                let drag = DragValue::new(value).speed(0.01);
                let path = vec![Step::Field("translation"), Step::Index(axis)];
                claim = claim.or(active(&mixed::drag(ui, drag, mixed, path), "Move"));
            }
        });
        ui.end_row();

        // An angle widget has no mixed state: the label carries it.
        mixed::label(ui, "rotation", mixed.field("rotation"));
        claim = claim.or(active(&ui.drag_angle(&mut transform.rotation), "Rotate"));
        ui.end_row();

        ui.label("scale");
        ui.horizontal(|ui| {
            for (axis, value) in [&mut transform.scale.x, &mut transform.scale.y]
                .into_iter()
                .enumerate()
            {
                let drag = DragValue::new(value).speed(0.01);
                let path = vec![Step::Field("scale"), Step::Index(axis)];
                claim = claim.or(active(&mixed::drag(ui, drag, mixed, path), "Scale"));
            }
        });
        ui.end_row();
    });
    claim
}
