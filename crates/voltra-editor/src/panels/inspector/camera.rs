//! The scene camera's framing and which one wins.

use voltra_core::egui::{self, DragValue, RichText, Ui};
use voltra_scene::Camera;

use super::active;

/// The camera's framing, in the units the component stores it in.
///
/// `size` is dragged rather than typed as a zoom because that is what the field
/// means — half the world height on screen — and it is the number an artist can
/// check against the scene. Bounded by the component's own constants, so a drag
/// stops at the wall instead of running to infinity and being silently clamped
/// somewhere else.
pub(super) fn show(ui: &mut Ui, camera: &mut Camera) -> Option<&'static str> {
    ui.label(RichText::new("Camera").strong());

    let mut claim = None;
    egui::Grid::new("camera").num_columns(2).show(ui, |ui| {
        ui.label("size")
            .on_hover_text("half the world height this camera shows");
        claim = claim.or(active(
            &ui.add(
                DragValue::new(&mut camera.size)
                    .speed(0.01)
                    .range(Camera::MIN_SIZE..=Camera::MAX_SIZE),
            ),
            "Set camera size",
        ));
        ui.end_row();

        ui.label("priority")
            .on_hover_text("the highest active camera is the one the game renders through");
        claim = claim.or(active(
            &ui.add(DragValue::new(&mut camera.priority)),
            "Set camera priority",
        ));
        ui.end_row();

        ui.label("active");
        // A checkbox is one click, not a held interaction, so `active` — which
        // asks whether a widget is being dragged or holds focus — would never
        // report it. `changed` is the whole of the edit.
        if ui.checkbox(&mut camera.active, "").changed() {
            claim = claim.or(Some("Toggle camera"));
        }
        ui.end_row();
    });
    claim
}
