//! Where an entity sits, which way it faces, and how big it is.

use voltra_core::egui::{self, DragValue, RichText, Ui};
use voltra_scene::Transform;

use super::active;

pub(super) fn show(ui: &mut Ui, transform: &mut Transform) -> Option<&'static str> {
    ui.label(RichText::new("Transform").strong());

    let mut claim = None;
    egui::Grid::new("transform").num_columns(2).show(ui, |ui| {
        ui.label("position");
        ui.horizontal(|ui| {
            claim = claim
                .or(active(
                    &ui.add(DragValue::new(&mut transform.translation.x).speed(0.01)),
                    "Move",
                ))
                .or(active(
                    &ui.add(DragValue::new(&mut transform.translation.y).speed(0.01)),
                    "Move",
                ));
        });
        ui.end_row();

        ui.label("rotation");
        claim = claim.or(active(&ui.drag_angle(&mut transform.rotation), "Rotate"));
        ui.end_row();

        ui.label("scale");
        ui.horizontal(|ui| {
            claim = claim
                .or(active(
                    &ui.add(DragValue::new(&mut transform.scale.x).speed(0.01)),
                    "Scale",
                ))
                .or(active(
                    &ui.add(DragValue::new(&mut transform.scale.y).speed(0.01)),
                    "Scale",
                ));
        });
        ui.end_row();
    });
    claim
}
