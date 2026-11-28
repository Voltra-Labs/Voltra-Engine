//! Layering one pose over another: what a layer of a player writes, on top
//! of — not as one more input to — [`blend`](super::blend::blend).
//!
//! A blend's weights are shares of one whole, so a property only one input
//! holds is that input's at full weight. A layer's weight is an opacity:
//! how much of what is under it is covered. The two need two operations —
//! a layer at weight `0.01` over a property nothing under it animates must
//! move that property by a hundredth, not snap it.

use std::borrow::Cow;

use voltra_render::glam::{Quat, Vec2, Vec3};

use super::blend::{mix, Mixing};
use super::layer::LayerBlend;
use super::pose::{Pose, Sample};
use super::property::Property;
use super::value::Value;
use crate::Interpolate;

/// `layer` over `below`, at `weight`, as `blend` says.
///
/// Every property either holds is in the result: one only `below` holds is
/// its value; one both hold is `layer`'s over `below`'s ([`over`]). One only
/// `layer` holds has nothing under it here and is taken whole — so a
/// caller compositing over the world puts into `below`, first, the value
/// each such property has without the layer (a player does: see
/// [`AnimationPlayer`](super::AnimationPlayer)'s layers).
pub fn composite<'a>(
    below: &Pose<'a>,
    layer: &Pose<'a>,
    weight: f32,
    blend: LayerBlend,
) -> Pose<'a> {
    let mut out = Pose::new();
    composite_into(below, layer, weight, blend, &mut out);
    out
}

/// [`composite`] into `out`, emptied first and its room reused. One pass
/// over both, side by side: they are sorted.
pub fn composite_into<'a>(
    below: &Pose<'a>,
    layer: &Pose<'a>,
    weight: f32,
    blend: LayerBlend,
    out: &mut Pose<'a>,
) {
    out.clear();
    let (mut under, mut over_it) = (
        below.samples().iter().peekable(),
        layer.samples().iter().peekable(),
    );
    loop {
        let order = match (under.peek(), over_it.peek()) {
            (None, None) => return,
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (Some(a), Some(b)) => key(a).cmp(&key(b)),
        };
        let sample = match order {
            std::cmp::Ordering::Less => under.next().cloned(),
            std::cmp::Ordering::Greater => over_it.next().cloned(),
            std::cmp::Ordering::Equal => match (under.next(), over_it.next()) {
                (Some(a), Some(b)) => Some(Sample {
                    target: a.target.clone(),
                    property: a.property,
                    value: over(a.property, a.value, b.value, weight, blend),
                }),
                _ => None,
            },
        };
        if let Some(sample) = sample {
            out.push_sorted(sample);
        }
    }
}

fn key<'s>(sample: &'s Sample<'_>) -> (&'s str, Property) {
    (sample.target.as_ref(), sample.property)
}

/// `value` of `property` over `under`, at `weight` (clamped to `0..=1`, zero
/// when not a number), as `blend` says.
///
/// **Override** is `lerp(under, value, w)` by the property's mixing — an
/// angle the short way round, a discrete value switching at `w >= 0.5` —
/// exactly `under` at `0` and exactly `value` at `1`, continuous between.
/// **Additive** is `under + w * value`, `value` being a change (a delta from
/// the layer's reference, see [`subtract`]); a discrete property overrides,
/// since a frame has no sum. A 3D rotation adds by composing: the change
/// turned by `w` of its way from no turn (`slerp` from the identity), then
/// applied over `under` — `change^w · under`, Unreal's local-space additive.
/// Values of two kinds answer `under`.
pub fn over(
    property: Property,
    under: Value,
    value: Value,
    weight: f32,
    blend: LayerBlend,
) -> Value {
    let w = if weight.is_nan() {
        0.0
    } else {
        weight.clamp(0.0, 1.0)
    };
    let replace = || mix(property, &[(under, 1.0 - w), (value, w)]).unwrap_or(under);
    if blend == LayerBlend::Override || property.mixing() == Mixing::Discrete {
        return replace();
    }
    match (under, value) {
        (Value::Float(a), Value::Float(d)) => Value::Float(a + d * w),
        (Value::Vec2(a), Value::Vec2(d)) => Value::Vec2(a + d * w),
        (Value::Color(a), Value::Color(d)) => {
            Value::Color(std::array::from_fn(|i| a[i] + d[i] * w))
        }
        (Value::Vec3(a), Value::Vec3(d)) => Value::Vec3(a + d * w),
        (Value::Quat(a), Value::Quat(d)) => {
            // Unit times unit is unit to rounding; normalising would cost
            // `w = 0` its exactness (no layer, to the bit).
            Value::Quat(Quat::IDENTITY.interpolate(&d, w) * a)
        }
        (Value::Int(_), Value::Int(_)) => replace(),
        _ => under,
    }
}

