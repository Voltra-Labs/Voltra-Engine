//! Joints, solved beside the contacts: Box2D v3's joint solver.
//!
//! Each [`Joint2D`] in the world becomes a [`JointConstraint`] at the start of
//! the step — its two bodies found, its anchors turned into the world, its
//! impulses from last step loaded, those of parts it no longer has reset
//! (`active.rs`) — and then goes through the same passes a
//! contact does: warm started, solved in every sub-step with the soft bias
//! (`SolverParams::joint_softness`), solved once more without it, and its
//! impulses stored back on the component for the next step. Joints are
//! solved before the contacts in each pass, as Box2D does: a contact against
//! a jointed body then sees the joint's correction.
//!
//! A joint's body is anything the solver gathers — a rigid body, or a
//! collider with none, which is immovable. The world, as `b`, is a body that
//! does not move, at the anchor's world point.
//!
//! After the step, each joint's load — its impulses over the sub-step — is
//! measured against its `break_force` and `break_torque` ([`breaking`]).

mod active;
pub mod breaking;
mod distance;
mod ends;
mod line;
mod mouse;
mod point;
mod prismatic;
mod revolute;
mod side;
mod slide;
mod weld;
mod wheel;

use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;
use voltra_scene::{Joint2D, JointImpulses, JointKind, Transform};

use super::{Softness, SolverBodies};
pub use ends::{joint_ends, JointEnds};
use side::{spin, turn, Side};

/// One joint, ready for the passes.
#[derive(Debug, Clone)]
pub struct JointConstraint {
    /// The entity carrying the [`Joint2D`], which the impulses go back to.
    pub owner: Entity,
    /// The bodies it joins, as the world names them: what a break reports.
    ends: (Entity, Option<Entity>),
    kind: JointKind,
    a: usize,
    /// `None` for the world.
    b: Option<usize>,
    /// The anchors relative to each body's centre, turned into the world as
    /// the step began.
    frame_a: Vec2,
    frame_b: Vec2,
    /// Where `b`'s centre was relative to `a`'s as the step began.
    delta_center: Vec2,
    /// `a`'s rotation less `b`'s as the step began.
    angle: f32,
    /// A wheel's or a prismatic's axis, unit length, turned into the world
    /// with `b` as the step began; zero for the other kinds.
    axis: Vec2,
    /// The kind's own spring — a distance's, a revolute's, a wheel's
    /// suspension, a prismatic's, a mouse's pull — solved at, this step.
    spring: Softness,
    impulses: JointImpulses,
}

/// The joints of `ends` whose bodies the solver has, prepared for sub-steps
/// `h` long.
pub fn prepare_joints(
    world: &World,
    bodies: &SolverBodies,
    ends: &[JointEnds],
    h: f32,
) -> Vec<JointConstraint> {
    let mut out = Vec::with_capacity(ends.len());
    for end in ends {
        let Some(joint) = world.get::<Joint2D>(end.owner) else {
            continue;
        };
        let Some(a) = bodies.index_of(end.a) else {
            continue;
        };
        let b = match end.b {
            None => None,
            Some(entity) => match bodies.index_of(entity) {
                Some(index) => Some(index),
                None => continue,
            },
        };
        let place = |entity: Entity| {
            world
                .get::<Transform>(entity)
                .map_or((Vec2::ZERO, 0.0), |t| (t.translation, t.rotation))
        };
        let (center_a, angle_a) = place(end.a);
        let (center_b, angle_b) = match end.b {
            Some(entity) => place(entity),
            // The world: its "centre" is the anchor itself.
            None => (joint.anchor_b, 0.0),
        };
        let frame_b = if b.is_some() {
            turn(angle_b, joint.anchor_b)
        } else {
            Vec2::ZERO
        };
        let values = [
            center_a.x,
            center_a.y,
            center_b.x,
            center_b.y,
            angle_a,
            angle_b,
            joint.anchor_a.x,
            joint.anchor_a.y,
            joint.anchor_b.x,
            joint.anchor_b.y,
        ];
        if !values.iter().all(|v| v.is_finite()) {
            continue;
        }
        let spring = match joint.kind {
            JointKind::Distance {
                spring: Some(spring),
                ..
            } => Softness::new(spring.hertz, spring.damping_ratio, h),
            JointKind::Revolute {
                spring: Some(spring),
                ..
            }
            | JointKind::Wheel {
                spring: Some(spring),
                ..
            }
            | JointKind::Prismatic {
                spring: Some(spring),
                ..
            } => Softness::new(spring.hertz, spring.damping_ratio, h),
            JointKind::Mouse {
                hertz,
                damping_ratio,
                ..
            } => Softness::new(hertz, damping_ratio, h),
            _ => Softness::RIGID,
        };
        // A line's axis, in `b`'s space: a zero or broken one is no line,
        // and the joint joins nothing, as one naming a missing body does.
        let axis = match joint.kind {
            JointKind::Wheel { axis, .. } | JointKind::Prismatic { axis, .. } => {
                match axis.try_normalize() {
                    Some(axis) => turn(angle_b, axis),
                    None => continue,
                }
            }
            _ => Vec2::ZERO,
        };
        out.push(JointConstraint {
            owner: end.owner,
            ends: (end.a, end.b),
            kind: joint.kind,
            a,
            b,
            frame_a: turn(angle_a, joint.anchor_a),
            frame_b,
            delta_center: center_b - center_a,
            angle: angle_a - angle_b,
            axis,
            spring,
            impulses: active::active(&joint.kind, &joint.impulses, h),
        });
    }
    out
}

