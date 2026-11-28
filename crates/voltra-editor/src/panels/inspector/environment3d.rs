//! The 3D world's surroundings: its ambient light.

use voltra_core::egui::{self, RichText, Ui};
use voltra_scene::Environment3D;

/// The ambient light, linear, picked in sRGB.
pub(super) fn show(ui: &mut Ui, environment: &mut Environment3D) -> Option<&'static str> {
    ui.label(RichText::new(Environment3D::NAME).strong()).on_hover_text(
        "on a camera, that camera's; on any other entity, the world's, for every camera without one",
    );
    let mut claim = None;
    egui::Grid::new("environment3d")
        .num_columns(2)
        .show(ui, |ui| {
            ui.label("ambient").on_hover_text(
                "a uniform sky's light: what lights a surface facing away from every light",
            );
            let popup = ui.auto_id_with("popup");
            let response = ui.color_edit_button_rgb(&mut environment.ambient);
            if egui::Popup::is_id_open(ui.ctx(), popup) || response.changed() {
                claim = Some("Set ambient");
            }
            ui.end_row();
        });
    claim
}
