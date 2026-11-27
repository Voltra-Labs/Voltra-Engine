//! The selected follower's dead zone, bounds and group, drawn in the scene
//! view.
//!
//! A `CameraFollow2D`'s numbers are boxes in the world, and a box typed as
//! two pairs of numbers is a guess until it is seen over the level. Godot
//! draws a selected `Camera2D`'s drag margins and limits; Cinemachine draws
//! its dead zone over the Game view. This draws them for the selected
//! follower only, as the camera's rectangle is.

use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;
use voltra_render::LineBatch;
use voltra_scene::format::entity_with_id;
use voltra_scene::hierarchy::world_matrix;
use voltra_scene::CameraFollow2D;

/// The dead zone: a rose no gizmo axis, overlay or the camera's amber uses.
const DEAD_ZONE: [f32; 4] = [1.0, 0.55, 0.75, 0.9];

/// The bounds: the same rose, faded — the outer of the follower's limits.
const BOUNDS: [f32; 4] = [1.0, 0.55, 0.75, 0.4];

/// The box around the group it frames, margin and all.
const GROUP: [f32; 4] = [0.55, 0.95, 0.75, 0.7];

/// Outline thickness, in pixels. Matches the other overlays.
const WIDTH: f32 = 1.5;

/// Pushes `entity`'s follow boxes into `lines`, if it follows: the dead
/// zone where its target moves freely (around where it rests, less its
/// offset), its bounds, and — framing more than one target — the box around
/// those in the scene, with the zoom's margin.
pub fn draw(world: &World, entity: Entity, lines: &mut LineBatch) {
    let Some(rig) = world.get::<CameraFollow2D>(entity) else {
        return;
    };
    // Where it rests: a shake moves the view, never the transform.
    let here = world_matrix(world, entity).transform_point2(Vec2::ZERO);
    let half = rig.dead_zone.max(Vec2::ZERO);
    if half != Vec2::ZERO {
        let centre = here - rig.offset;
        rect(centre - half, centre + half, DEAD_ZONE, lines);
    }
    if let Some(bounds) = rig.bounds {
        rect(bounds.min, bounds.max, BOUNDS, lines);
    }
    let group = rig.group();
    if group.len() < 2 {
        return;
    }
    let at: Vec<Vec2> = group
        .iter()
        .filter_map(|&id| entity_with_id(world, id))
        .filter(|&target| target != entity)
        .map(|target| world_matrix(world, target).transform_point2(Vec2::ZERO))
        .filter(|at| at.is_finite())
        .collect();
    let Some(&first) = at.first() else {
        return;
    };
    let (min, max) = at
        .iter()
        .fold((first, first), |(lo, hi), &p| (lo.min(p), hi.max(p)));
    let margin = rig
        .zoom
        .map_or(Vec2::ZERO, |zoom| zoom.margin.max(Vec2::ZERO));
    rect(min - margin, max + margin, GROUP, lines);
}

/// The four edges of the box `a`..`b`, either corner first.
fn rect(a: Vec2, b: Vec2, color: [f32; 4], lines: &mut LineBatch) {
    let (min, max) = (a.min(b), a.max(b));
    let corners = [min, Vec2::new(max.x, min.y), max, Vec2::new(min.x, max.y)];
    for i in 0..corners.len() {
        lines.push(corners[i], corners[(i + 1) % corners.len()], WIDTH, color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use voltra_scene::{FollowBounds, FollowZoom, SceneId, Transform};

    fn placed(world: &mut World, at: Vec2) -> Entity {
        let entity = world.spawn();
        world.insert(entity, SceneId::new());
        world.insert(entity, Transform::from_translation(at));
        entity
    }

    fn corners(lines: &LineBatch) -> Vec<Vec2> {
        lines.vertices().iter().map(|v| Vec2::from(v.a)).collect()
    }

    #[test]
    fn a_follower_s_dead_zone_and_bounds_are_drawn_where_they_are() {
        let mut world = World::new();
        let camera = placed(&mut world, Vec2::new(3.0, 1.0));
        world.insert(
            camera,
            CameraFollow2D {
                offset: Vec2::new(0.0, 1.0),
                dead_zone: Vec2::new(2.0, 0.5),
                bounds: Some(FollowBounds {
                    min: Vec2::splat(-20.0),
                    max: Vec2::splat(20.0),
                }),
                ..CameraFollow2D::default()
            },
        );
        let mut lines = LineBatch::default();
        draw(&world, camera, &mut lines);
        assert_eq!(lines.len(), 8);
        let at = corners(&lines);
        // Around where it rests, less the offset: centred on (3, 0).
        assert!(at.contains(&Vec2::new(1.0, -0.5)) && at.contains(&Vec2::new(5.0, 0.5)));
        assert!(at.contains(&Vec2::splat(-20.0)) && at.contains(&Vec2::splat(20.0)));
    }

    #[test]
    fn a_group_s_box_is_drawn_with_its_margin_and_without_the_missing() {
        let mut world = World::new();
        let a = placed(&mut world, Vec2::new(-4.0, 0.0));
        let b = placed(&mut world, Vec2::new(6.0, 2.0));
        let ids = [a, b].map(|e| *world.get::<SceneId>(e).expect("an id"));
        let camera = placed(&mut world, Vec2::ZERO);
        world.insert(
            camera,
            CameraFollow2D {
                zoom: Some(FollowZoom {
                    margin: Vec2::ONE,
                    ..FollowZoom::default()
                }),
                targets: vec![ids[1], SceneId::new()],
                ..CameraFollow2D::new(ids[0])
            },
        );
        let mut lines = LineBatch::default();
        draw(&world, camera, &mut lines);
        assert_eq!(lines.len(), 4);
        let at = corners(&lines);
        assert!(at.contains(&Vec2::new(-5.0, -1.0)) && at.contains(&Vec2::new(7.0, 3.0)));
    }

    #[test]
    fn an_entity_that_does_not_follow_draws_nothing() {
        let mut world = World::new();
        let entity = placed(&mut world, Vec2::ZERO);
        let mut lines = LineBatch::default();
        draw(&world, entity, &mut lines);
        assert!(lines.is_empty());
    }
}
