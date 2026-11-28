//! Where a 3D entity sits, which way it faces, and how big it is.
//!
//! The rotation is edited as Euler angles in degrees, YXZ (Godot's order),
//! through [`EulerHints`]: what was typed is what stays shown, though the
//! file stores the quaternion.

use voltra_core::egui::{self, DragValue, RichText, Ui};
use voltra_ecs::Entity;
use voltra_scene::format::serial_tree::Step;
use voltra_scene::Transform3D;

use super::active;
use super::mixed::{self, Fields};
use crate::euler_hint::EulerHints;

/// The three rows; a translation or scale axis that differs across the
/// selection reads `—`, and a rotation that does is marked on its label.
pub(super) fn show(
    ui: &mut Ui,
    entity: Entity,
    transform: &mut Transform3D,
    hints: &mut EulerHints,
    mixed: Fields<'_>,
) -> Option<&'static str> {
    ui.label(RichText::new(Transform3D::NAME).strong());

    let mut claim = None;
    egui::Grid::new("transform3d")
        .num_columns(2)
        .show(ui, |ui| {
            ui.label("position");
            ui.horizontal(|ui| {
                for (axis, value) in transform.translation.as_mut().iter_mut().enumerate() {
                    let drag = DragValue::new(value).speed(0.01);
                    let path = vec![Step::Field("translation"), Step::Index(axis)];
                    claim = claim.or(active(&mixed::drag(ui, drag, mixed, path), "Move"));
                }
            });
            ui.end_row();

            // Written whole: the angles are a view of one quaternion, so an
            // edit to any of them writes the rotation to the whole selection.
            // Each angle is the shared mixed number on the whole rotation:
            // `—` when the selection's rotations differ, and a number typed
            // into it and left is written to all even when it is the
            // primary's own (the touch `mixed::drag` makes on that path).
            mixed::label(ui, "rotation", mixed.field("rotation"));
            let mut degrees = hints.degrees(entity, transform.rotation);
            let mut changed = false;
            ui.horizontal(|ui| {
                for value in degrees.as_mut().iter_mut() {
                    let drag = DragValue::new(value).speed(0.5).suffix("°");
                    let response = mixed::drag(ui, drag, mixed, vec![Step::Field("rotation")]);
                    changed |= response.changed();
                    claim = claim.or(active(&response, "Rotate"));
                }
            })
            .response
            .on_hover_text("degrees about X, Y and Z, applied Y, then X, then Z");
            if changed {
                transform.rotation = hints.typed(entity, degrees);
                // One value: the selection is written the whole quaternion,
                // not the components that moved, which would mix the
                // primary's rotation into each other entity's.
                mixed.touch(ui, vec![Step::Field("rotation")]);
            }
            ui.end_row();

            ui.label("scale");
            ui.horizontal(|ui| {
                for (axis, value) in transform.scale.as_mut().iter_mut().enumerate() {
                    let drag = DragValue::new(value).speed(0.01);
                    let path = vec![Step::Field("scale"), Step::Index(axis)];
                    claim = claim.or(active(&mixed::drag(ui, drag, mixed, path), "Scale"));
                }
            });
            ui.end_row();
        });
    claim
}
