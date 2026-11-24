//! A colour that changes over `0..=1`: a particle's colour over its life.
//!
//! Colour stops, linear RGBA between them — Unity's `Gradient` and Godot's
//! `Gradient` resource without their separate alpha keys: one stop carries
//! all four channels, which is what a stop picked in a colour picker is.

use serde::{Deserialize, Serialize};

/// One stop of a [`Gradient`].
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct GradientKey {
    /// Where along `0..=1` the stop sits.
    pub t: f32,
    /// Linear RGBA there.
    pub color: [f32; 4],
}

impl GradientKey {
    /// A stop of `color` at `t`.
    pub const fn new(t: f32, color: [f32; 4]) -> Self {
        Self { t, color }
    }

    fn is_finite(&self) -> bool {
        self.t.is_finite() && self.color.iter().all(|c| c.is_finite())
    }
}

/// Colours over `0..=1`, from stops.
///
/// Read as a [`Curve`](crate::curve::Curve) is: stops in any order, broken
/// ones skipped, flat past the ends. A gradient with no stop is `None`
/// wherever it is sampled, so its owner falls back to what it had before.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Gradient {
    /// The stops, in any order.
    pub keys: Vec<GradientKey>,
}

impl Gradient {
    /// A gradient through `keys`.
    pub fn new(keys: Vec<GradientKey>) -> Self {
        Self { keys }
    }

    /// From `from` at `0` to `to` at `1`.
    pub fn linear(from: [f32; 4], to: [f32; 4]) -> Self {
        Self::new(vec![GradientKey::new(0.0, from), GradientKey::new(1.0, to)])
    }

    /// The colour at `t`, clamped into `0..=1`, or `None` with no stop.
    pub fn sample(&self, t: f32) -> Option<[f32; 4]> {
        let t = if t.is_nan() { 0.0 } else { t.clamp(0.0, 1.0) };
        let mut before: Option<GradientKey> = None;
        let mut after: Option<GradientKey> = None;
        for key in self.keys.iter().filter(|k| k.is_finite()) {
            if key.t <= t {
                if before.is_none_or(|b| key.t >= b.t) {
                    before = Some(*key);
                }
            } else if after.is_none_or(|a| key.t < a.t) {
                after = Some(*key);
            }
        }
        Some(match (before, after) {
            (None, None) => return None,
            (Some(key), None) | (None, Some(key)) => key.color,
            (Some(a), Some(b)) => {
                let span = b.t - a.t;
                let local = if span > 0.0 { (t - a.t) / span } else { 1.0 };
                std::array::from_fn(|i| a.color[i] + (b.color[i] - a.color[i]) * local)
            }
        })
    }

    /// The stops in `t` order, broken ones left out.
    pub fn sorted(&self) -> Vec<GradientKey> {
        let mut keys: Vec<GradientKey> = self
            .keys
            .iter()
            .copied()
            .filter(GradientKey::is_finite)
            .collect();
        keys.sort_by(|a, b| a.t.total_cmp(&b.t));
        keys
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RED: [f32; 4] = [1.0, 0.0, 0.0, 1.0];
    const CLEAR: [f32; 4] = [0.0, 0.0, 1.0, 0.0];

    #[test]
    fn an_empty_gradient_has_no_colour() {
        assert_eq!(Gradient::default().sample(0.5), None);
    }

    #[test]
    fn stops_blend_linearly_and_hold_past_the_ends() {
        let gradient = Gradient::new(vec![
            GradientKey::new(0.75, CLEAR),
            GradientKey::new(0.25, RED),
        ]);
        assert_eq!(gradient.sample(0.0), Some(RED));
        assert_eq!(gradient.sample(0.5), Some([0.5, 0.0, 0.5, 0.5]));
        assert_eq!(gradient.sample(1.0), Some(CLEAR));
    }

    #[test]
    fn a_broken_stop_is_skipped() {
        let gradient = Gradient::new(vec![
            GradientKey::new(0.0, RED),
            GradientKey::new(0.5, [f32::NAN; 4]),
        ]);
        assert_eq!(gradient.sample(0.9), Some(RED));
        assert_eq!(gradient.sorted().len(), 1);
    }
}
