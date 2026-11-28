//! A camera's grade: exposure, contrast, saturation, vignette, bloom and
//! tint — the same fields on a 2D camera and a 3D one.

use voltra_core::egui::{self, DragValue, RichText, Ui};
use voltra_scene::PostProcess;

use super::active;

/// `post`'s fields in a grid named `id`, and Reset once it is not neutral.
pub(super) fn show(ui: &mut Ui, id: &str, post: &mut PostProcess) -> Option<&'static str> {
    let mut claim = None;
    ui.label(RichText::new("Post-processing").strong())
        .on_hover_text("how the frame through this camera is graded; the HUD is not");
    egui::Grid::new(id).num_columns(2).show(ui, |ui| {
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
        let response = crate::panels::colour_edit::rgb(ui, &mut post.tint);
        if egui::Popup::is_id_open(ui.ctx(), popup) || response.changed() {
            claim = claim.or(Some("Set post-processing"));
        }
        ui.end_row();
        if !post.is_neutral() && ui.small_button("Reset").clicked() {
            *post = PostProcess::default();
            claim = claim.or(Some("Set post-processing"));
        }
    });
    claim
}
