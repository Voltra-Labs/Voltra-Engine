//! The rest of the selection following a drag on its primary.
//!
//! The gizmo sits on the primary and the drag works out where the primary
//! goes; every other selected entity then takes the same change, about the
//! primary's pivot — Godot's rule, and Unity's "Pivot" handle with its
//! active object: a move moves them all by the same amount, a turn swings
//! them round the primary and turns each by the same angle, a scale
//! stretches their distances from it along its axes and scales each by the
//! same factors. World transforms throughout; the frame turns them back
//! into local ones, as it does for the primary.

use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;
use voltra_scene::{hierarchy, Transform};

use crate::tool::Tool;

/// Below this, a scale on an axis is taken as collapsed and the factor on
/// it as one, rather than dividing by it.
const MIN_SCALE: f32 = 1e-6;

/// Where `follower` (its world transform when the drag began) goes, the
/// primary having gone from `start` to `now` under `tool`.
pub fn follow(tool: Tool, start: &Transform, now: &Transform, follower: &Transform) -> Transform {
    let pivot = start.translation;
    match tool {
        Tool::Translate => Transform {
            translation: follower.translation + (now.translation - start.translation),
            ..*follower
        },
        Tool::Rotate => {
            let turn = now.rotation - start.rotation;
            Transform {
                translation: pivot + Vec2::from_angle(turn).rotate(follower.translation - pivot),
                rotation: follower.rotation + turn,
                ..*follower
            }
        }
        Tool::Scale => {
            let factor = |was: f32, is: f32| if was.abs() < MIN_SCALE { 1.0 } else { is / was };
            let ratio = Vec2::new(
                factor(start.scale.x, now.scale.x),
                factor(start.scale.y, now.scale.y),
            );
            // The offset in the primary's axes, stretched, and back.
            let axes = Vec2::from_angle(start.rotation);
            let local = Vec2::from_angle(-start.rotation).rotate(follower.translation - pivot);
            Transform {
                translation: pivot + axes.rotate(local * ratio),
                scale: follower.scale * ratio,
                ..*follower
            }
        }
    }
}

/// The selected entities a drag on `primary` carries, with their world
/// transforms now: every one of `selected` but the primary and those under
/// another selected entity — a child already moves with its parent, and
/// moving it too would move it twice. Entities without a transform stay.
pub fn followers(
    world: &World,
    primary: Entity,
    selected: impl IntoIterator<Item = Entity>,
) -> Vec<(Entity, Transform)> {
    let selected: Vec<Entity> = selected.into_iter().collect();
    let carried = |entity: Entity| {
        let mut up = hierarchy::parent_of(world, entity);
        // Bounded by the scene's size: a cycle in a hand-edited file stops.
        let mut steps = 0;
        while let Some(parent) = up {
            if parent == primary || selected.contains(&parent) {
                return true;
            }
            steps += 1;
            if steps > selected.len() + 1024 {
                break;
            }
            up = hierarchy::parent_of(world, parent);
        }
        false
    };
    selected
        .iter()
        .copied()
        .filter(|&entity| entity != primary && world.is_alive(entity))
        .filter(|&entity| world.get::<Transform>(entity).is_some() && !carried(entity))
        .map(|entity| (entity, hierarchy::world_transform(world, entity)))
        .collect()
}

#[cfg(test)]
mod tests {
    use std::f32::consts::FRAC_PI_2;

    use voltra_scene::SceneId;

    use super::*;

    fn near(a: Vec2, b: Vec2) -> bool {
        a.distance(b) < 1e-5
    }

    #[test]
    fn a_move_moves_every_follower_by_the_same_amount() {
        let start = Transform::from_translation(Vec2::new(1.0, 1.0));
        let now = Transform::from_translation(Vec2::new(3.0, 0.0));
        let other = Transform::from_translation(Vec2::new(-1.0, 5.0)).with_rotation(0.3);
        let moved = follow(Tool::Translate, &start, &now, &other);
        assert!(near(moved.translation, Vec2::new(1.0, 4.0)));
        assert_eq!(moved.rotation, 0.3, "nothing else changes");
    }

    #[test]
    fn a_turn_swings_followers_round_the_primary_and_turns_them() {
        let start = Transform::from_translation(Vec2::new(1.0, 0.0));
        let now = start.with_rotation(FRAC_PI_2);
        let other = Transform::from_translation(Vec2::new(3.0, 0.0));
        let turned = follow(Tool::Rotate, &start, &now, &other);
        assert!(near(turned.translation, Vec2::new(1.0, 2.0)), "{turned:?}");
        assert!((turned.rotation - FRAC_PI_2).abs() < 1e-6);
    }

    #[test]
    fn a_scale_stretches_distances_along_the_primary_s_axes() {
        // The primary turned a quarter: its x axis is the world's up.
        let start = Transform::default().with_rotation(FRAC_PI_2);
        let now = Transform {
            scale: Vec2::new(2.0, 1.0),
            ..start
        };
        let above = Transform::from_translation(Vec2::new(0.0, 1.0));
        let beside = Transform::from_translation(Vec2::new(1.0, 0.0));
        let a = follow(Tool::Scale, &start, &now, &above);
        let b = follow(Tool::Scale, &start, &now, &beside);
        assert!(
            near(a.translation, Vec2::new(0.0, 2.0)),
            "along its x: {a:?}"
        );
        assert!(near(b.translation, Vec2::new(1.0, 0.0)), "across it: {b:?}");
        assert_eq!(a.scale, Vec2::new(2.0, 1.0));
    }

    #[test]
    fn a_collapsed_axis_scales_nothing_rather_than_dividing_by_zero() {
        let start = Transform {
            scale: Vec2::new(0.0, 1.0),
            ..Transform::default()
        };
        let now = Transform {
            scale: Vec2::new(5.0, 1.0),
            ..start
        };
        let other = Transform::from_translation(Vec2::new(2.0, 0.0));
        let scaled = follow(Tool::Scale, &start, &now, &other);
        assert!(scaled.translation.is_finite() && scaled.scale.is_finite());
        assert!(near(scaled.translation, Vec2::new(2.0, 0.0)));
    }

    #[test]
    fn followers_leave_out_the_primary_and_what_rides_on_a_selected_parent() {
        let mut world = World::new();
        let spawn = |world: &mut World, x: f32| {
            let e = world.spawn();
            world.insert(e, SceneId::new());
            world.insert(e, Transform::from_translation(Vec2::new(x, 0.0)));
            e
        };
        let primary = spawn(&mut world, 0.0);
        let parent = spawn(&mut world, 1.0);
        let child = spawn(&mut world, 2.0);
        let under_primary = spawn(&mut world, 3.0);
        let bare = world.spawn();
        hierarchy::set_parent(&mut world, child, parent).expect("no cycle");
        hierarchy::set_parent(&mut world, under_primary, primary).expect("no cycle");
        let carried: Vec<Entity> = followers(
            &world,
            primary,
            [parent, child, under_primary, bare, primary],
        )
        .into_iter()
        .map(|(e, _)| e)
        .collect();
        assert_eq!(carried, [parent]);
    }
}
