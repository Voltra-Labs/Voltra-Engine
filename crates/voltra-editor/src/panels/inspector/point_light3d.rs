//! A 3D point light, and the fields it shares with a spot light.

use voltra_core::egui::{self, DragValue, RichText, Ui};
use voltra_scene::PointLight3D;

use super::active;

/// The light's colour (linear, picked in sRGB), energy, range and layers;
/// what it cannot draw with, under them.
pub(super) fn show(ui: &mut Ui, light: &mut PointLight3D) -> Option<&'static str> {
    ui.label(RichText::new(PointLight3D::NAME).strong())
        .on_hover_text(
            "lights the 3D world from the entity's position, every way, by the inverse square to \
         nothing at its range; a mesh is lit by the eight lights that matter most to it",
        );
    let mut claim = None;
    egui::Grid::new("point_light3d")
        .num_columns(2)
        .show(ui, |ui| {
            claim = shared(ui, &mut light.color, &mut light.energy, &mut light.range);
        });
    claim = claim.or(super::light_layers::show(
        ui,
        "point_light3d_layers",
        "lights these layers",
        "lights only meshes on one of these",
        &mut light.layers,
    ));
    problem(ui, light.problem());
    claim
}

/// The colour, energy and range rows of a point or spot light, in a grid.
pub(super) fn shared(
    ui: &mut Ui,
    color: &mut [f32; 3],
    energy: &mut f32,
    range: &mut f32,
) -> Option<&'static str> {
    let mut claim = None;
    ui.label("colour");
    let popup = ui.auto_id_with("popup");
    let response = crate::panels::colour_edit::rgb(ui, color);
    if egui::Popup::is_id_open(ui.ctx(), popup) || response.changed() {
        claim = Some("Set light colour");
    }
    ui.end_row();

    ui.label("energy").on_hover_text(
        "at 1 m, 1 lights as a sun of 1 does (simple units: the intensity over K = 1.2·2¹⁵ cd)",
    );
    claim = claim.or(active(
        &ui.add(DragValue::new(energy).speed(0.01).range(0.0..=f32::MAX)),
        "Set light energy",
    ));
    ui.end_row();

    ui.label("range")
        .on_hover_text("metres to where it gives nothing; not scaled with the entity");
    claim = claim.or(active(
        &ui.add(
            DragValue::new(range)
                .speed(0.05)
                .range(0.0..=f32::MAX)
                .suffix(" m"),
        ),
        "Set light range",
    ));
    ui.end_row();
    claim
}

/// What a light cannot draw with, in the error colour.
pub(super) fn problem(ui: &mut Ui, problem: Option<&'static str>) {
    if let Some(problem) = problem {
        ui.label(
            RichText::new(problem)
                .color(ui.visuals().error_fg_color)
                .small(),
        );
    }
}
