//! The follow section: what it follows and how.

use voltra_core::egui::{self, RichText, Ui};
use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;
use voltra_scene::format::entity_with_id;
use voltra_scene::{CameraFollow2D, FollowBounds, FollowZoom, SceneId};

use super::{number, pair};

pub(super) fn follow(ui: &mut Ui, world: &mut World, entity: Entity) -> Option<&'static str> {
    ui.label(RichText::new("Follow").strong())
        .on_hover_text("keeps this entity on a target while the game runs");
    let mut claim = target(ui, world, entity);
    claim = claim.or(group(ui, world, entity));
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
            ui.label("zoom to fit")
                .on_hover_text("a camera's size eased so every target fits, with a margin");
            let mut zooming = rig.zoom.is_some();
            if ui.checkbox(&mut zooming, "").changed() {
                rig.zoom = zooming.then(FollowZoom::default);
                claim = claim.or(Some("Toggle follow zoom"));
            }
            ui.end_row();
            if let Some(zoom) = rig.zoom.as_mut() {
                claim = claim.or(pair(
                    ui,
                    "margin",
                    &mut zoom.margin,
                    "room kept around the targets on each side",
                ));
                zoom.margin = zoom.margin.max(Vec2::ZERO);
                claim = claim.or(number(
                    ui,
                    "min size",
                    &mut zoom.min_size,
                    "the closest it zooms in: the least camera size",
                ));
                claim = claim.or(number(
                    ui,
                    "max size",
                    &mut zoom.max_size,
                    "the furthest it zooms out: the greatest camera size",
                ));
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

/// The rest of the group it frames with its target: each listed with a
/// button that takes it out, and a picker that adds one.
fn group(ui: &mut Ui, world: &mut World, entity: Entity) -> Option<&'static str> {
    let rig = world.get::<CameraFollow2D>(entity)?;
    let listed = rig.targets.clone();
    let group = rig.group();
    let mut removed = None;
    let mut added = None;
    ui.horizontal_wrapped(|ui| {
        ui.label("also frames")
            .on_hover_text("more targets: it frames the box around all of them");
        for (i, id) in listed.iter().enumerate() {
            let name = entity_with_id(world, *id).map_or_else(
                || "an entity that is gone".to_owned(),
                |e| super::super::name::of(world, e),
            );
            if ui
                .small_button(format!("{name} ×"))
                .on_hover_text("stop framing it")
                .clicked()
            {
                removed = Some(i);
            }
        }
        let mut choices: Vec<(SceneId, String)> = world
            .query::<SceneId>()
            .filter(|(other, id)| *other != entity && !group.contains(id))
            .map(|(other, id)| (*id, super::super::name::of(world, other)))
            .collect();
        choices.sort_by(|a, b| a.1.cmp(&b.1));
        egui::ComboBox::from_id_salt("follow_group_add")
            .selected_text("add…")
            .show_ui(ui, |ui| {
                for (id, name) in &choices {
                    if ui.selectable_label(false, name).clicked() {
                        added = Some(*id);
                    }
                }
            });
    });
    let rig = world.get_mut::<CameraFollow2D>(entity)?;
    if let Some(i) = removed {
        rig.targets.remove(i);
        return Some("Remove follow target");
    }
    if let Some(id) = added {
        rig.targets.push(id);
        return Some("Add follow target");
    }
    None
}
