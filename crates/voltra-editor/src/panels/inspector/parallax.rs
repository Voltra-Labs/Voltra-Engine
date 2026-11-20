//! How much of the camera's movement a layer follows.

use voltra_core::egui::{self, DragValue, RichText, Ui};
use voltra_scene::Parallax;

use super::active;

/// The factor per axis: 1 moves with the world, 0 stays with the camera.
pub(super) fn show(ui: &mut Ui, parallax: &mut Parallax) -> Option<&'static str> {
    ui.label(RichText::new("Parallax").strong()).on_hover_text(
        "drawn as if the camera moved this much: 1 = world, 0 = fixed to the camera",
    );
    let mut claim = None;
    egui::Grid::new("parallax").num_columns(3).show(ui, |ui| {
        ui.label("factor");
        for component in [&mut parallax.factor.x, &mut parallax.factor.y] {
            claim = claim.or(active(
                &ui.add(DragValue::new(component).speed(0.01).range(-4.0..=4.0)),
                "Set parallax",
            ));
        }
        ui.end_row();
    });
    claim
}
