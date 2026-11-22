//! A camera's rig: what it follows and how, and how it shakes.

use voltra_core::egui::{self, DragValue, RichText, Ui};
use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;
use voltra_scene::format::entity_with_id;
use voltra_scene::{Camera, CameraFollow2D, CameraShake, FollowBounds, SceneId};

use super::active;

/// The follow and shake sections, or the buttons that add them to a
/// camera. A follower that is not a camera — a light trailing the player —
/// shows its section too.
pub(super) fn show(ui: &mut Ui, world: &mut World, entity: Entity) -> Option<&'static str> {
    let camera = world.get::<Camera>(entity).is_some();
    let following = world.get::<CameraFollow2D>(entity).is_some();
    let shaking = world.get::<CameraShake>(entity).is_some();
    if !camera && !following && !shaking {
        return None;
    }
    ui.separator();
    let mut claim = None;
    if following {
        claim = follow(ui, world, entity);
    }
    if shaking {
        claim = claim.or(shake(ui, world, entity));
    }
    if camera && !(following && shaking) {
        ui.horizontal(|ui| {
            if !following && ui.button("Add follow").clicked() {
                world.insert(entity, CameraFollow2D::default());
                claim = claim.or(Some("Add camera follow"));
            }
            if !shaking && ui.button("Add shake").clicked() {
                world.insert(entity, CameraShake::default());
                claim = claim.or(Some("Add camera shake"));
            }
        });
    }
    claim
}

fn follow(ui: &mut Ui, world: &mut World, entity: Entity) -> Option<&'static str> {
    ui.label(RichText::new("Follow").strong())
        .on_hover_text("keeps this entity on a target while the game runs");
    let mut claim = target(ui, world, entity);
    let rig = world
        .get_mut::<CameraFollow2D>(entity)
        .expect("tested by the caller");
    egui::Grid::new("camera_follow")
        .num_columns(2)
        .show(ui, |ui| {
            claim = claim.or(pair(
                ui,
                "offset",
                &mut rig.offset,
                "where it sits from the target",
            ));
            claim = claim.or(pair(
                ui,
                "dead zone",
                &mut rig.dead_zone,
                "half the box the target moves in freely",
            ));
            rig.dead_zone = rig.dead_zone.max(Vec2::ZERO);
            claim = claim.or(number(
                ui,
                "smoothing",
                &mut rig.smoothing,
                "how fast it closes in; 0 snaps",
            ));
            claim = claim.or(number(
                ui,
                "look ahead",
                &mut rig.look_ahead,
                "seconds of the target's velocity it leads by",
            ));
            ui.label("bounds")
                .on_hover_text("what it may show; outside them it stops");
            let mut bounded = rig.bounds.is_some();
            if ui.checkbox(&mut bounded, "").changed() {
                rig.bounds = bounded.then_some(FollowBounds {
                    min: Vec2::splat(-10.0),
                    max: Vec2::splat(10.0),
                });
                claim = claim.or(Some("Toggle follow bounds"));
            }
            ui.end_row();
            if let Some(bounds) = rig.bounds.as_mut() {
                claim = claim.or(pair(ui, "min", &mut bounds.min, "the bottom-left corner"));
                claim = claim.or(pair(ui, "max", &mut bounds.max, "the top-right corner"));
            }
        });
    claim
}

/// The picker for what it follows: any entity with an id but itself.
fn target(ui: &mut Ui, world: &mut World, entity: Entity) -> Option<&'static str> {
    let current = world.get::<CameraFollow2D>(entity).and_then(|r| r.target);
    let shown = match current {
        None => "nothing".to_owned(),
        Some(id) => entity_with_id(world, id).map_or_else(
            || "an entity that is gone".to_owned(),
            |e| super::name::of(world, e),
        ),
    };
    let mut choices: Vec<(SceneId, String)> = world
        .query::<SceneId>()
        .filter(|(other, _)| *other != entity)
        .map(|(other, id)| (*id, super::name::of(world, other)))
        .collect();
    choices.sort_by(|a, b| a.1.cmp(&b.1));
    let mut picked = current;
    ui.horizontal(|ui| {
        ui.label("target");
        egui::ComboBox::from_id_salt("follow_target")
            .selected_text(shown)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut picked, None, "nothing");
                for (id, name) in &choices {
                    ui.selectable_value(&mut picked, Some(*id), name);
                }
            });
    });
    if picked == current {
        return None;
    }
    let rig = world.get_mut::<CameraFollow2D>(entity)?;
    rig.target = picked;
    rig.snap();
    Some("Set follow target")
}

fn shake(ui: &mut Ui, world: &mut World, entity: Entity) -> Option<&'static str> {
    ui.label(RichText::new("Shake").strong())
        .on_hover_text("shaken by trauma a game adds: add_trauma");
    let rig = world
        .get_mut::<CameraShake>(entity)
        .expect("tested by the caller");
    let mut claim = None;
    egui::Grid::new("camera_shake")
        .num_columns(2)
        .show(ui, |ui| {
            claim = claim.or(pair(
                ui,
                "max offset",
                &mut rig.max_offset,
                "how far it moves at full trauma",
            ));
            claim = claim.or(number(
                ui,
                "frequency",
                &mut rig.frequency,
                "roughly shakes a second",
            ));
            claim = claim.or(number(
                ui,
                "decay",
                &mut rig.decay,
                "trauma lost per second",
            ));
        });
    // Seen only while the game runs, which is when trauma does anything.
    if ui
        .button("Test shake")
        .on_hover_text("adds half a trauma; shakes while playing")
        .clicked()
    {
        rig.add_trauma(0.5);
    }
    claim
}

fn number(ui: &mut Ui, label: &str, value: &mut f32, hint: &str) -> Option<&'static str> {
    ui.label(label).on_hover_text(hint);
    let response = ui.add(DragValue::new(value).speed(0.01).range(0.0..=1000.0));
    ui.end_row();
    active(&response, "Edit camera rig")
}

fn pair(ui: &mut Ui, label: &str, value: &mut Vec2, hint: &str) -> Option<&'static str> {
    ui.label(label).on_hover_text(hint);
    let mut claim = None;
    ui.horizontal(|ui| {
        for axis in [&mut value.x, &mut value.y] {
            claim = claim.or(active(
                &ui.add(DragValue::new(axis).speed(0.01)),
                "Edit camera rig",
            ));
        }
    });
    ui.end_row();
    claim
}
