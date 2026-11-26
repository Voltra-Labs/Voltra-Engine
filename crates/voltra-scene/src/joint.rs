//! Joints: two bodies held to each other, or one held to the world.
//!
//! Box2D's revolute, distance, weld, wheel, prismatic and mouse joints,
//! solved by `voltra-physics`
//! inside the same sub-steps as the contacts. A [`Joint2D`] lives on any
//! entity and names the two bodies it joins, Godot's `PinJoint2D` shape
//! rather than Unity's component-on-the-body: one entity holds one component
//! of a kind, and a bridge plank pinned at both ends, or a door with two
//! hinges, needs two joints. The common case — a joint on the body itself,
//! held to the world — names neither.

use serde::{Deserialize, Serialize};
use voltra_render::glam::Vec2;

use crate::scene_id::SceneId;

mod anchor;
mod impulses;
mod kind;
mod parts;

pub use impulses::JointImpulses;
pub use kind::JointKind;
pub use parts::{AngleLimit, LinearMotor, Motor, RestSpring, Spring, TranslationLimit};

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
    /// Where on `a` the joint holds, in `a`'s own space: turned and moved
    /// with `a`, not scaled ([`Joint2D::anchor_in_world`]).
    pub anchor_a: Vec2,
    /// Where on `b` the joint holds, in `b`'s own space as `anchor_a` is in
    /// `a`'s — or the point in the world, when `b` is the world.
    pub anchor_b: Vec2,
    /// Whether the two bodies still collide with each other. Off — Box2D's
    /// default — so a hinge's two halves can overlap where they meet.
    pub collide_connected: bool,
    /// Which constraint this is, and its own parameters.
    pub kind: JointKind,
    /// The force, in newtons, past which the joint breaks: the physics
    /// removes the component and reports a `JointBroken`. `None` — the
    /// default, and not written — never breaks. Measured, as Box2D does,
    /// from the impulse the solver needed over the sub-step it acted in.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub break_force: Option<f32>,
    /// The torque past which the joint breaks, as `break_force`. Only the
    /// kinds that hold an angle feel one: a revolute's limit, motor and
    /// spring, a weld, a wheel's motor, a prismatic's held angle.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub break_torque: Option<f32>,
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
            break_force: None,
            break_torque: None,
            impulses: JointImpulses::default(),
        }
    }
}

impl Joint2D {
    /// The registered name a scene file stores it under.
    pub const NAME: &'static str = "Joint2D";

    /// Whether a load of `force` newtons and `torque` newton-metres (either
    /// way) is past what the joint holds. A threshold that is not a number
    /// never breaks it.
    pub fn breaks_under(&self, force: f32, torque: f32) -> bool {
        let past = |limit: Option<f32>, load: f32| limit.is_some_and(|limit| load.abs() > limit);
        past(self.break_force, force) || past(self.break_torque, torque)
    }
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

    #[test]
    fn every_kind_and_a_break_round_trip() {
        for name in JointKind::NAMES {
            let joint = Joint2D {
                kind: JointKind::named(name),
                break_force: Some(50.0),
                break_torque: Some(5.0),
                ..Joint2D::default()
            };
            assert_eq!(joint.kind.name(), name);
            let text = ron::to_string(&joint).expect("writes");
            let back: Joint2D = ron::from_str(&text).expect("reads");
            assert_eq!(back, joint, "{text}");
        }
    }

    #[test]
    fn a_file_from_before_the_new_kinds_reads_as_it_did() {
        // What a 0.3 scene holds: no spring on the revolute, no thresholds.
        let old = "(anchor_a: (1.0, 0.0), kind: Revolute(reference_angle: 0.5, \
                   motor: Some((speed: 1.0, max_torque: 2.0))))";
        let joint: Joint2D = ron::from_str(old).expect("reads");
        assert_eq!(
            joint.kind,
            JointKind::Revolute {
                reference_angle: 0.5,
                limit: None,
                motor: Some(Motor {
                    speed: 1.0,
                    max_torque: 2.0
                }),
                spring: None,
            }
        );
        assert_eq!((joint.break_force, joint.break_torque), (None, None));
        let text = ron::to_string(&joint).expect("writes");
        assert!(
            !text.contains("break") && !text.contains("spring"),
            "{text}"
        );
    }

    #[test]
    fn a_line_joint_written_short_takes_its_kinds_axis() {
        let wheel: Joint2D = ron::from_str("(kind: Wheel())").expect("reads");
        assert!(
            matches!(wheel.kind, JointKind::Wheel { axis, spring: None, .. } if axis == Vec2::Y)
        );
        let slider: Joint2D = ron::from_str("(kind: Prismatic())").expect("reads");
        assert!(matches!(slider.kind, JointKind::Prismatic { axis, .. } if axis == Vec2::X));
    }

    #[test]
    fn a_threshold_breaks_only_past_it() {
        let joint = Joint2D {
            break_force: Some(10.0),
            ..Joint2D::default()
        };
        assert!(
            !joint.breaks_under(10.0, 1e9),
            "at it holds; no torque limit"
        );
        assert!(joint.breaks_under(-10.5, 0.0), "either way");
        assert!(!Joint2D::default().breaks_under(f32::MAX, f32::MAX));
        let nan = Joint2D {
            break_torque: Some(f32::NAN),
            ..Joint2D::default()
        };
        assert!(!nan.breaks_under(0.0, 1e9), "not a number never breaks");
    }
}
