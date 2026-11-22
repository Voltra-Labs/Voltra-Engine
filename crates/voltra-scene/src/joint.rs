//! Joints: two bodies held to each other, or one held to the world.
//!
//! Box2D's revolute, distance and weld joints, solved by `voltra-physics`
//! inside the same sub-steps as the contacts. A [`Joint2D`] lives on any
//! entity and names the two bodies it joins, Godot's `PinJoint2D` shape
//! rather than Unity's component-on-the-body: one entity holds one component
//! of a kind, and a bridge plank pinned at both ends, or a door with two
//! hinges, needs two joints. The common case — a joint on the body itself,
//! held to the world — names neither.

use serde::{Deserialize, Serialize};
use voltra_render::glam::Vec2;

use crate::scene_id::SceneId;

/// Holds two bodies together by `kind`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Joint2D {
    /// The first body, by identity; `None` — the default, and not written —
    /// is the entity carrying the joint.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub a: Option<SceneId>,
    /// The second body, by identity; `None` — the default, and not written —
    /// is the world, which does not move.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub b: Option<SceneId>,
    /// Where on `a` the joint holds, in `a`'s own space.
    pub anchor_a: Vec2,
    /// Where on `b` the joint holds, in `b`'s own space — or the point in
    /// the world, when `b` is the world.
    pub anchor_b: Vec2,
    /// Whether the two bodies still collide with each other. Off — Box2D's
    /// default — so a hinge's two halves can overlap where they meet.
    pub collide_connected: bool,
    pub kind: JointKind,
    /// The impulses the joint needed last step, which the next one starts
    /// from — warm starting, as the contacts do. Never saved: a loaded
    /// joint starts from rest.
    #[serde(skip)]
    pub impulses: JointImpulses,
}

impl Default for Joint2D {
    /// A pin through the carrying entity's origin, held to the world at the
    /// origin.
    fn default() -> Self {
        Self {
            a: None,
            b: None,
            anchor_a: Vec2::ZERO,
            anchor_b: Vec2::ZERO,
            collide_connected: false,
            kind: JointKind::default(),
            impulses: JointImpulses::default(),
        }
    }
}

impl Joint2D {
    /// The registered name a scene file stores it under.
    pub const NAME: &'static str = "Joint2D";
}

impl crate::format::SceneRefs for Joint2D {
    /// The bodies it names: a copy's joint to a body inside the copy follows
    /// the copy, one to a body outside it keeps that body.
    fn remap(&mut self, map: &dyn Fn(SceneId) -> Option<SceneId>) {
        for end in [&mut self.a, &mut self.b] {
            if let Some(id) = *end {
                *end = Some(map(id).unwrap_or(id));
            }
        }
    }
}

/// What a joint holds.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum JointKind {
    /// The two anchors held together, the bodies free to turn about them —
    /// a hinge, a pendulum's pivot, a wheel's axle.
    Revolute {
        /// `a`'s rotation less `b`'s at which the joint's angle is zero:
        /// what the limit is measured from. The angle, the limit and the
        /// motor are all `a`'s against `b`'s — the joint lives on the body
        /// that turns, held to what it turns on.
        #[serde(default)]
        reference_angle: f32,
        /// How far it may turn, in radians either way of the reference.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        limit: Option<AngleLimit>,
        /// A motor turning `a` against `b`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        motor: Option<Motor>,
    },
    /// The anchors kept a distance apart: a rod at `length`, a spring about
    /// it, or — with `min` below `max` — anything in between, a rope at
    /// `min` zero.
    Distance {
        /// The rest length.
        length: f32,
        /// With `min` below `max`, the range the distance is held in; equal,
        /// the joint is held at `length` (by `spring`, if it has one).
        #[serde(default)]
        min: f32,
        #[serde(default)]
        max: f32,
        /// Pulls the distance towards `length` like a spring; `None` holds it
        /// there rigidly, or not at all inside a range.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        spring: Option<Spring>,
    },
    /// The two bodies held as one: the anchors together, and the angle
    /// between them kept.
    Weld {
        /// `a`'s rotation less `b`'s that is kept.
        #[serde(default)]
        reference_angle: f32,
    },
}

impl Default for JointKind {
    fn default() -> Self {
        Self::Revolute {
            reference_angle: 0.0,
            limit: None,
            motor: None,
        }
    }
}

impl JointKind {
    /// What the kind is called, for an editor and a log.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Revolute { .. } => "revolute",
            Self::Distance { .. } => "distance",
            Self::Weld { .. } => "weld",
        }
    }

    /// A rigid rod `length` long.
    pub fn rod(length: f32) -> Self {
        Self::Distance {
            length,
            min: length,
            max: length,
            spring: None,
        }
    }

    /// A rope: slack up to `length`, taut at it.
    pub fn rope(length: f32) -> Self {
        Self::Distance {
            length,
            min: 0.0,
            max: length,
            spring: None,
        }
    }
}

/// How far a revolute joint may turn.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AngleLimit {
    /// Radians, at most `upper`.
    pub lower: f32,
    pub upper: f32,
}

impl Default for AngleLimit {
    /// A quarter turn either way.
    fn default() -> Self {
        Self {
            lower: -std::f32::consts::FRAC_PI_4,
            upper: std::f32::consts::FRAC_PI_4,
        }
    }
}

/// A revolute joint's motor.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Motor {
    /// The speed it drives `a` at against `b`, radians per second,
    /// counter-clockwise.
    pub speed: f32,
    /// The most torque it may use getting there.
    pub max_torque: f32,
}

impl Default for Motor {
    /// A turn a second, with the torque to do it to a small body.
    fn default() -> Self {
        Self {
            speed: std::f32::consts::TAU,
            max_torque: 10.0,
        }
    }
}

/// A distance joint's spring.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Spring {
    /// How fast it springs back, in hertz.
    pub hertz: f32,
    /// `0` bounces for ever, `1` settles without overshooting.
    pub damping_ratio: f32,
}

impl Default for Spring {
    /// Bouncy, and settling in a second or two.
    fn default() -> Self {
        Self {
            hertz: 4.0,
            damping_ratio: 0.3,
        }
    }
}

/// The impulses a joint accumulated, carried from one step to the next.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct JointImpulses {
    /// Holding the anchors together (revolute, weld).
    pub linear: Vec2,
    /// Holding the angle (weld).
    pub angular: f32,
    /// Along the axis: the rod or the spring (distance).
    pub axial: f32,
    /// The motor's.
    pub motor: f32,
    /// The lower and upper limits' — angle or distance.
    pub lower: f32,
    pub upper: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_default_joint_writes_little_and_reads_back() {
        let text = ron::to_string(&Joint2D::default()).expect("writes");
        assert!(
            !text.contains("impulses") && !text.starts_with("(a:"),
            "{text}"
        );
        let back: Joint2D = ron::from_str(&text).expect("reads");
        assert_eq!(back, Joint2D::default());
        let rope = Joint2D {
            b: Some(SceneId::new()),
            kind: JointKind::rope(2.0),
            ..Joint2D::default()
        };
        let back: Joint2D = ron::from_str(&ron::to_string(&rope).expect("writes")).expect("reads");
        assert_eq!(back, rope);
    }
}
