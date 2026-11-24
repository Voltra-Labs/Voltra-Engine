//! What a particle looks like over its life: its texture, size and size
//! curve, speed curve, colours or gradient, alpha curve, and spin.

use voltra_assets::{AssetPath, Textures};
use voltra_core::egui::{self, DragValue, Ui};
use voltra_render::wgpu;
use voltra_scene::{Curve, Gradient, ParticleEmitter};

use super::super::active;
use super::super::path_field::{self, PathEdit};
use super::{curve_edit, drag, gradient_edit};

pub(super) fn show(
    ui: &mut Ui,
    emitter: &mut ParticleEmitter,
    textures: &mut Textures,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> Option<&'static str> {
    let mut claim = texture(ui, emitter, textures, device, queue);
    egui::Grid::new("particle_size")
        .num_columns(3)
        .show(ui, |ui| {
            ui.label("size start / end");
            claim = claim.or(drag(ui, &mut emitter.size.0, 0.01, 0.0..=100.0));
            claim = claim.or(drag(ui, &mut emitter.size.1, 0.01, 0.0..=100.0));
            ui.end_row();
        });
    claim = claim.or(optional_curve(
        ui,
        "size over life",
        "multiplies the size, birth to death",
        &mut emitter.size_curve,
        2.0,
    ));
    claim = claim.or(optional_curve(
        ui,
        "speed over life",
        "multiplies how far a particle goes: 0 holds it still",
        &mut emitter.speed_curve,
        2.0,
    ));

    let mut graded = emitter.gradient.is_some();
    if ui
        .checkbox(&mut graded, "colour gradient")
        .on_hover_text("in place of the start and end colours")
        .changed()
    {
        emitter.gradient = graded.then(|| Gradient::linear(emitter.color_start, emitter.color_end));
        claim = claim.or(Some("Edit gradient"));
    }
    match &mut emitter.gradient {
        Some(gradient) => {
            claim = claim.or(gradient_edit::show(ui, "particle_gradient", gradient));
        }
        None => {
            ui.horizontal(|ui| {
                ui.label("colour start / end");
                for colour in [&mut emitter.color_start, &mut emitter.color_end] {
                    let popup = ui.auto_id_with("popup");
                    let response = ui.color_edit_button_rgba_unmultiplied(colour);
                    if egui::Popup::is_id_open(ui.ctx(), popup) || response.changed() {
                        claim = claim.or(Some("Edit particles"));
                    }
                }
            });
        }
    }
    claim = claim.or(optional_curve(
        ui,
        "alpha over life",
        "multiplies the colour's alpha",
        &mut emitter.alpha_curve,
        1.0,
    ));

    egui::Grid::new("particle_spin")
        .num_columns(3)
        .show(ui, |ui| {
            ui.label("rotation °");
            claim = claim.or(degree_range(ui, &mut emitter.rotation));
            ui.end_row();
            ui.label("spin ° / s");
            claim = claim.or(degree_range(ui, &mut emitter.angular_velocity));
            ui.end_row();
        });
    claim
}

/// The image every particle draws, by path.
fn texture(
    ui: &mut Ui,
    emitter: &mut ParticleEmitter,
    textures: &mut Textures,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> Option<&'static str> {
    ui.label("texture");
    let committed = emitter
        .texture
        .as_ref()
        .map(AssetPath::as_str)
        .unwrap_or("")
        .to_owned();
    let path = match path_field::show(ui, "particle_texture", "path/to/texture.png", &committed)? {
        PathEdit::Cleared => None,
        PathEdit::Set(path) => Some(path),
    };
    emitter.set_texture(path, textures, device, queue);
    Some("Set texture")
}

/// A checkbox turning a curve on — as the identity, so nothing jumps — and
/// the curve's editor while it is.
fn optional_curve(
    ui: &mut Ui,
    label: &'static str,
    hint: &str,
    curve: &mut Option<Curve>,
    top: f32,
) -> Option<&'static str> {
    let mut on = curve.is_some();
    let mut claim = None;
    if ui.checkbox(&mut on, label).on_hover_text(hint).changed() {
        *curve = on.then(|| Curve::linear(1.0, 1.0));
        claim = Some(curve_edit::EDIT);
    }
    if let Some(curve) = curve {
        claim = claim.or(curve_edit::show(ui, label, curve, top));
    }
    claim
}

/// A range of angles kept in radians, dragged in degrees.
fn degree_range(ui: &mut Ui, range: &mut (f32, f32)) -> Option<&'static str> {
    let mut claim = None;
    for end in [&mut range.0, &mut range.1] {
        let mut degrees = end.to_degrees();
        let response = ui.add(DragValue::new(&mut degrees).speed(1.0));
        if let Some(label) = active(&response, "Edit particles") {
            *end = degrees.to_radians();
            claim = Some(label);
        }
    }
    claim
}
