//! An emitter's preview, presets, rate, life, motion, shape and look.
//!
//! The sections a particle editor has in Unity and Godot, in the order an
//! effect is built: start from a preset, say how many and how they move,
//! where they are born, then what they look like over their life. Each
//! section is a file under `particles/`; this one lays them out and holds
//! the plain fields.

mod curve_edit;
mod emission;
mod flipbook_edit;
mod gradient_edit;
mod looks;
mod preview;

pub(super) use curve_edit::show as curve;

use voltra_assets::{Atlases, Textures};
use voltra_core::egui::{self, DragValue, RichText, Ui};
use voltra_render::particles::MAX_GPU_PARTICLES;
use voltra_render::wgpu;
use voltra_scene::particles::{ParticleCollision, Simulation, MAX_PARTICLES};
use voltra_scene::ParticleEmitter;

use super::active;
use super::collision::bits;
use super::mixed;
use voltra_scene::format::serial_tree::Step;

/// The stores the emitter's texture and flipbook resolve against.
pub(super) struct Stores<'a> {
    pub textures: &'a mut Textures,
    pub atlases: &'a mut Atlases,
    pub device: &'a wgpu::Device,
    pub queue: &'a wgpu::Queue,
}

pub(super) fn show(
    ui: &mut Ui,
    emitter: &mut ParticleEmitter,
    stores: Stores<'_>,
) -> Option<&'static str> {
    ui.label(RichText::new("Particles").strong());
    let mut claim = preview::show(ui, emitter);
    egui::Grid::new("particles").num_columns(3).show(ui, |ui| {
        claim = claim.or(motion(ui, emitter));
    });
    ui.separator();
    claim = claim.or(emission::show(ui, emitter));
    ui.separator();
    claim = claim.or(looks::show(
        ui,
        emitter,
        stores.textures,
        stores.device,
        stores.queue,
    ));
    claim = claim.or(flipbook_edit::show(ui, emitter, stores.atlases));
    ui.separator();
    egui::Grid::new("particles_drawing")
        .num_columns(3)
        .show(ui, |ui| {
            claim = claim.or(drawing(ui, emitter));
        });
    claim.or(super::light_layers::show(
        ui,
        "particle_light_layers",
        "light layers",
        "its particles are lit only by lights sharing one of these",
        &mut emitter.light_layers,
    ))
}

/// How many, how long, which way and how fast.
fn motion(ui: &mut Ui, emitter: &mut ParticleEmitter) -> Option<&'static str> {
    let mut claim = None;
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
        &ui.add(DragValue::new(&mut emitter.max_particles).range(
            0..=match emitter.simulation {
                Simulation::Cpu => MAX_PARTICLES,
                Simulation::Gpu => MAX_GPU_PARTICLES,
            },
        )),
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
    claim = claim.or(degrees(ui, &mut emitter.direction, -360.0..=360.0));
    ui.end_row();

    ui.label("spread °");
    claim = claim.or(degrees(ui, &mut emitter.spread, 0.0..=360.0));
    ui.end_row();

    ui.label("acceleration");
    claim = claim.or(drag(ui, &mut emitter.acceleration.x, 0.05, -100.0..=100.0));
    claim = claim.or(drag(ui, &mut emitter.acceleration.y, 0.05, -100.0..=100.0));
    ui.end_row();
    claim
}

/// Where and how it is simulated, what it hits, and where it draws.
fn drawing(ui: &mut Ui, emitter: &mut ParticleEmitter) -> Option<&'static str> {
    let mut claim = None;
    ui.label("simulation");
    let before = emitter.simulation;
    egui::ComboBox::from_id_salt("particle_simulation")
        .selected_text(name(before))
        .show_ui(ui, |ui| {
            for option in [Simulation::Cpu, Simulation::Gpu] {
                if ui
                    .selectable_value(&mut emitter.simulation, option, name(option))
                    .clicked()
                {
                    mixed::touch(ui, "ParticleEmitter", vec![Step::Field("simulation")]);
                }
            }
        })
        .response
        .on_hover_text("GPU: tens of thousands for the cost of a few; falls back to the CPU on an adapter without compute shaders");
    if emitter.simulation != before {
        claim = claim.or(Some("Edit particles"));
    }
    ui.end_row();

    ui.label("world space");
    if ui.checkbox(&mut emitter.world_space, "").changed() {
        claim = claim.or(Some("Edit particles"));
    }
    ui.end_row();

    ui.label("collides");
    let mut collides = emitter.collision.is_some();
    if ui
        .checkbox(&mut collides, "")
        .on_hover_text("Hit the scene's colliders: boxes, circles and solid tiles")
        .changed()
    {
        emitter.collision = collides.then(ParticleCollision::default);
        claim = claim.or(Some("Edit particles"));
    }
    ui.end_row();

    if let Some(collision) = &mut emitter.collision {
        ui.label("bounce / friction");
        claim = claim.or(drag(ui, &mut collision.bounce, 0.01, 0.0..=1.0));
        claim = claim.or(drag(ui, &mut collision.friction, 0.01, 0.0..=1.0));
        ui.end_row();

        ui.label("lifetime loss");
        claim = claim.or(drag(ui, &mut collision.lifetime_loss, 0.01, 0.0..=1.0));
        ui.end_row();

        ui.label("hits layers");
        if bits(ui, "particle_mask", &mut collision.mask) {
            claim = claim.or(Some("Edit particles"));
        }
        ui.end_row();
    }

    ui.label("sort order");
    claim = claim.or(active(
        &ui.add(DragValue::new(&mut emitter.sort_order)),
        "Set sort order",
    ));
    ui.end_row();
    claim
}

/// A field of `ParticleEmitter`, dragged.
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

/// An angle kept in radians, dragged in degrees.
fn degrees(
    ui: &mut Ui,
    radians: &mut f32,
    range: std::ops::RangeInclusive<f32>,
) -> Option<&'static str> {
    let mut degrees = radians.to_degrees();
    let label = drag(ui, &mut degrees, 1.0, range)?;
    *radians = degrees.to_radians();
    Some(label)
}

fn name(simulation: Simulation) -> &'static str {
    match simulation {
        Simulation::Cpu => "CPU",
        Simulation::Gpu => "GPU",
    }
}
