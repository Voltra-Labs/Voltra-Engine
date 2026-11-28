//! The 3D sun: its colour, energy and layers.

use voltra_core::egui::{self, DragValue, RichText, Ui};
use voltra_scene::DirectionalLight3D;

use super::active;

/// The light's colour (linear, picked in sRGB), its energy, in the light
/// units' stored value, and its layers; what it cannot draw with, under
/// them.
pub(super) fn show(ui: &mut Ui, light: &mut DirectionalLight3D) -> Option<&'static str> {
    ui.label(RichText::new(DirectionalLight3D::NAME).strong())
        .on_hover_text(
        "lights the 3D world along the entity's −Z, every ray parallel; up to four light a view",
    );
    let mut claim = None;
    egui::Grid::new("directional_light3d")
        .num_columns(2)
        .show(ui, |ui| {
            ui.label("colour");
            // A popup, as every colour field is: see `sprite::show`.
            let popup = ui.auto_id_with("popup");
            let response = crate::panels::colour_edit::rgb(ui, &mut light.color);
            if egui::Popup::is_id_open(ui.ctx(), popup) || response.changed() {
                claim = Some("Set light colour");
            }
            ui.end_row();

            ui.label("energy").on_hover_text(
                "1 is a usable sun (simple units: the illuminance over K = 1.2·2¹⁵ lux)",
            );
            claim = claim.or(active(
                &ui.add(
                    DragValue::new(&mut light.energy)
                        .speed(0.01)
                        .range(0.0..=f32::MAX),
                ),
                "Set light energy",
            ));
            ui.end_row();
        });
    claim = claim.or(super::light_layers::show(
        ui,
        "directional_light3d_layers",
        "lights these layers",
        "lights only meshes on one of these",
        &mut light.layers,
    ));
    if let Some(problem) = light.problem() {
        ui.label(
            RichText::new(problem)
                .color(ui.visuals().error_fg_color)
                .small(),
        );
    }
    claim
}
