//! What a joint holds: its kind, and the parameters each kind has.

use serde::{Deserialize, Serialize};

use super::{AngleLimit, Motor, Spring};

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
        /// The longest distance held; see `min`.
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
