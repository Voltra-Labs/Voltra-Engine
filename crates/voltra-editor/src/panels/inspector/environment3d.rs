//! The 3D world's surroundings: its ambient light, background and fog.

use voltra_core::egui::{self, DragValue, RichText, Ui};
use voltra_scene::{Background3D, Environment3D, Fog3D};

use super::active;

/// The ambient light (linear, picked in sRGB) and its energy, the
/// background — the camera's clear, a colour, or a gradient sky — and the
/// fog, on or off; what cannot be drawn, under them.
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
            claim = claim.or(colour(ui, &mut environment.ambient, "Set ambient"));
            ui.end_row();

            ui.label("ambient energy")
                .on_hover_text("what the ambient colour is multiplied by");
            claim = claim.or(active(
                &ui.add(
                    DragValue::new(&mut environment.ambient_energy)
                        .speed(0.01)
                        .range(0.0..=f32::MAX),
                ),
                "Set ambient",
            ));
            ui.end_row();

            ui.label("background")
                .on_hover_text("what is drawn where no mesh is");
            claim = claim.or(background(ui, &mut environment.background));
            ui.end_row();
            claim = claim.or(background_colours(ui, &mut environment.background));

            ui.label("fog").on_hover_text(
                "a medium filling the world, denser below its height: distant things fade into it",
            );
            let mut on = environment.fog.is_some();
            if ui.checkbox(&mut on, "").changed() {
                environment.fog = on.then(Fog3D::default);
                claim = claim.or(Some("Set fog"));
            }
            ui.end_row();
            if let Some(fog) = &mut environment.fog {
                claim = claim.or(fog_fields(ui, fog));
            }
        });
    if let Some(problem) = environment.problem() {
        ui.label(
            RichText::new(problem)
                .color(ui.visuals().error_fg_color)
                .small(),
        );
    }
    claim
}

/// A colour field, linear, picked in sRGB: written only when changed.
fn colour(ui: &mut Ui, color: &mut [f32; 3], label: &'static str) -> Option<&'static str> {
    // A popup, as every colour field is: see `sprite::show`.
    let popup = ui.auto_id_with("popup");
    let response = crate::panels::colour_edit::rgb(ui, color);
    (egui::Popup::is_id_open(ui.ctx(), popup) || response.changed()).then_some(label)
}

/// The background's kind: the camera's clear, a colour, a gradient — a
/// new gradient starting from a daylight sky.
fn background(ui: &mut Ui, background: &mut Background3D) -> Option<&'static str> {
    let shown = match background {
        Background3D::Clear => "the camera's clear",
        Background3D::Color(_) => "colour",
        Background3D::Gradient { .. } => "gradient sky",
    };
    let mut claim = None;
    egui::ComboBox::from_id_salt("environment3d_background")
        .selected_text(shown)
        .show_ui(ui, |ui| {
            let choices = [
                ("the camera's clear", Background3D::Clear),
                ("colour", Background3D::Color([0.05, 0.05, 0.07])),
                ("gradient sky", Background3D::DAYLIGHT),
            ];
            for (label, choice) in choices {
                let chosen = std::mem::discriminant(background) == std::mem::discriminant(&choice);
                if ui.selectable_label(chosen, label).clicked() && !chosen {
                    *background = choice;
                    claim = Some("Set background");
                }
            }
        });
    claim
}

/// The background's colours, a row each.
fn background_colours(ui: &mut Ui, background: &mut Background3D) -> Option<&'static str> {
    let mut claim = None;
    let mut row = |ui: &mut Ui, label: &str, color: &mut [f32; 3]| {
        ui.label(format!("  {label}"));
        claim = claim.or(colour(ui, color, "Set background"));
        ui.end_row();
    };
    match background {
        Background3D::Clear => {}
        Background3D::Color(color) => row(ui, "colour", color),
        Background3D::Gradient {
            top,
            horizon,
            bottom,
        } => {
            row(ui, "top", top);
            row(ui, "horizon", horizon);
            row(ui, "bottom", bottom);
        }
    }
    claim
}

/// The fog's colour, density, height, falloff and sky tint, a row each.
fn fog_fields(ui: &mut Ui, fog: &mut Fog3D) -> Option<&'static str> {
    let mut claim = None;
    ui.label("  colour");
    claim = claim.or(colour(ui, &mut fog.color, "Set fog"));
    ui.end_row();
    let rows: [(&str, &mut f32, f64, &str); 4] = [
        (
            "  density",
            &mut fog.density,
            0.0005,
            "per metre at its height; at 0.01 half the light is gone in some 69 m",
        ),
        (
            "  height",
            &mut fog.height,
            0.05,
            "metres: where it is as dense as said",
        ),
        (
            "  height falloff",
            &mut fog.height_falloff,
            0.005,
            "how fast it thins upward, per metre; 0 the same everywhere",
        ),
        (
            "  sky tint",
            &mut fog.sky_tint,
            0.01,
            "how much the sky behind replaces the fog's colour: 0 none, 1 all",
        ),
    ];
    for (label, value, speed, hint) in rows {
        ui.label(label).on_hover_text(hint);
        let mut drag = DragValue::new(value).speed(speed);
        drag = match label.trim() {
            "height" => drag,
            "sky tint" => drag.range(0.0..=1.0),
            _ => drag.range(0.0..=f32::MAX),
        };
        claim = claim.or(active(&ui.add(drag), "Set fog"));
        ui.end_row();
    }
    claim
}
