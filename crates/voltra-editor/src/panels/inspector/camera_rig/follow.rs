//! The follow section: what it follows and how.

use voltra_core::egui::{self, RichText, Ui};
use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;
use voltra_scene::format::entity_with_id;
use voltra_scene::{CameraFollow2D, FollowBounds, SceneId};

use super::{number, pair};

pub(super) fn follow(ui: &mut Ui, world: &mut World, entity: Entity) -> Option<&'static str> {
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
            |e| super::super::name::of(world, e),
        ),
    };
    let mut choices: Vec<(SceneId, String)> = world
        .query::<SceneId>()
        .filter(|(other, _)| *other != entity)
        .map(|(other, id)| (*id, super::super::name::of(world, other)))
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
