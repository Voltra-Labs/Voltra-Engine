//! Joints, as line segments: the two anchors joined, each marked, and a
//! distance joint's slack or stretch visible as the line's length; a wheel's
//! or a slider's axis and its limits, a hinge's limits, a mouse joint's
//! target.

use voltra_ecs::World;
use voltra_render::glam::Vec2;
use voltra_render::LineBatch;
use voltra_scene::{Joint2D, JointKind, Transform};

use crate::solver::joint_ends;

/// Joints, in the cool white no collider or contact uses.
const JOINT_COLOR: [f32; 4] = [0.75, 0.85, 1.0, 0.9];

/// A joint held to the world, its world end in the colour of fixed things.
const WORLD_COLOR: [f32; 4] = [0.6, 0.6, 0.6, 0.9];

/// Thickness, in pixels. Matches the colliders'.
const WIDTH: f32 = 1.5;

/// Half the side of the cross marking an anchor, in world units.
const MARK: f32 = 0.08;

/// An axis, a limit and a hinge's travel: the warm amber the overlay keeps
/// for what constrains rather than what holds.
const LIMIT_COLOR: [f32; 4] = [1.0, 0.75, 0.3, 0.9];

/// How far an axis with no limit is drawn either way, and how long a hinge
/// limit's rays are, in world units.
const REACH: f32 = 0.5;

/// Pushes every joint whose bodies are there into `lines`.
pub fn draw(world: &World, lines: &mut LineBatch) {
    for end in joint_ends(world) {
        let Some(joint) = world.get::<Joint2D>(end.owner) else {
            continue;
        };
        // The joint's own rule: turned and moved with the body, unscaled,
        // as the solver holds it.
        let place = |entity, anchor: Vec2| {
            world
                .get::<Transform>(entity)
                .map(|t| Joint2D::anchor_in_world(t, anchor))
        };
        // A line joint with no axis joins nothing in the solver: nothing is
        // drawn for it either.
        let axis = match joint.kind {
            JointKind::Wheel { axis, .. } | JointKind::Prismatic { axis, .. } => {
                match axis.try_normalize() {
                    Some(axis) => axis,
                    None => continue,
                }
            }
            _ => Vec2::ZERO,
        };
        // What `b`'s space turns by: the world's is none.
        let b_turn = end
            .b
            .and_then(|entity| world.get::<Transform>(entity))
            .map_or(0.0, |t| t.rotation);
        let Some(a) = place(end.a, joint.anchor_a) else {
            continue;
        };
        let (b, b_color) = match end.b {
            Some(entity) => match place(entity, joint.anchor_b) {
                Some(b) => (b, JOINT_COLOR),
                None => continue,
            },
            None => (joint.anchor_b, WORLD_COLOR),
        };
        if !(a.is_finite() && b.is_finite()) {
            continue;
        }
        lines.push(a, b, WIDTH, JOINT_COLOR);
        mark(lines, a, JOINT_COLOR);
        mark(lines, b, b_color);
        match joint.kind {
            JointKind::Wheel { limit, .. } | JointKind::Prismatic { limit, .. } => {
                let axis = Vec2::from_angle(b_turn).rotate(axis);
                let (lower, upper) = limit.map_or((-REACH, REACH), |l| (l.lower, l.upper));
                lines.push(b + axis * lower, b + axis * upper, WIDTH, LIMIT_COLOR);
                if limit.is_some() {
                    let across = axis.perp() * MARK * 1.5;
                    for at in [b + axis * lower, b + axis * upper] {
                        lines.push(at - across, at + across, WIDTH, LIMIT_COLOR);
                    }
                }
            }
            JointKind::Revolute {
                reference_angle,
                limit: Some(limit),
                ..
            } => {
                // Where `a`'s own x axis may turn to, from the hinge.
                for angle in [limit.lower, limit.upper] {
                    let ray = Vec2::from_angle(b_turn + reference_angle + angle) * REACH;
                    lines.push(a, a + ray, WIDTH, LIMIT_COLOR);
                }
                if let Some(t) = world.get::<Transform>(end.a) {
                    lines.push(
                        a,
                        a + Vec2::from_angle(t.rotation) * REACH,
                        WIDTH,
                        JOINT_COLOR,
                    );
                }
            }
            _ => {}
        }
        // A weld's ends coincide; a square says it holds the angle too.
        if matches!(joint.kind, JointKind::Weld { .. }) {
            let corners = [
                a + Vec2::new(-MARK, -MARK),
                a + Vec2::new(MARK, -MARK),
                a + Vec2::new(MARK, MARK),
                a + Vec2::new(-MARK, MARK),
            ];
            for i in 0..4 {
                lines.push(corners[i], corners[(i + 1) % 4], WIDTH, JOINT_COLOR);
            }
        }
    }
}

