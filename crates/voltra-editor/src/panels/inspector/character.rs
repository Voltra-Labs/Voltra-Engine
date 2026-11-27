//! The character body: how it slides, steps and snaps, and what it touches.

use voltra_core::egui::{self, DragValue, RichText, Ui};
use voltra_ecs::{Entity, World};
use voltra_scene::{CharacterBody2D, Collider2D, RigidBody2D};

use super::active;

/// The character's rows, or the button that makes a collider one.
///
/// Offered for a collider without a rigid body: a character is moved by
/// the game's `move_and_slide`, and a body the step also moved would be
/// moved twice.
pub(super) fn show(ui: &mut Ui, world: &mut World, entity: Entity) -> Option<&'static str> {
    if world.get::<CharacterBody2D>(entity).is_none() {
        if world.get::<Collider2D>(entity).is_none() || world.get::<RigidBody2D>(entity).is_some() {
            return None;
        }
        ui.separator();
        return ui
            .button("Make character")
            .on_hover_text(
                "moved by the game's move_and_slide: stops at walls, walks slopes and steps",
            )
            .clicked()
            .then(|| {
                world.insert(entity, CharacterBody2D::default());
                "Make character"
            });
    }
    let doubled = world.get::<RigidBody2D>(entity).is_some();
    let body = world
        .get_mut::<CharacterBody2D>(entity)
        .expect("the component is there: it was just tested for");

    ui.separator();
    ui.label(RichText::new("Character").strong())
        .on_hover_text("a velocity in from the game's tick, a move that stops and slides");
    if doubled {
        ui.label(
            RichText::new("has a rigid body too: the physics step will also move it")
                .color(ui.visuals().warn_fg_color),
        );
    }
    let mut claim = None;
    egui::Grid::new("character").num_columns(2).show(ui, |ui| {
        ui.label("max floor")
            .on_hover_text("the steepest slope it stands on; steeper is a wall");
        let mut degrees = body.floor_max_angle.to_degrees();
        let response = ui.add(
            DragValue::new(&mut degrees)
                .speed(0.5)
                .range(0.0..=89.0)
                .suffix("°"),
        );
        if response.changed() {
            body.floor_max_angle = degrees.to_radians();
        }
        claim = claim.or(active(&response, "Edit character"));
        ui.end_row();

        for (label, value, hint, max) in [
            (
                "step",
                &mut body.step_height,
                "the highest ledge it walks onto",
                10.0,
            ),
            (
                "snap",
                &mut body.snap,
                "how far down it reaches to stay on a floor",
                10.0,
            ),
            (
                "skin",
                &mut body.skin,
                "the gap kept from what it touches",
                1.0,
            ),
        ] {
            ui.label(label).on_hover_text(hint);
            let response = ui.add(DragValue::new(value).speed(0.005).range(0.0..=max));
            claim = claim.or(active(&response, "Edit character"));
            ui.end_row();
        }

        ui.label("slides")
            .on_hover_text("the most surfaces one move slides along");
        let response = ui.add(DragValue::new(&mut body.max_slides).range(1..=16));
        claim = claim.or(active(&response, "Edit character"));
        ui.end_row();

        ui.label("push").on_hover_text(
            "the most force it shoves a dynamic body it walks into with; 0 pushes nothing",
        );
        let response = ui.add(
            DragValue::new(&mut body.push_force)
                .speed(0.5)
                .range(0.0..=f32::MAX)
                .suffix(" N"),
        );
        claim = claim.or(active(&response, "Edit character"));
        ui.end_row();

        for (value, label, hint) in [
            (
                &mut body.stop_on_slope,
                "stop on slope",
                "standing on a slope holds still instead of sliding down it",
            ),
            (
                &mut body.constant_speed_on_slopes,
                "constant speed on slopes",
                "up or down a slope it covers the same ground across as on the flat",
            ),
            (
                &mut body.carry_rotation,
                "carry rotation",
                "a turning floor turns it too, not only carries it round",
            ),
        ] {
            ui.label("");
            if ui.checkbox(value, label).on_hover_text(hint).changed() {
                claim = claim.or(Some("Edit character"));
            }
            ui.end_row();
        }
    });
    let contact = body.contact;
    let touching: Vec<&str> = [
        (contact.on_floor, "floor"),
        (contact.on_wall, "wall"),
        (contact.on_ceiling, "ceiling"),
    ]
    .into_iter()
    .filter_map(|(on, name)| on.then_some(name))
    .collect();
    if !touching.is_empty() {
        ui.label(RichText::new(format!("on {}", touching.join(", "))).weak());
    }
    claim
}
