//! A joint's kind, what it joins, its anchors, the parts of its kind and
//! what breaks it.

use voltra_core::egui::{self, DragValue, RichText, Ui};
use voltra_scene::{Joint2D, JointKind};

use super::active;

mod kinds;
mod rows;

use super::mixed;
use rows::threshold;
use voltra_scene::format::serial_tree::Step;

/// What the joint's two ends are called, worked out before the joint is
/// borrowed out of the world.
pub(super) struct Ends {
    pub a: String,
    pub b: String,
}

pub(super) fn show(ui: &mut Ui, joint: &mut Joint2D, ends: &Ends) -> Option<&'static str> {
    ui.label(RichText::new("Joint").strong())
        .on_hover_text("holds two bodies together, or one to the world, as the physics steps");
    ui.label(RichText::new(format!("joins {} to {}", ends.a, ends.b)).weak());
    let mut claim = None;

    let mut kind = joint.kind.name();
    egui::ComboBox::from_id_salt("joint_kind")
        .selected_text(kind)
        .show_ui(ui, |ui| {
            for name in JointKind::NAMES {
                if ui.selectable_value(&mut kind, name, name).clicked() {
                    mixed::touch(ui, "Joint2D", vec![Step::Field("kind")]);
                }
            }
        });
    if kind != joint.kind.name() {
        joint.kind = JointKind::named(kind);
        joint.impulses = Default::default();
        claim = Some("Set joint kind");
    }
    let target = matches!(joint.kind, JointKind::Mouse { .. });

    egui::Grid::new("joint").num_columns(2).show(ui, |ui| {
        for (label, anchor, hint) in [
            (
                "anchor on a",
                &mut joint.anchor_a,
                "where on a, in its own space",
            ),
            (
                if target { "target" } else { "anchor on b" },
                &mut joint.anchor_b,
                "where on b, in its own space — or the point in the world",
            ),
        ] {
            ui.label(label).on_hover_text(hint);
            ui.horizontal(|ui| {
                for value in [&mut anchor.x, &mut anchor.y] {
                    claim = claim.or(active(
                        &ui.add(DragValue::new(value).speed(0.01)),
                        "Move joint anchor",
                    ));
                }
            });
            ui.end_row();
        }
        ui.label("collide");
        if ui
            .checkbox(&mut joint.collide_connected, "")
            .on_hover_text("whether the two bodies still collide with each other")
            .changed()
        {
            claim = claim.or(Some("Toggle joint collision"));
        }
        ui.end_row();

        claim = claim.or(kinds::show(ui, &mut joint.kind));
        claim = claim.or(threshold(
            ui,
            "break force",
            "past this force, in newtons, the joint breaks: it is removed and the game told",
            &mut joint.break_force,
            1000.0,
        ));
        claim = claim.or(threshold(
            ui,
            "break torque",
            "past this torque, in newton-metres, the joint breaks",
            &mut joint.break_torque,
            1000.0,
        ));
    });
    claim
}

/// What a joint on `entity` joins, as the inspector names them.
pub(super) fn ends(world: &voltra_ecs::World, entity: voltra_ecs::Entity) -> Ends {
    let Some(j) = world.get::<voltra_scene::Joint2D>(entity) else {
        return Ends {
            a: String::new(),
            b: String::new(),
        };
    };
    let named = |id: voltra_scene::SceneId| {
        voltra_scene::format::entity_with_id(world, id).map_or_else(
            || "a body that is gone".to_owned(),
            |e| super::name::of(world, e),
        )
    };
    Ends {
        a: j.a.map_or_else(|| "this entity".to_owned(), named),
        b: j.b.map_or_else(|| "the world".to_owned(), named),
    }
}
