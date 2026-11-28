//! What every 3D light shares: its colour and energy, their defaults and
//! the check that they can be drawn with, and its layers.

/// White: a light's default colour.
pub(crate) fn white() -> [f32; 3] {
    [1.0; 3]
}

pub(crate) fn is_white(color: &[f32; 3]) -> bool {
    *color == white()
}

/// 1: a light's default energy, a usable light in simple units.
pub(crate) fn one() -> f32 {
    1.0
}

pub(crate) fn is_one(energy: &f32) -> bool {
    *energy == 1.0
}

/// What a colour and energy a light cannot draw with make of it, if
/// anything: negative or not a number.
pub(crate) fn problem(color: &[f32; 3], energy: f32) -> Option<&'static str> {
    if !color.iter().all(|c| c.is_finite() && *c >= 0.0) {
        Some("its colour is negative or not a number; it lights nothing")
    } else if !energy.is_finite() || energy < 0.0 {
        Some("its energy is negative or not a number; it lights nothing")
    } else {
        None
    }
}

/// What a range a point or spot light cannot draw with makes of it.
pub(crate) fn range_problem(range: f32) -> Option<&'static str> {
    (!range.is_finite() || range < 0.0)
        .then_some("its range is negative or not a number; it lights nothing")
}

/// How bright a light is at its brightest channel: what choosing the lights
/// that matter most weighs it by.
pub(crate) fn intensity(color: &[f32; 3], energy: f32) -> f32 {
    color.iter().fold(0.0f32, |m, c| m.max(*c)) * energy
}