/// Applies every joint's impulses from last step, before the sub-steps.
pub fn warm_start_joints(joints: &[JointConstraint], bodies: &mut SolverBodies) {
    for joint in joints {
        let (mut a, mut b) = (
            Side::load(bodies, Some(joint.a)),
            Side::load(bodies, joint.b),
        );
        let (ra, rb) = (turn(a.dq, joint.frame_a), turn(b.dq, joint.frame_b));
        let i = &joint.impulses;
        // The angular part turns `a` and `b` the other way: the joint's angle
        // is `a`'s against `b`'s.
        let (linear, angular) = match joint.kind {
            JointKind::Revolute { .. } => (i.linear, -(i.spring + i.motor + i.lower - i.upper)),
            JointKind::Weld { .. } => (i.linear, -i.angular),
            JointKind::Mouse { .. } => (i.linear, 0.0),
            JointKind::Wheel { .. } | JointKind::Prismatic { .. } => {
                // A line's arm on `b` reaches to `a`'s anchor, not `b`'s.
                line::warm_start(joint, &mut a, &mut b);
                a.store(bodies, Some(joint.a));
                b.store(bodies, joint.b);
                continue;
            }
            JointKind::Distance { .. } => {
                let d = (b.dp - a.dp) + joint.delta_center + rb - ra;
                let axis = d.normalize_or_zero();
                (axis * (i.axial + i.lower - i.upper), 0.0)
            }
        };
        a.v -= a.m * linear;
        a.w -= a.i * (ra.perp_dot(linear) + angular);
        b.v += b.m * linear;
        b.w += b.i * (rb.perp_dot(linear) + angular);
        a.store(bodies, Some(joint.a));
        b.store(bodies, joint.b);
    }
}

/// Solves every joint once: in a sub-step with `use_bias`, in the relax pass
/// without. `softness` is the joints' own spring; `h` the sub-step.
pub fn solve_joints(
    joints: &mut [JointConstraint],
    bodies: &mut SolverBodies,
    softness: Softness,
    h: f32,
    use_bias: bool,
) {
    for joint in joints {
        let (mut a, mut b) = (
            Side::load(bodies, Some(joint.a)),
            Side::load(bodies, joint.b),
        );
        match joint.kind {
            JointKind::Revolute {
                limit,
                motor,
                spring,
                ..
            } => revolute::solve(
                joint,
                &mut a,
                &mut b,
                revolute::Parts {
                    limit,
                    motor,
                    spring,
                },
                softness,
                h,
                use_bias,
            ),
            JointKind::Wheel { .. } => wheel::solve(joint, &mut a, &mut b, softness, h, use_bias),
            JointKind::Prismatic { .. } => {
                prismatic::solve(joint, &mut a, &mut b, softness, h, use_bias)
            }
            JointKind::Mouse { .. } => mouse::solve(joint, &mut a, &mut b, softness, h, use_bias),
            JointKind::Weld { .. } => weld::solve(joint, &mut a, &mut b, softness, use_bias),
            JointKind::Distance { .. } => {
                distance::solve(joint, &mut a, &mut b, softness, h, use_bias)
            }
        }
        a.store(bodies, Some(joint.a));
        b.store(bodies, joint.b);
    }
}

/// Puts every joint's impulses back on its component, for the next step.
pub fn store_joints(joints: &[JointConstraint], world: &mut World) {
    for joint in joints {
        if let Some(component) = world.get_mut::<Joint2D>(joint.owner) {
            component.impulses = joint.impulses;
        }
    }
}

impl JointConstraint {
    /// The anchors relative to the centres, turned with the bodies so far.
    fn arms(&self, a: &Side, b: &Side) -> (Vec2, Vec2) {
        (turn(a.dq, self.frame_a), turn(b.dq, self.frame_b))
    }

    /// The joint's angle now — `a`'s rotation against `b`'s — less
    /// `reference`.
    fn angle_now(&self, a: &Side, b: &Side, reference: f32) -> f32 {
        self.angle + (a.dq - b.dq) - reference
    }
}

#[cfg(test)]
mod tests;
