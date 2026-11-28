//! Mixing poses by weight: what a crossfade writes, and the one mechanism
//! anything that plays several clips at once mixes them through.

use std::borrow::Cow;
use std::f32::consts::{PI, TAU};

use voltra_render::glam::{Quat, Vec2, Vec3};

use super::pose::{Pose, Sample};
use super::property::Property;
use super::value::Value;
use crate::Interpolate;

/// How a property's values mix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mixing {
    /// Each number weighted and summed: positions, scales, colours,
    /// intensities.
    Linear,
    /// An angle in radians, the short way round: `350°` and `10°` mix
    /// through `0°`, not through `180°`.
    Angle,
    /// A 3D rotation, the short way round: `slerp`, and for more than two
    /// inputs each one slerped in by its share of the weight so far.
    Quaternion,
    /// No value between two: the heaviest input's — a frame, a switch.
    Discrete,
}

/// The weighted mix of `inputs`: one value for every (target, property)
/// some input of positive weight holds.
///
/// Each property is mixed among only the inputs that hold it, their
/// weights scaled to sum to one among those — so a property one clip
/// animates and another does not is that clip's at full weight, never
/// pulled towards a default the other does not state. N inputs rather than
/// two: a crossfade is two, a blend tree several.
///
/// The weights are shares of one whole — a crossfade's `1 - w` and `w` —
/// not opacities: an input of weight zero (or a weight that is not a finite
/// number) is absent, and a property only such inputs hold is not in the
/// mix at all. Layering one pose *over* another, where a weight is how much
/// of the lower one shows through, is a composite on top of this, not an
/// input to it.
pub fn blend<'a>(inputs: &[(&Pose<'a>, f32)]) -> Pose<'a> {
    let mut out = Pose::new();
    blend_into(inputs, &mut out);
    out
}

/// Inputs a blend walks without allocating; more take a buffer each call.
const FEW: usize = 8;

/// [`blend`] into `out`, emptied first and its room reused — what a frame
/// of players mixes through, one buffer for them all.
///
/// One pass over the inputs side by side: a pose is sorted, so the next
/// property to mix is the least any input has not passed, and the inputs
/// holding it are the ones there. Linear in the values held, times the
/// inputs; nothing is allocated per property, nor for up to eight inputs.
/// A target is borrowed from an input that borrows it, so mixing clips
/// copies no names.
pub fn blend_into<'a>(inputs: &[(&Pose<'a>, f32)], out: &mut Pose<'a>) {
    out.clear();
    let (mut few_at, mut many_at) = ([0usize; FEW], Vec::new());
    let (mut few_parts, mut many_parts) = ([(Value::Int(0), 0.0); FEW], Vec::new());
    let (at, parts): (&mut [usize], &mut [(Value, f32)]) = if inputs.len() <= FEW {
        (&mut few_at[..inputs.len()], &mut few_parts[..])
    } else {
        many_at.resize(inputs.len(), 0);
        many_parts.resize(inputs.len(), (Value::Int(0), 0.0));
        (&mut many_at, &mut many_parts)
    };
    loop {
        let mut next: Option<(&str, Property)> = None;
        for (&(pose, _), &i) in inputs.iter().zip(at.iter()) {
            if let Some(s) = pose.samples().get(i) {
                let key = (s.target.as_ref(), s.property);
                if next.is_none_or(|least| key < least) {
                    next = Some(key);
                }
            }
        }
        let Some((target, property)) = next else {
            return;
        };
        let mut held = 0;
        let mut name: Option<&Cow<'a, str>> = None;
        for (&(pose, weight), i) in inputs.iter().zip(at.iter_mut()) {
            let Some(s) = pose.samples().get(*i) else {
                continue;
            };
            if s.property != property || s.target != target {
                continue;
            }
            parts[held] = (s.value, weight);
            held += 1;
            *i += 1;
            if name.is_none_or(|n| matches!(n, Cow::Owned(_))) {
                name = Some(&s.target);
            }
        }
        if let (Some(value), Some(name)) = (mix(property, &parts[..held]), name) {
            out.push_sorted(Sample {
                target: name.clone(),
                property,
                value,
            });
        }
    }
}

