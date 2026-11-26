//! What a joint holds: its kind, and the parameters each kind has.

use serde::{Deserialize, Serialize};
use voltra_render::glam::Vec2;

use super::{AngleLimit, LinearMotor, Motor, RestSpring, Spring, TranslationLimit};

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
        /// Pulls the angle towards the spring's rest — radians from the
        /// reference — a door closing itself, a flipper returning.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        spring: Option<RestSpring>,
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
    /// A wheel on a suspension: `a`'s anchor kept on a line through `b`'s
    /// along `axis`, sprung along it, and free to turn — Box2D's
    /// `b2WheelJoint`. The joint lives on the wheel (`a`), held to the
    /// chassis (`b`) or the world.
    Wheel {
        /// The line the suspension travels along, in `b`'s space — the
        /// world's when `b` is the world. Up by default. Its length does not
        /// matter; a zero axis joins nothing.
        #[serde(default = "up")]
        axis: Vec2,
        /// The suspension, pulling the wheel along the axis towards the
        /// spring's rest; `None` leaves it free to slide.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        spring: Option<RestSpring>,
        /// How far it may travel along the axis.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        limit: Option<TranslationLimit>,
        /// A motor turning the wheel against `b` — what drives a car.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        motor: Option<Motor>,
    },
    /// A slider: `a` kept on a line through `b`'s anchor along `axis` and
    /// at the angle it stands at, free only to slide — a piston, a lift, a
    /// drawer (Box2D's `b2PrismaticJoint`).
    Prismatic {
        /// The line it slides along, in `b`'s space — the world's when `b`
        /// is the world. Right by default; a zero axis joins nothing.
        #[serde(default = "right")]
        axis: Vec2,
        /// `a`'s rotation less `b`'s that is kept.
        #[serde(default)]
        reference_angle: f32,
        /// How far it may slide, along the axis from where the anchors meet.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        limit: Option<TranslationLimit>,
        /// A motor sliding `a` along the axis against `b`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        motor: Option<LinearMotor>,
        /// Pulls the slide towards the spring's rest along the axis.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        spring: Option<RestSpring>,
    },
    /// `a`'s anchor pulled towards `b`'s — the target, a point in the world
    /// when `b` is the world — by a spring no stronger than `max_force`:
    /// Box2D's `b2MouseJoint`, what dragging a body with the pointer is.
    /// Move the target by moving `anchor_b`.
    Mouse {
        /// The most force it pulls with, in newtons: past it the body lags
        /// the target rather than being yanked through what is in the way.
        max_force: f32,
        /// How stiff the pull is, in hertz; `0` pulls as hard as a joint
        /// holds its anchors.
        hertz: f32,
        /// `0` overshoots and swings about the target, `1` settles on it.
        damping_ratio: f32,
    },
}

fn up() -> Vec2 {
    Vec2::Y
}

fn right() -> Vec2 {
    Vec2::X
}

impl Default for JointKind {
    fn default() -> Self {
        Self::Revolute {
            reference_angle: 0.0,
            limit: None,
            motor: None,
            spring: None,
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
            Self::Wheel { .. } => "wheel",
            Self::Prismatic { .. } => "prismatic",
            Self::Mouse { .. } => "mouse",
        }
    }

    /// Every kind's [`name`](Self::name), in the order an editor lists them.
    pub const NAMES: [&'static str; 6] = [
        "revolute",
        "distance",
        "weld",
        "wheel",
        "prismatic",
        "mouse",
    ];

    /// The kind called `name`, with the parameters a new one starts from;
    /// a revolute for a name that is none of [`NAMES`](Self::NAMES).
    pub fn named(name: &str) -> Self {
        match name {
            "distance" => Self::rod(1.0),
            "weld" => Self::Weld {
                reference_angle: 0.0,
            },
            "wheel" => Self::wheel(),
            "prismatic" => Self::prismatic(Vec2::X),
            "mouse" => Self::mouse(),
            _ => Self::default(),
        }
    }

    /// A wheel on a suspension travelling up and down, sprung at its rest
    /// and turning freely.
    pub fn wheel() -> Self {
        Self::Wheel {
            axis: Vec2::Y,
            spring: Some(RestSpring::default()),
            limit: None,
            motor: None,
        }
    }

    /// A slider along `axis`, free to slide as far as it is pushed.
    pub fn prismatic(axis: Vec2) -> Self {
        Self::Prismatic {
            axis,
            reference_angle: 0.0,
            limit: None,
            motor: None,
            spring: None,
        }
    }

    /// A pull towards the target with Box2D's testbed tuning: five hertz,
    /// damped to settle, a thousand newtons at most.
    pub fn mouse() -> Self {
        Self::Mouse {
            max_force: 1000.0,
            hertz: 5.0,
            damping_ratio: 0.7,
        }
    }

    /// Whether a motor drives it, or a hand pulls it: its bodies' island
    /// then never sleeps.
    pub fn is_driven(&self) -> bool {
        match self {
            Self::Revolute { motor: Some(m), .. } | Self::Wheel { motor: Some(m), .. } => {
                m.speed != 0.0 && m.max_torque > 0.0
            }
            Self::Prismatic { motor: Some(m), .. } => m.speed != 0.0 && m.max_force > 0.0,
            Self::Mouse { max_force, .. } => *max_force > 0.0,
            _ => false,
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
