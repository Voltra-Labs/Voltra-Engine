//! One frame of every follower: where it aims, how it eases, and the zoom.

use voltra_assets::Textures;
use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;

use super::super::{set_world_position, world_position};
use super::{group, limits, zoom, CameraFollow2D, FollowBounds, FollowZoom};

/// One frame of every follower; `screen` is the screen's aspect and
/// `textures` the render textures, for each camera's own aspect.
pub(in crate::camera_rig) fn update(
    world: &mut World,
    delta: f32,
    screen: f32,
    textures: Option<&Textures>,
) {
    let followers: Vec<Entity> = world
        .query::<CameraFollow2D>()
        .map(|(entity, _)| entity)
        .collect();
    for entity in followers {
        let aspect = crate::camera::aspect(world, entity, screen, textures);
        follow_one(world, entity, delta, aspect);
    }
}

/// What a frame of following reads off the rig — copied out, so the rig
/// is not cloned whole every frame.
struct Rig {
    offset: Vec2,
    smoothing: f32,
    dead_zone: Vec2,
    look_ahead: f32,
    bounds: Option<FollowBounds>,
    zoom: Option<FollowZoom>,
    last_target: Option<Vec2>,
    lead: Vec2,
    members: u64,
}

fn follow_one(world: &mut World, entity: Entity, delta: f32, aspect: f32) {
    let Some((rig, group)) = world.get::<CameraFollow2D>(entity).map(|rig| {
        (
            Rig {
                offset: rig.offset,
                smoothing: rig.smoothing,
                dead_zone: rig.dead_zone.max(Vec2::ZERO),
                look_ahead: rig.look_ahead,
                bounds: rig.bounds,
                zoom: rig.zoom,
                last_target: rig.state.last_target,
                lead: rig.state.lead,
                members: rig.state.members,
            },
            rig.group(),
        )
    }) else {
        return;
    };
    if group.is_empty() {
        return;
    }
    let gathered = group::gather(world, entity, &group);
    let Some(present) = group::say_missing(world, entity, gathered) else {
        return;
    };
    let at = present.centre();
    let here = world_position(world, entity);
    let rotation = crate::camera::placement(world, entity).1;

    let first = rig.last_target.is_none();
    let rate = if first || rig.smoothing <= 0.0 || !rig.smoothing.is_finite() {
        1.0
    } else {
        1.0 - (-rig.smoothing * delta).exp()
    };
    // A target joining or leaving moves the middle at once: not a velocity.
    let regrouped = present.members != rig.members;
    let velocity = match rig.last_target {
        Some(was) if delta > 0.0 && !regrouped => (at - was) / delta,
        _ => Vec2::ZERO,
    };
    let wanted_lead = velocity * rig.look_ahead.max(0.0);
    let lead = if first {
        Vec2::ZERO
    } else {
        rig.lead + (wanted_lead - rig.lead) * rate
    };

    // The camera may sit a dead zone and an offset away from the middle,
    // so the fit makes room for both.
    let slack = rig.dead_zone + rig.offset.abs();
    let fit = zoom::Fit {
        zoom: rig.zoom,
        half: (present.max - present.min) * 0.5 + slack,
        aspect,
        rotation,
        rate,
    };
    let size = zoom::zoom_to(world, entity, fit);
    let half_view = size.map_or(Vec2::ZERO, |size| {
        limits::seen_box(Vec2::new(size * aspect.max(0.0), size), rotation)
    });
    let aim = at + rig.offset + lead;
    let goal = limits::clamp_to(
        limits::dead_zone(here, aim, rig.dead_zone),
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
