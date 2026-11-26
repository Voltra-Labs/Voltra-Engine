//! A joint's kind, what it joins, its anchors and the parts of its kind.

use voltra_core::egui::{self, DragValue, RichText, Ui};
use voltra_scene::{Joint2D, JointKind};

use super::active;
use super::light::optional;

mod rows;

use rows::{degrees, number};

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
            for name in ["revolute", "distance", "weld"] {
                ui.selectable_value(&mut kind, name, name);
            }
        });
    if kind != joint.kind.name() {
        joint.kind = match kind {
            "distance" => JointKind::rod(1.0),
            "weld" => JointKind::Weld {
                reference_angle: 0.0,
            },
            _ => JointKind::default(),
        };
        joint.impulses = Default::default();
        claim = Some("Set joint kind");
    }

    egui::Grid::new("joint").num_columns(2).show(ui, |ui| {
        for (label, anchor, hint) in [
            (
                "anchor on a",
                &mut joint.anchor_a,
                "where on a, in its own space",
            ),
            (
                "anchor on b",
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

        match &mut joint.kind {
            JointKind::Revolute {
                reference_angle,
                limit,
                motor,
            } => {
                claim = claim.or(degrees(ui, "reference", reference_angle));
                ui.label("limit")
                    .on_hover_text("how far a may turn against b");
                if optional(ui, "joint_limit", limit) {
                    claim = claim.or(Some("Toggle joint limit"));
                }
                ui.end_row();
                if let Some(limit) = limit {
                    claim = claim.or(degrees(ui, "lower", &mut limit.lower));
                    claim = claim.or(degrees(ui, "upper", &mut limit.upper));
                }
                ui.label("motor")
                    .on_hover_text("turns a against b at a speed");
                if optional(ui, "joint_motor", motor) {
                    claim = claim.or(Some("Toggle joint motor"));
                }
                ui.end_row();
                if let Some(motor) = motor {
                    claim = claim.or(degrees(ui, "speed /s", &mut motor.speed));
                    claim = claim.or(number(ui, "max torque", &mut motor.max_torque, 0.0..=1e6));
                }
            }
            JointKind::Distance {
                length,
                min,
                max,
                spring,
            } => {
                claim = claim.or(number(ui, "length", length, 0.0..=1e4));
                claim = claim.or(number(ui, "min", min, 0.0..=1e4));
                claim = claim.or(number(ui, "max", max, 0.0..=1e4));
                if *min >= *max {
                    ui.label("");
                    ui.label(RichText::new("held at its length").weak());
                    ui.end_row();
                }
                ui.label("spring").on_hover_text(
                    "pulls towards the length instead of holding it; needs min < max",
                );
                if optional(ui, "joint_spring", spring) {
                    claim = claim.or(Some("Toggle joint spring"));
                }
                ui.end_row();
                if let Some(spring) = spring {
                    claim = claim.or(number(ui, "hertz", &mut spring.hertz, 0.0..=60.0));
                    claim = claim.or(number(ui, "damping", &mut spring.damping_ratio, 0.0..=10.0));
                }
            }
            JointKind::Weld { reference_angle } => {
                claim = claim.or(degrees(ui, "reference", reference_angle));
            }
        }
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
