//! One frame of every follower: where it aims, how it eases, and the zoom.

use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;

use super::super::{set_world_position, world_position};
use super::{group, limits, zoom, CameraFollow2D};

/// One frame of every follower.
pub(in crate::camera_rig) fn update(world: &mut World, delta: f32, aspect: f32) {
    let followers: Vec<Entity> = world
        .query::<CameraFollow2D>()
        .map(|(entity, _)| entity)
        .collect();
    for entity in followers {
        follow_one(world, entity, delta, aspect);
    }
}

fn follow_one(world: &mut World, entity: Entity, delta: f32, aspect: f32) {
    let Some(rig) = world.get::<CameraFollow2D>(entity).cloned() else {
        return;
    };
    let group = rig.group();
    if group.is_empty() {
        return;
    }
    let (present, missing) = group::gather(world, entity, &group);
    group::say_missing(world, entity, missing, present.is_some());
    let Some(present) = present else {
        return;
    };
    let at = present.centre();
    let here = world_position(world, entity);
    let rotation = crate::camera::placement(world, entity).1;

    let first = rig.state.last_target.is_none();
    let rate = if first || rig.smoothing <= 0.0 || !rig.smoothing.is_finite() {
        1.0
    } else {
        1.0 - (-rig.smoothing * delta).exp()
    };
    // A target joining or leaving moves the middle at once: not a velocity.
    let regrouped = present.members != rig.state.members;
    let velocity = match rig.state.last_target {
        Some(was) if delta > 0.0 && !regrouped => (at - was) / delta,
        _ => Vec2::ZERO,
    };
    let wanted_lead = velocity * rig.look_ahead.max(0.0);
    let lead = if first {
        Vec2::ZERO
    } else {
        rig.state.lead + (wanted_lead - rig.state.lead) * rate
    };

    let size = zoom::zoom_to(world, entity, &rig, &present, aspect, rotation, rate);
    let half_view = size.map_or(Vec2::ZERO, |size| {
        limits::seen_box(Vec2::new(size * aspect.max(0.0), size), rotation)
    });
    let aim = at + rig.offset + lead;
    let goal = limits::clamp_to(
        limits::dead_zone(here, aim, rig.dead_zone.max(Vec2::ZERO)),
        rig.bounds,
        half_view,
    );
    let next = here + (goal - here) * rate;
    if next.is_finite() {
        set_world_position(world, entity, next);
    }
    if let Some(rig) = world.get_mut::<CameraFollow2D>(entity) {
        rig.state.last_target = Some(at);
        rig.state.lead = if lead.is_finite() { lead } else { Vec2::ZERO };
        rig.state.members = present.members;
    }
}
