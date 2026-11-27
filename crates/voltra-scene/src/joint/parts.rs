//! The parts a joint kind is built from: a limit, a motor, a spring.

use serde::{Deserialize, Serialize};

/// How far a revolute joint may turn.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AngleLimit {
    /// Radians, at most `upper`.
    pub lower: f32,
    /// Radians, at least `lower`.
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

/// A motor that turns: a revolute joint's, or a wheel's — what drives a
/// car. A prismatic joint's slides: a [`LinearMotor`].
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

/// How far a prismatic or wheel joint may slide along its axis.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TranslationLimit {
    /// World units along the axis from where the anchors meet, at most
    /// `upper`.
    pub lower: f32,
    /// World units along the axis, at least `lower`.
    pub upper: f32,
}

impl Default for TranslationLimit {
    /// Half a unit either way.
    fn default() -> Self {
        Self {
            lower: -0.5,
            upper: 0.5,
        }
    }
}

/// A prismatic joint's motor: it slides rather than turns.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LinearMotor {
    /// The speed it drives `a` at along the axis against `b`, world units
    /// per second.
    pub speed: f32,
    /// The most force it may use getting there.
    pub max_force: f32,
}

impl Default for LinearMotor {
    /// A unit a second, with the force to lift a small body.
    fn default() -> Self {
        Self {
            speed: 1.0,
            max_force: 100.0,
        }
    }
}

/// A spring pulling a joint's angle (revolute) or its slide along the axis
/// (prismatic, wheel) towards `rest`.
///
/// A distance joint's spring is a [`Spring`]: it rests at the joint's
/// length.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RestSpring {
    /// How fast it springs back, in hertz.
    pub hertz: f32,
    /// `0` bounces for ever, `1` settles without overshooting.
    pub damping_ratio: f32,
    /// Where it rests: radians from the reference angle for a revolute,
    /// world units along the axis for a prismatic or a wheel.
    #[serde(default)]
    pub rest: f32,
}

impl Default for RestSpring {
    /// A car's suspension: firm, and settling without much bounce.
    fn default() -> Self {
        Self {
            hertz: 4.0,
            damping_ratio: 0.7,
            rest: 0.0,
        }
    }
}
