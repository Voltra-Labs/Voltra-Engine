//! How a value moves from one key to the next.

use serde::{Deserialize, Serialize};

/// The shape of the segment that starts at a key.
///
/// On the key the segment *leaves*, as Godot's per-key transition is and
/// Unity's out-tangent half is: "ease out of this pose" is said where the
/// pose is. A closed set of shapes rather than Bézier tangents — every 2D
/// tween library's vocabulary, and nothing an author has to drag handles
/// for. Tangents are a curve editor's, and can join this as a variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Ease {
    /// Holds the key's value until the next key: a light that switches, a
    /// frame that cuts.
    Step,
    /// A straight line.
    #[default]
    Linear,
    /// Slow out of the key, fast into the next.
    EaseIn,
    /// Fast out of the key, slow into the next.
    EaseOut,
    /// Slow at both ends — smoothstep.
    EaseInOut,
}

impl Ease {
    /// Every shape, in the order an editor lists them.
    pub const ALL: [Self; 5] = [
        Self::Step,
        Self::Linear,
        Self::EaseIn,
        Self::EaseOut,
        Self::EaseInOut,
    ];

    /// How far along the segment the value is, `0` to `1`, at `t` of the
    /// way through its time. `t` outside `0..=1` is clamped: no shape here
    /// overshoots, so a value never leaves the range of its two keys.
    pub fn apply(self, t: f32) -> f32 {
        let t = if t.is_nan() { 0.0 } else { t.clamp(0.0, 1.0) };
        match self {
            Self::Step => 0.0,
            Self::Linear => t,
            Self::EaseIn => t * t,
            Self::EaseOut => t * (2.0 - t),
            Self::EaseInOut => t * t * (3.0 - 2.0 * t),
        }
    }

    /// What an editor calls it.
    pub fn label(self) -> &'static str {
        match self {
            Self::Step => "step",
            Self::Linear => "linear",
            Self::EaseIn => "ease in",
            Self::EaseOut => "ease out",
            Self::EaseInOut => "ease in-out",
        }
    }

    pub(super) fn is_linear(&self) -> bool {
        *self == Self::Linear
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_shape_starts_at_zero_and_ends_at_one_but_step() {
        for ease in Ease::ALL {
            assert_eq!(ease.apply(0.0), 0.0, "{ease:?}");
            let end = if ease == Ease::Step { 0.0 } else { 1.0 };
            assert!((ease.apply(1.0) - end).abs() < 1e-6, "{ease:?}");
            assert!((0.0..=1.0).contains(&ease.apply(0.5)), "{ease:?}");
        }
        assert!(Ease::EaseIn.apply(0.5) < 0.5 && Ease::EaseOut.apply(0.5) > 0.5);
        assert_eq!(Ease::Linear.apply(7.0), 1.0, "clamped");
        assert_eq!(Ease::Linear.apply(f32::NAN), 0.0);
    }
}
