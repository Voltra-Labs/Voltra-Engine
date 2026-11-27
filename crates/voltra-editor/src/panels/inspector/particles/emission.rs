//! Where round the emitter its particles are born.

use voltra_core::egui::{self, DragValue, Ui};
use voltra_scene::particles::EmitShape;
use voltra_scene::ParticleEmitter;

use super::super::active;
use super::super::mixed;
use voltra_scene::format::serial_tree::Step;

const EDIT: &str = "Edit emission shape";

pub(super) fn show(ui: &mut Ui, emitter: &mut ParticleEmitter) -> Option<&'static str> {
    let mut claim = None;
    ui.horizontal(|ui| {
        ui.label("emit from");
        egui::ComboBox::from_id_salt("particle_shape")
            .selected_text(emitter.shape.label())
            .show_ui(ui, |ui| {
                for shape in EmitShape::ALL {
                    let current = std::mem::discriminant(&emitter.shape);
                    let picked = ui
                        .selectable_label(current == std::mem::discriminant(&shape), shape.label())
                        .clicked();
                    if picked {
                        mixed::touch_variant(ui, "ParticleEmitter", vec![Step::Field("shape")]);
                    }
                    if picked && current != std::mem::discriminant(&shape) {
                        emitter.shape = shape;
                        claim = Some(EDIT);
                    }
                }
            })
            .response
            .on_hover_text("the collider outline is the entity's own collider, as it sits");
    });
    let mut size = |ui: &mut Ui, label: &str, value: &mut f32| {
        ui.label(label);
        claim = claim.or(active(
            &ui.add(DragValue::new(value).speed(0.01).range(0.0..=1000.0)),
            EDIT,
        ));
    };
    ui.horizontal(|ui| match &mut emitter.shape {
        EmitShape::Point | EmitShape::Outline => {}
        EmitShape::Circle { radius } => size(ui, "radius", radius),
        EmitShape::Ring { inner, outer } => {
            size(ui, "inner", inner);
            size(ui, "outer", outer);
        }
        EmitShape::Box { half_extents } => {
            size(ui, "half width", &mut half_extents.x);
            size(ui, "half height", &mut half_extents.y);
        }
        EmitShape::Edge { half_length } => size(ui, "half length", half_length),
    });
    claim
}
