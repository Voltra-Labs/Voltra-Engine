//! A 3D spot light: a point light's fields and its cone.

use voltra_core::egui::{self, DragValue, RichText, Ui};
use voltra_scene::SpotLight3D;

use super::active;
use super::point_light3d::{problem, shared};

/// The light's colour, energy and range, its cone's inner and outer
/// angles in degrees from the axis, and its layers.
pub(super) fn show(ui: &mut Ui, light: &mut SpotLight3D) -> Option<&'static str> {
    ui.label(RichText::new(SpotLight3D::NAME).strong())
        .on_hover_text(
        "lights the 3D world down the entity's −Z in a cone, whole within the inner angle, gone \
         past the outer; a mesh is lit by the eight lights that matter most to it",
    );
    let mut claim = None;
    egui::Grid::new("spot_light3d")
        .num_columns(2)
        .show(ui, |ui| {
            claim = shared(ui, &mut light.color, &mut light.energy, &mut light.range);
            for (label, angle, hint) in [
                (
                    "inner angle",
                    &mut light.inner_angle,
                    "degrees from the axis it shines whole within",
                ),
                (
                    "outer angle",
                    &mut light.outer_angle,
                    "degrees from the axis past which it gives nothing",
                ),
            ] {
                ui.label(label).on_hover_text(hint);
                claim = claim.or(active(
                    &ui.add(
                        DragValue::new(angle)
                            .speed(0.2)
                            .range(0.0..=SpotLight3D::MAX_ANGLE)
                            .suffix("°"),
                    ),
                    "Set spot cone",
                ));
                ui.end_row();
            }
        });
    claim = claim.or(super::light_layers::show(
        ui,
        "spot_light3d_layers",
        "lights these layers",
        "lights only meshes on one of these",
        &mut light.layers,
    ));
    problem(ui, light.problem());
    claim
}
