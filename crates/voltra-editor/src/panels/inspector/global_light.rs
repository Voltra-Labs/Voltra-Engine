//! A global light's colour, strength and light layers.

use voltra_core::egui::{self, DragValue, RichText, Ui};
use voltra_scene::GlobalLight2D;

use super::active;

pub(super) fn show(ui: &mut Ui, light: &mut GlobalLight2D) -> Option<&'static str> {
    ui.label(RichText::new("Global light").strong())
        .on_hover_text(
            "added onto the camera's ambient for the pixels of its light layers; \
         only in a frame whose camera is lit",
        );
    let mut claim = None;
    egui::Grid::new("global_light")
        .num_columns(2)
        .show(ui, |ui| {
            ui.label("colour");
            let popup = ui.auto_id_with("popup");
            let response = ui.color_edit_button_rgb(&mut light.color);
            if egui::Popup::is_id_open(ui.ctx(), popup) || response.changed() {
                claim = claim.or(Some("Edit global light"));
            }
            ui.end_row();

            ui.label("intensity");
            claim = claim.or(active(
                &ui.add(
                    DragValue::new(&mut light.intensity)
                        .speed(0.01)
                        .range(0.0..=16.0),
                ),
                "Edit global light",
            ));
            ui.end_row();
        });
    claim.or(super::light_layers::show(
        ui,
        "global_light_layers",
        "light layers",
        "it lights only what is on one of these",
        &mut light.layers,
    ))
}