/// Turns `pose` into its change from `reference`: each numeric value, of a
/// property that mixes as numbers, less the reference's value of it — what
/// an additive layer adds. A discrete value is left whole, and so is one the
/// reference does not hold. Both sorted, so one pass.
///
/// An angle is subtracted as numbers, not the short way: an additive clip
/// keyed from `0` to `2π` adds a whole turn, as the clip itself would turn.
/// A 3D rotation's change is the turn from the reference to it, `value ·
/// reference⁻¹`, which [`over`] applies on the left.
pub fn subtract(pose: &mut Pose<'_>, reference: &Pose<'_>) {
    let mut from = reference.samples().iter().peekable();
    for sample in pose.samples_mut() {
        while from.next_if(|r| key(r) < key(sample)).is_some() {}
        let Some(r) = from.next_if(|r| key(r) == key(sample)) else {
            continue;
        };
        if sample.property.mixing() == Mixing::Discrete {
            continue;
        }
        sample.value = match (sample.value, r.value) {
            (Value::Float(a), Value::Float(b)) => Value::Float(a - b),
            (Value::Vec2(a), Value::Vec2(b)) => Value::Vec2(a - b),
            (Value::Color(a), Value::Color(b)) => {
                Value::Color(std::array::from_fn(|i| a[i] - b[i]))
            }
            (Value::Vec3(a), Value::Vec3(b)) => Value::Vec3(a - b),
            (Value::Quat(a), Value::Quat(b)) => Value::Quat(a * b.conjugate()),
            (value, _) => value,
        };
    }
}

/// Gives `into` a change of nothing for every numeric property `from` holds
/// and it does not: inside an additive layer, a clip that does not animate a
/// property adds nothing to it, so a fade or a tree mixing two changes mixes
/// towards none — not, as [`blend`](super::blend::blend) would, holding the
/// other side's at full weight. Discrete properties are left one-sided.
pub(super) fn pad<'a, 'b: 'a>(into: &mut Pose<'a>, from: &Pose<'b>) {
    for s in from.samples() {
        if let Some(zero) = zero(s.property) {
            if into.get(&s.target, s.property).is_none() {
                into.set(s.target.clone(), s.property, zero);
            }
        }
    }
}

/// [`pad`] into a pose kept across frames, which owns its names.
pub(super) fn pad_owned(into: &mut Pose<'static>, from: &Pose<'_>) {
    for s in from.samples() {
        if let Some(zero) = zero(s.property) {
            if into.get(&s.target, s.property).is_none() {
                into.set(Cow::Owned(s.target.to_string()), s.property, zero);
            }
        }
    }
}

/// A change of nothing, of `property`'s kind; `None` for a discrete one.
fn zero(property: Property) -> Option<Value> {
    if property.mixing() == Mixing::Discrete {
        return None;
    }
    match property.kind() {
        super::ValueKind::Float => Some(Value::Float(0.0)),
        super::ValueKind::Vec2 => Some(Value::Vec2(Vec2::ZERO)),
        super::ValueKind::Color => Some(Value::Color([0.0; 4])),
        super::ValueKind::Vec3 => Some(Value::Vec3(Vec3::ZERO)),
        super::ValueKind::Quat => Some(Value::Quat(Quat::IDENTITY)),
        super::ValueKind::Int => None,
    }
}

#[cfg(test)]
mod tests;