/// `parts` — values of `property` and their weights — mixed as the
/// property's [`Mixing`] says. `None` when none is of the property's kind
/// with a positive weight.
///
/// A discrete property takes the heaviest part's value — the later of two
/// that weigh the same, so a crossfade switches at half way. An angle is
/// mixed the short way from the *first* part, a fixed side whatever the
/// weights, so two angles half a turn apart sweep one way through the
/// whole fade rather than flipping at half way; one part alone is its own
/// value exactly. A 3D rotation is likewise mixed from the first part:
/// each further one slerped in by its share of the weight met so far —
/// exactly `slerp` for two, and for more an order-dependent but continuous
/// mix (Unity's and Unreal's blend trees accumulate the same way).
pub fn mix(property: Property, parts: &[(Value, f32)]) -> Option<Value> {
    let kind = property.kind();
    let fits = |&&(v, w): &&(Value, f32)| v.kind() == kind && w.is_finite() && w > 0.0;
    let valid = || parts.iter().filter(fits);
    let mut iter = valid();
    let first = *iter.next()?;
    let mut heaviest = first;
    let mut alone = true;
    for &(v, w) in iter {
        alone = false;
        if w >= heaviest.1 {
            heaviest = (v, w);
        }
    }
    if alone {
        return Some(first.0);
    }
    // Over the heaviest first, so a sum of large weights stays finite.
    let most = valid().map(|(_, w)| *w).fold(0.0, f32::max);
    let total: f32 = valid().map(|(_, w)| w / most).sum();
    let share = |w: f32| w / most / total;
    let heaviest = heaviest.0;
    Some(match (property.mixing(), heaviest, first.0) {
        (Mixing::Discrete, ..) | (_, Value::Int(_), _) => heaviest,
        (_, Value::Quat(_), _) => {
            let (mut turned, mut met): (Option<Quat>, f32) = (None, 0.0);
            for &(v, w) in valid() {
                if let Value::Quat(q) = v {
                    met += share(w);
                    turned = Some(match turned {
                        None => q,
                        Some(t) if met > 0.0 => t.interpolate(&q, share(w) / met),
                        Some(t) => t,
                    });
                }
            }
            Value::Quat(turned.unwrap_or(Quat::IDENTITY))
        }
        (Mixing::Angle, Value::Float(top), Value::Float(from)) => {
            let turn: f32 = valid()
                .map(|&(v, w)| match v {
                    Value::Float(a) => shortest(a - from) * share(w),
                    _ => 0.0,
                })
                .sum();
            // The same angle, wound as the heaviest part is, so a fade
            // ending on it does not end a turn away.
            Value::Float(top + shortest(from + turn - top))
        }
        (_, Value::Float(_), _) => Value::Float(
            valid()
                .map(|&(v, w)| match v {
                    Value::Float(a) => a * share(w),
                    _ => 0.0,
                })
                .sum(),
        ),
        (_, Value::Vec2(_), _) => Value::Vec2(
            valid()
                .map(|&(v, w)| match v {
                    Value::Vec2(a) => a * share(w),
                    _ => Vec2::ZERO,
                })
                .sum(),
        ),
        (_, Value::Vec3(_), _) => Value::Vec3(
            valid()
                .map(|&(v, w)| match v {
                    Value::Vec3(a) => a * share(w),
                    _ => Vec3::ZERO,
                })
                .sum(),
        ),
        (_, Value::Color(_), _) => {
            let mut sum = [0.0; 4];
            for &(v, w) in valid() {
                if let Value::Color(c) = v {
                    for (s, c) in sum.iter_mut().zip(c) {
                        *s += c * share(w);
                    }
                }
            }
            Value::Color(sum)
        }
    })
}

/// `delta` radians wrapped into `[-π, π)`: the short way round.
fn shortest(delta: f32) -> f32 {
    (delta + PI).rem_euclid(TAU) - PI
}

#[cfg(test)]
mod tests;
