//! A number that changes over `0..=1`: a particle's size over its life, a
//! line's width along its length.
//!
//! Keys at a position `t` with a value, each with the [`Ease`] of the
//! segment that leaves it — the vocabulary the animation clips already use
//! (stage 66), so a curve and a clip read the same. Unity's
//! `AnimationCurve` and Godot's `Curve` add Bézier tangents; those join as
//! an `Ease` variant when a game needs one.

use serde::{Deserialize, Serialize};

use crate::keyframes::Ease;

/// One point of a [`Curve`].
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CurveKey {
    /// Where along `0..=1` the key sits.
    pub t: f32,
    /// The value there.
    pub value: f32,
    /// The shape of the segment from this key to the next.
    #[serde(default, skip_serializing_if = "is_linear")]
    pub ease: Ease,
}

fn is_linear(ease: &Ease) -> bool {
    *ease == Ease::Linear
}

impl CurveKey {
    /// A key of `value` at `t`, linear to the next.
    pub const fn new(t: f32, value: f32) -> Self {
        Self {
            t,
            value,
            ease: Ease::Linear,
        }
    }

    fn is_finite(&self) -> bool {
        self.t.is_finite() && self.value.is_finite()
    }
}

/// A value over `0..=1`, from keys.
///
/// Sampled leniently: keys need not be sorted (they are read in `t`
/// order), a key that is not finite is skipped, before the first key is the
/// first key's value and after the last the last's. A curve with no key is
/// `1` everywhere — the multiplier that changes nothing, which is what every
/// curve here is used as.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Curve {
    /// The keys, in any order.
    pub keys: Vec<CurveKey>,
}

impl Curve {
    /// A curve through `keys`.
    pub fn new(keys: Vec<CurveKey>) -> Self {
        Self { keys }
    }

    /// The same value everywhere.
    pub fn constant(value: f32) -> Self {
        Self::new(vec![CurveKey::new(0.0, value)])
    }

    /// A straight line from `from` at `0` to `to` at `1`.
    pub fn linear(from: f32, to: f32) -> Self {
        Self::new(vec![CurveKey::new(0.0, from), CurveKey::new(1.0, to)])
    }

    /// The value at `t`, clamped into `0..=1`. A NaN `t` is `0`.
    pub fn sample(&self, t: f32) -> f32 {
        let t = if t.is_nan() { 0.0 } else { t.clamp(0.0, 1.0) };
        let mut before: Option<CurveKey> = None;
        let mut after: Option<CurveKey> = None;
        for key in self.keys.iter().filter(|k| k.is_finite()) {
            if key.t <= t {
                // The latest key at or before `t`; of two at one place, the
                // later in the list — the one an editor appended last.
                if before.is_none_or(|b| key.t >= b.t) {
                    before = Some(*key);
                }
            } else if after.is_none_or(|a| key.t < a.t) {
                after = Some(*key);
            }
        }
        match (before, after) {
            (None, None) => 1.0,
            (Some(key), None) | (None, Some(key)) => key.value,
            (Some(a), Some(b)) => {
                let span = b.t - a.t;
                let local = if span > 0.0 { (t - a.t) / span } else { 1.0 };
                a.value + (b.value - a.value) * a.ease.apply(local)
            }
        }
    }

    /// The keys in `t` order, the ones that are not finite left out: what an
    /// editor draws.
    pub fn sorted(&self) -> Vec<CurveKey> {
        let mut keys: Vec<CurveKey> = self
            .keys
            .iter()
            .copied()
            .filter(CurveKey::is_finite)
            .collect();
        keys.sort_by(|a, b| a.t.total_cmp(&b.t));
        keys
    }

    /// The smallest and largest value the curve takes — at its keys, since
    /// no ease overshoots. `(1, 1)` for a curve with no key.
    pub fn range(&self) -> (f32, f32) {
        let keys = self.sorted();
        if keys.is_empty() {
            return (1.0, 1.0);
        }
        keys.iter()
            .fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), k| {
                (lo.min(k.value), hi.max(k.value))
            })
    }

    /// `samples` values evenly spaced from `t = 0` to `t = 1`, both ends
    /// included: the table a shader reads in place of the keys.
    pub fn table(&self, samples: usize) -> Vec<f32> {
        table(samples, |t| self.sample(t))
    }
}

/// `samples` values of `f` evenly spaced over `0..=1`, both ends included;
/// one sample is the value at `0`.
pub(crate) fn table<T>(samples: usize, f: impl Fn(f32) -> T) -> Vec<T> {
    let last = samples.saturating_sub(1).max(1) as f32;
    (0..samples).map(|i| f(i as f32 / last)).collect()
}

#[cfg(test)]
mod tests;
