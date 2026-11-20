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

        ui.label("repeat")
            .on_hover_text("draw the layer again every this many units, to fill the view");
        let mut repeats = parallax.repeat.is_some();
        if ui.checkbox(&mut repeats, "").changed() {
            parallax.repeat = repeats.then_some(voltra_render::glam::Vec2::new(10.0, 0.0));
            claim = claim.or(Some("Set parallax"));
        }
        ui.end_row();
        if let Some(period) = parallax.repeat.as_mut() {
            ui.label("period");
            for component in [&mut period.x, &mut period.y] {
                claim = claim.or(active(
                    &ui.add(DragValue::new(component).speed(0.1).range(0.0..=10_000.0)),
                    "Set parallax",
                ));
            }
            ui.end_row();
        }
    });
    claim
}
