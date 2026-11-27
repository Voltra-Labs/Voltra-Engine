//! Joints loaded past what they hold: removed, and reported.
//!
//! Box2D v3's joint events measure a joint's load as its constraint force —
//! the impulse it needed over the sub-step, since the impulse is a
//! sub-step's (warm started in each) — against a threshold. The same here,
//! once the step is solved, so the load is the step's settled one rather
//! than a sub-step's spike.

use voltra_ecs::World;
use voltra_scene::{Joint2D, JointImpulses, JointKind};

use super::JointConstraint;
use crate::events::JointBroken;

/// The force and the torque the joint's impulses hold over a sub-step `h`
/// long: every linear part at once, and every angular part (counter-clockwise
/// on `a`) at once.
pub fn load(kind: &JointKind, impulses: &JointImpulses, h: f32) -> (f32, f32) {
    let i = impulses;
    let (force, torque) = match kind {
        JointKind::Revolute { .. } => (i.linear.length(), i.spring + i.motor + i.lower - i.upper),
        JointKind::Weld { .. } => (i.linear.length(), i.angular),
        JointKind::Mouse { .. } => (i.linear.length(), 0.0),
        JointKind::Distance { .. } => ((i.axial + i.lower - i.upper).abs(), 0.0),
        JointKind::Wheel { .. } => (i.perp.hypot(i.spring + i.lower - i.upper), i.motor),
        JointKind::Prismatic { .. } => (
            i.perp.hypot(i.spring + i.motor + i.lower - i.upper),
            i.angular,
        ),
    };
    if h > 0.0 {
        (force / h, torque / h)
    } else {
        (0.0, 0.0)
    }
}

/// Removes every joint of `joints` its load broke this step, sub-steps `h`
/// long, and says which. A joint whose entity is gone — despawned by a game
/// between steps — has nothing left to break.
pub fn break_joints(joints: &[JointConstraint], world: &mut World, h: f32) -> Vec<JointBroken> {
    let mut broken = Vec::new();
    for joint in joints {
        let Some(component) = world.get::<Joint2D>(joint.owner) else {
            continue;
        };
        if component.break_force.is_none() && component.break_torque.is_none() {
            continue;
        }
        let (force, torque) = load(&joint.kind, &joint.impulses, h);
        if !component.breaks_under(force, torque) {
            continue;
        }
        world.remove::<Joint2D>(joint.owner);
        let (a, b) = joint.ends;
        log::debug!(
            "a {} joint broke under {force:.1} N and {torque:.1} N·m",
            joint.kind.name()
        );
        broken.push(JointBroken {
            entity: joint.owner,
            a,
            b,
            force,
            torque,
        });
    }
    broken
}
