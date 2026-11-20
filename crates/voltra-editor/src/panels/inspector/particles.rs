//! An emitter's rate, life, motion and look.

use voltra_core::egui::{self, DragValue, RichText, Ui};
use voltra_scene::particles::MAX_PARTICLES;
use voltra_scene::ParticleEmitter;

use super::active;

pub(super) fn show(ui: &mut Ui, emitter: &mut ParticleEmitter) -> Option<&'static str> {
    ui.label(RichText::new("Particles").strong());
    let mut claim = None;
    egui::Grid::new("particles").num_columns(3).show(ui, |ui| {
        ui.label("emitting");
        if ui.checkbox(&mut emitter.emitting, "").changed() {
            claim = claim.or(Some("Toggle emitting"));
        }
        ui.end_row();

        ui.label("rate / s");
        claim = claim.or(drag(ui, &mut emitter.rate, 0.5, 0.0..=10_000.0));
        ui.end_row();

        ui.label("max");
        claim = claim.or(active(
            &ui.add(DragValue::new(&mut emitter.max_particles).range(0..=MAX_PARTICLES)),
            "Edit particles",
        ));
        ui.end_row();

        ui.label("lifetime s");
        claim = claim.or(drag(ui, &mut emitter.lifetime.0, 0.01, 0.0..=60.0));
        claim = claim.or(drag(ui, &mut emitter.lifetime.1, 0.01, 0.0..=60.0));
        ui.end_row();

        ui.label("speed");
        claim = claim.or(drag(ui, &mut emitter.speed.0, 0.05, -100.0..=100.0));
        claim = claim.or(drag(ui, &mut emitter.speed.1, 0.05, -100.0..=100.0));
        ui.end_row();

        ui.label("direction °");
        let mut degrees = emitter.direction.to_degrees();
        if let Some(label) = drag(ui, &mut degrees, 1.0, -360.0..=360.0) {
            emitter.direction = degrees.to_radians();
            claim = claim.or(Some(label));
        }
        ui.end_row();

        ui.label("spread °");
        let mut spread = emitter.spread.to_degrees();
        if let Some(label) = drag(ui, &mut spread, 1.0, 0.0..=360.0) {
            emitter.spread = spread.to_radians();
            claim = claim.or(Some(label));
        }
        ui.end_row();

        ui.label("acceleration");
        claim = claim.or(drag(ui, &mut emitter.acceleration.x, 0.05, -100.0..=100.0));
        claim = claim.or(drag(ui, &mut emitter.acceleration.y, 0.05, -100.0..=100.0));
        ui.end_row();

        ui.label("size start / end");
        claim = claim.or(drag(ui, &mut emitter.size.0, 0.01, 0.0..=100.0));
        claim = claim.or(drag(ui, &mut emitter.size.1, 0.01, 0.0..=100.0));
        ui.end_row();

        ui.label("colour start / end");
        for colour in [&mut emitter.color_start, &mut emitter.color_end] {
            let popup = ui.auto_id_with("popup");
            let response = ui.color_edit_button_rgba_unmultiplied(colour);
            if egui::Popup::is_id_open(ui.ctx(), popup) || response.changed() {
                claim = claim.or(Some("Edit particles"));
            }
        }
        ui.end_row();

        ui.label("world space");
        if ui.checkbox(&mut emitter.world_space, "").changed() {
            claim = claim.or(Some("Edit particles"));
        }
        ui.end_row();

        ui.label("sort order");
        claim = claim.or(active(
            &ui.add(DragValue::new(&mut emitter.sort_order)),
            "Set sort order",
        ));
        ui.end_row();
    });
    claim
}

fn drag(
    ui: &mut Ui,
    value: &mut f32,
    speed: f64,
    range: std::ops::RangeInclusive<f32>,
) -> Option<&'static str> {
    active(
        &ui.add(DragValue::new(value).speed(speed).range(range)),
        "Edit particles",
    )
}
