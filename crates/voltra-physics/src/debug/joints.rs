//! Joints, as line segments: the two anchors joined, each marked, and a
//! distance joint's slack or stretch visible as the line's length.

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

/// Pushes every joint whose bodies are there into `lines`.
pub fn draw(world: &World, lines: &mut LineBatch) {
    for end in joint_ends(world) {
        let Some(joint) = world.get::<Joint2D>(end.owner) else {
            continue;
        };
        let place = |entity, anchor: Vec2| {
            world
                .get::<Transform>(entity)
                .map(|t| t.matrix().transform_point2(anchor))
        };
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
