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
