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

mod impulses;
mod kind;
mod parts;

pub use impulses::JointImpulses;
pub use kind::JointKind;
pub use parts::{AngleLimit, Motor, Spring};

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
    /// Which constraint this is, and its own parameters.
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
