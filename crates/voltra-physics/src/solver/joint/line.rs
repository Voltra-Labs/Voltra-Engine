//! A point kept on a line: the geometry a wheel and a prismatic joint share,
//! worked out as Box2D's `b2WheelJoint` and `b2PrismaticJoint` do.
//!
//! Box2D's body A carries the line and its B slides on it. Here the joint
//! lives on the body that moves, as a revolute's lives on the body that
//! turns, so Box2D's B is `a` — the mover — and its A is `b`, the base the
//! axis belongs to. The arm on the base reaches from its centre to the
//! *mover's* anchor (Box2D's `d + rA`): a push along the line acts where the
//! mover is on it, not where the base's anchor is.

use voltra_render::glam::Vec2;
use voltra_scene::JointKind;

use super::{turn, JointConstraint, Side};

/// The line as the bodies stand now.
#[derive(Debug, Clone, Copy)]
pub(super) struct Line {
    /// The axis in the world, unit length.
    pub(super) axis: Vec2,
    /// The axis turned a quarter counter-clockwise.
    pub(super) perp: Vec2,
    /// How far the mover's anchor is along the axis from the base's.
    pub(super) translation: f32,
    /// How far it is off the line.
    pub(super) across: f32,
    /// The base's and the mover's lever arms along the axis.
    a1: f32,
    a2: f32,
    /// Their lever arms across it.
    pub(super) s1: f32,
    pub(super) s2: f32,
    /// One over the effective mass along the axis — zero when nothing can
    /// move along it.
    pub(super) axial_mass: f32,
}

impl Line {
    /// The line between `a` (the mover) and `b` (the base) now.
    pub(super) fn now(joint: &JointConstraint, a: &Side, b: &Side) -> Self {
        let (ra, rb) = joint.arms(a, b);
        // The mover's anchor less the base's.
        let d = (a.dp - b.dp) - joint.delta_center + ra - rb;
        let axis = turn(b.dq, joint.axis);
        let perp = axis.perp();
        let base_arm = d + rb;
        let (a1, a2) = (base_arm.perp_dot(axis), ra.perp_dot(axis));
        let (s1, s2) = (base_arm.perp_dot(perp), ra.perp_dot(perp));
        let k = a.m + b.m + b.i * a1 * a1 + a.i * a2 * a2;
        Self {
            axis,
            perp,
            translation: axis.dot(d),
            across: perp.dot(d),
            a1,
            a2,
            s1,
            s2,
            axial_mass: if k > 0.0 { 1.0 / k } else { 0.0 },
        }
    }

    /// The mover's speed along the axis, against the base's.
    pub(super) fn along_speed(&self, a: &Side, b: &Side) -> f32 {
        self.axis.dot(a.v - b.v) + self.a2 * a.w - self.a1 * b.w
    }

    /// Its speed across the axis.
    pub(super) fn across_speed(&self, a: &Side, b: &Side) -> f32 {
        self.perp.dot(a.v - b.v) + self.s2 * a.w - self.s1 * b.w
    }

    /// One over the effective mass across the axis.
    pub(super) fn perp_mass(&self, a: &Side, b: &Side) -> f32 {
        let k = a.m + b.m + b.i * self.s1 * self.s1 + a.i * self.s2 * self.s2;
        if k > 0.0 {
            1.0 / k
        } else {
            0.0
        }
    }

    /// `impulse` along the axis: the mover pushed forward, the base back.
    pub(super) fn push_along(&self, a: &mut Side, b: &mut Side, impulse: f32) {
        let p = self.axis * impulse;
        b.v -= b.m * p;
        b.w -= b.i * self.a1 * impulse;
        a.v += a.m * p;
        a.w += a.i * self.a2 * impulse;
    }

    /// `across` along the perpendicular and `angular` about the joint, at
    /// once: a wheel's line held and, for a prismatic, its angle.
    pub(super) fn push_across(&self, a: &mut Side, b: &mut Side, across: f32, angular: f32) {
        let p = self.perp * across;
        b.v -= b.m * p;
        b.w -= b.i * (self.s1 * across + angular);
        a.v += a.m * p;
        a.w += a.i * (self.s2 * across + angular);
    }
}

/// Applies a wheel's or a prismatic's impulses from before, at the start of
/// a sub-step.
pub(super) fn warm_start(joint: &JointConstraint, a: &mut Side, b: &mut Side) {
    let line = Line::now(joint, a, b);
    let i = &joint.impulses;
    let (axial, angular) = match joint.kind {
        // A wheel's motor turns it; it does not slide it.
        JointKind::Wheel { .. } => (i.spring + i.lower - i.upper, i.motor),
        _ => (i.spring + i.motor + i.lower - i.upper, i.angular),
    };
    line.push_along(a, b, axial);
    line.push_across(a, b, i.perp, angular);
}
