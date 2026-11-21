//! A point light's colour, strength and reach.

use voltra_core::egui::{self, DragValue, RichText, Ui};
use voltra_scene::PointLight2D;

use super::active;

pub(super) fn show(ui: &mut Ui, light: &mut PointLight2D) -> Option<&'static str> {
    ui.label(RichText::new("Point light").strong())
        .on_hover_text("lights only a frame whose camera is lit");
    let mut claim = None;
    egui::Grid::new("point_light")
        .num_columns(2)
        .show(ui, |ui| {
            ui.label("colour");
            let popup = ui.auto_id_with("popup");
            let response = ui.color_edit_button_rgb(&mut light.color);
            if egui::Popup::is_id_open(ui.ctx(), popup) || response.changed() {
                claim = claim.or(Some("Edit light"));
            }
            ui.end_row();

            for (label, value, range) in [
                ("intensity", &mut light.intensity, 0.0..=16.0),
                ("radius", &mut light.radius, 0.0..=1000.0),
                ("falloff", &mut light.falloff, 0.05..=16.0),
            ] {
                ui.label(label);
                claim = claim.or(active(
                    &ui.add(DragValue::new(value).speed(0.01).range(range)),
                    "Edit light",
                ));
                ui.end_row();
            }
        });
    claim
}
