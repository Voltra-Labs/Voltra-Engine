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
    ui.label(RichText::new(Camera::NAME).strong());

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

    ui.label(RichText::new("Post-processing").strong())
        .on_hover_text("how the frame through this camera is graded; the HUD is not");
    egui::Grid::new("camera_post")
        .num_columns(2)
        .show(ui, |ui| {
            let post = &mut camera.post;
            for (label, value, range) in [
                ("exposure", &mut post.exposure, 0.0..=8.0),
                ("contrast", &mut post.contrast, 0.0..=4.0),
                ("saturation", &mut post.saturation, 0.0..=4.0),
                ("vignette", &mut post.vignette, 0.0..=1.0),
                ("bloom", &mut post.bloom.intensity, 0.0..=4.0),
                ("bloom threshold", &mut post.bloom.threshold, 0.0..=4.0),
                ("bloom softness", &mut post.bloom.softness, 0.0..=1.0),
                ("bloom radius", &mut post.bloom.radius, 0.25..=4.0),
            ] {
                ui.label(label);
                claim = claim.or(active(
                    &ui.add(DragValue::new(value).speed(0.01).range(range)),
                    "Set post-processing",
                ));
                ui.end_row();
            }
            ui.label("tint");
            // A popup, as every colour field is: see `sprite::show`.
            let popup = ui.auto_id_with("popup");
            let response = ui.color_edit_button_rgb(&mut post.tint);
            if egui::Popup::is_id_open(ui.ctx(), popup) || response.changed() {
                claim = claim.or(Some("Set post-processing"));
            }
            ui.end_row();
            if !post.is_neutral() && ui.small_button("Reset").clicked() {
                *post = voltra_scene::PostProcess::default();
                claim = claim.or(Some("Set post-processing"));
            }
        });

    ui.label(RichText::new("Lighting").strong())
        .on_hover_text("lit: the ambient everything gets, and every point light adds to it");
    egui::Grid::new("camera_lighting")
        .num_columns(2)
        .show(ui, |ui| {
            ui.label("lit");
            let mut lit = camera.ambient.is_some();
            if ui.checkbox(&mut lit, "").changed() {
                // A dim blue night, so turning it on shows the lights at once.
                camera.ambient = lit.then_some([0.2, 0.22, 0.3]);
                claim = claim.or(Some("Set lighting"));
            }
            ui.end_row();
            if let Some(ambient) = &mut camera.ambient {
                ui.label("ambient");
                let popup = ui.auto_id_with("popup");
                let response = ui.color_edit_button_rgb(ambient);
                if egui::Popup::is_id_open(ui.ctx(), popup) || response.changed() {
                    claim = claim.or(Some("Set lighting"));
                }
                ui.end_row();

                ui.label("shadow texels").on_hover_text(
                    "shadow map texels per quarter turn around a light: sharper for more memory",
                );
                claim = claim.or(active(
                    &ui.add(
                        DragValue::new(&mut camera.shadow_resolution)
                            .speed(8.0)
                            .range(64..=4096)
                            // A value written outside the range by hand is
                            // the renderer's to clamp, not the inspector's to
                            // rewrite on sight.
                            .clamp_existing_to_range(false),
                    ),
                    "Set lighting",
                ));
                ui.end_row();
            }
        });
    claim
}