/// A small cross at `at`.
fn mark(lines: &mut LineBatch, at: Vec2, color: [f32; 4]) {
    lines.push(at - Vec2::X * MARK, at + Vec2::X * MARK, WIDTH, color);
    lines.push(at - Vec2::Y * MARK, at + Vec2::Y * MARK, WIDTH, color);
}

#[cfg(test)]
mod tests {
    use super::*;
    use voltra_scene::{AngleLimit, TranslationLimit};

    /// Every segment `joint` draws, with its colour, on a body placed at
    /// `placed` held to the world.
    fn drawn(placed: Transform, joint: Joint2D) -> Vec<(Vec2, Vec2, [f32; 4])> {
        let mut world = World::new();
        let body = world.spawn();
        world.insert(body, placed);
        world.insert(body, joint);
        let mut lines = LineBatch::default();
        draw(&world, &mut lines);
        lines
            .vertices()
            .chunks(4)
            .map(|quad| (Vec2::from(quad[0].a), Vec2::from(quad[0].b), quad[0].color))
            .collect()
    }

    /// The segments `kind` draws in the limits' colour, on a body at the
    /// origin held to the world there.
    fn limit_segments(kind: JointKind) -> Vec<(Vec2, Vec2)> {
        let joint = Joint2D {
            kind,
            ..Joint2D::default()
        };
        drawn(Transform::default(), joint)
            .into_iter()
            .filter(|&(_, _, color)| color == LIMIT_COLOR)
            .map(|(from, to, _)| (from, to))
            .collect()
    }

    #[test]
    fn a_slider_draws_its_axis_between_its_limits_and_marks_them() {
        let segments = limit_segments(JointKind::Prismatic {
            axis: Vec2::new(0.0, 2.0),
            reference_angle: 0.0,
            limit: Some(TranslationLimit {
                lower: -1.0,
                upper: 2.0,
            }),
            motor: None,
            spring: None,
        });
        assert_eq!(segments.len(), 3, "the axis and a mark at each limit");
        let (from, to) = segments[0];
        assert!((from - Vec2::new(0.0, -1.0)).length() < 1e-5, "{from}");
        assert!((to - Vec2::new(0.0, 2.0)).length() < 1e-5, "{to}");
    }

    #[test]
    fn a_hinge_draws_its_limits_and_a_free_one_does_not() {
        let limited = JointKind::Revolute {
            reference_angle: 0.0,
            limit: Some(AngleLimit::default()),
            motor: None,
            spring: None,
        };
        assert_eq!(limit_segments(limited).len(), 2, "a ray to each limit");
        assert!(limit_segments(JointKind::default()).is_empty());
    }

    #[test]
    fn a_zero_axis_draws_nothing_as_the_solver_joins_nothing() {
        let joint = Joint2D {
            kind: JointKind::prismatic(Vec2::ZERO),
            ..Joint2D::default()
        };
        assert!(drawn(Transform::default(), joint).is_empty());
    }

    #[test]
    fn an_anchor_on_a_scaled_body_is_drawn_unscaled_as_it_is_held() {
        let placed = Transform::from_translation(Vec2::new(1.0, 0.0)).with_scale(Vec2::splat(2.0));
        let joint = Joint2D {
            anchor_a: Vec2::new(0.5, 0.0),
            anchor_b: Vec2::new(3.0, 0.0),
            ..Joint2D::default()
        };
        // Half a unit out from the centre, not the scaled unit.
        let (from, to, _) = drawn(placed, joint)[0];
        assert!((from - Vec2::new(1.5, 0.0)).length() < 1e-5, "{from}");
        assert!((to - Vec2::new(3.0, 0.0)).length() < 1e-5, "{to}");
    }
}
