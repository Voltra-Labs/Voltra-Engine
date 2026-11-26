//! Mixing poses by weight: what a crossfade writes, and the one mechanism
//! anything that plays several clips at once mixes them through.

use std::f32::consts::{PI, TAU};

use super::pose::Pose;
use super::property::Property;
use super::value::Value;

/// How a property's values mix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mixing {
    /// Each number weighted and summed: positions, scales, colours,
    /// intensities.
    Linear,
    /// An angle in radians, the short way round: `350°` and `10°` mix
    /// through `0°`, not through `180°`.
    Angle,
    /// No value between two: the heaviest input's — a frame, a switch.
    Discrete,
}

/// The weighted mix of `inputs`: one value for every (target, property)
/// some input of positive weight holds, in the order first met.
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
    let mut parts = Vec::with_capacity(inputs.len());
    for (at, (pose, _)) in inputs.iter().enumerate() {
        for sample in pose.samples() {
            let (target, property) = (&sample.target, sample.property);
            let earlier = inputs[..at]
                .iter()
                .any(|(p, _)| p.get(target, property).is_some());
            if earlier {
                continue;
            }
            parts.clear();
            parts.extend(
                inputs[at..]
                    .iter()
                    .filter_map(|(p, w)| p.get(target, property).map(|v| (v, *w))),
            );
            if let Some(value) = mix(property, &parts) {
                out.set(target.clone(), property, value);
            }
        }
    }
    out
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
/// value exactly.
pub fn mix(property: Property, parts: &[(Value, f32)]) -> Option<Value> {
    let kind = property.kind();
    let parts: Vec<(Value, f32)> = parts
        .iter()
        .filter(|(v, w)| v.kind() == kind && w.is_finite() && *w > 0.0)
        .copied()
        .collect();
    let (first, rest) = parts.split_first()?;
    if rest.is_empty() {
        return Some(first.0);
    }
    // Over the heaviest first, so a sum of large weights stays finite.
    let most = parts.iter().map(|(_, w)| *w).fold(0.0, f32::max);
    let total: f32 = parts.iter().map(|(_, w)| w / most).sum();
    let share = |w: f32| w / most / total;
    let mut heaviest = *first;
    for &(v, w) in rest {
        if w >= heaviest.1 {
            heaviest = (v, w);
        }
    }
    let heaviest = heaviest.0;
    Some(match (property.mixing(), heaviest, first.0) {
        (Mixing::Discrete, ..) | (_, Value::Int(_), _) => heaviest,
        (Mixing::Angle, Value::Float(top), Value::Float(from)) => {
            let turn: f32 = parts
                .iter()
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
            parts
                .iter()
                .map(|&(v, w)| match v {
                    Value::Float(a) => a * share(w),
                    _ => 0.0,
                })
                .sum(),
        ),
        (_, Value::Vec2(_), _) => Value::Vec2(
            parts
                .iter()
                .map(|&(v, w)| match v {
                    Value::Vec2(a) => a * share(w),
                    _ => voltra_render::glam::Vec2::ZERO,
                })
                .sum(),
        ),
        (_, Value::Color(_), _) => {
            let mut sum = [0.0; 4];
            for &(v, w) in &parts {
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
mod tests {
    use super::*;
    use voltra_render::glam::Vec2;

    fn pose(entries: &[(&'static str, Property, Value)]) -> Pose<'static> {
        let mut pose = Pose::new();
        for &(target, property, value) in entries {
            pose.set(target, property, value);
        }
        pose
    }

    fn close(a: Option<Value>, b: Value) -> bool {
        match (a, b) {
            (Some(Value::Float(a)), Value::Float(b)) => (a - b).abs() < 1e-5,
            (Some(Value::Vec2(a)), Value::Vec2(b)) => (a - b).length() < 1e-5,
            (Some(Value::Color(a)), Value::Color(b)) => {
                a.iter().zip(b).all(|(a, b)| (a - b).abs() < 1e-5)
            }
            (a, b) => a == Some(b),
        }
    }

    #[test]
    fn numbers_vectors_and_colours_mix_halfway() {
        let out = pose(&[
            ("", Property::LightIntensity, Value::Float(0.0)),
            ("", Property::Translation, Value::Vec2(Vec2::ZERO)),
            (
                "",
                Property::SpriteColor,
                Value::Color([0.0, 0.0, 0.0, 1.0]),
            ),
        ]);
        let into = pose(&[
            ("", Property::LightIntensity, Value::Float(2.0)),
            ("", Property::Translation, Value::Vec2(Vec2::new(4.0, -2.0))),
            (
                "",
                Property::SpriteColor,
                Value::Color([1.0, 0.5, 0.0, 1.0]),
            ),
        ]);
        let mid = blend(&[(&out, 0.5), (&into, 0.5)]);
        assert!(close(
            mid.get("", Property::LightIntensity),
            Value::Float(1.0)
        ));
        assert!(close(
            mid.get("", Property::Translation),
            Value::Vec2(Vec2::new(2.0, -1.0))
        ));
        assert!(close(
            mid.get("", Property::SpriteColor),
            Value::Color([0.5, 0.25, 0.0, 1.0])
        ));
        let end = blend(&[(&out, 0.0), (&into, 1.0)]);
        assert_eq!(end, into, "all one input is that input exactly");
    }

    #[test]
    fn angles_mix_the_short_way_round() {
        let near_turn = 350f32.to_radians();
        let out = pose(&[("", Property::Rotation, Value::Float(near_turn))]);
        let into = pose(&[("", Property::Rotation, Value::Float(10f32.to_radians()))]);
        let Some(Value::Float(mid)) =
            blend(&[(&out, 0.5), (&into, 0.5)]).get("", Property::Rotation)
        else {
            panic!("a rotation");
        };
        let wrapped = mid.rem_euclid(TAU);
        assert!(
            wrapped < 1e-4 || TAU - wrapped < 1e-4,
            "through 0, not 180: {}",
            mid.to_degrees()
        );
        let end = blend(&[(&out, 0.0), (&into, 1.0)]);
        assert_eq!(
            end.get("", Property::Rotation),
            into.get("", Property::Rotation)
        );
    }

    #[test]
    fn angles_half_a_turn_apart_sweep_one_way_through_half_way() {
        let out = pose(&[("", Property::Rotation, Value::Float(0.0))]);
        let into = pose(&[("", Property::Rotation, Value::Float(PI))]);
        let at = |w: f32| match blend(&[(&out, 1.0 - w), (&into, w)]).get("", Property::Rotation) {
            Some(Value::Float(a)) => a,
            other => panic!("a rotation, not {other:?}"),
        };
        // The same direction either side of half way: no flip.
        let (before, after) = (at(0.49), at(0.51));
        let gap = shortest(after - before);
        assert!(
            gap.abs() < 0.1,
            "{} then {}",
            before.to_degrees(),
            after.to_degrees()
        );
        assert!(
            (shortest(before - -0.49 * PI)).abs() < 1e-4,
            "{}",
            before.to_degrees()
        );
        assert_eq!(at(1.0), PI, "ends on the input exactly");
        assert_eq!(at(0.0), 0.0);
    }

    #[test]
    fn an_input_of_weight_zero_is_absent() {
        let base = pose(&[("", Property::LightIntensity, Value::Float(1.0))]);
        let layer = pose(&[
            ("", Property::LightIntensity, Value::Float(9.0)),
            ("lamp", Property::LightRadius, Value::Float(5.0)),
        ]);
        let mixed = blend(&[(&base, 1.0), (&layer, 0.0)]);
        assert_eq!(
            mixed, base,
            "nothing of the layer, not even its own property"
        );
    }

    #[test]
    fn a_discrete_value_switches_at_half_weight() {
        let out = pose(&[("", Property::SpriteFrame, Value::Int(2))]);
        let into = pose(&[("", Property::SpriteFrame, Value::Int(7))]);
        let at = |w: f32| blend(&[(&out, 1.0 - w), (&into, w)]).get("", Property::SpriteFrame);
        assert_eq!(at(0.49), Some(Value::Int(2)));
        assert_eq!(at(0.5), Some(Value::Int(7)));
        assert_eq!(at(0.9), Some(Value::Int(7)));
    }

    #[test]
    fn a_property_only_one_input_holds_is_taken_at_full_weight() {
        let out = pose(&[
            ("", Property::Scale, Value::Vec2(Vec2::ONE)),
            ("lamp", Property::LightRadius, Value::Float(3.0)),
        ]);
        let into = pose(&[("", Property::Scale, Value::Vec2(Vec2::splat(3.0)))]);
        let mixed = blend(&[(&out, 0.75), (&into, 0.25)]);
        assert_eq!(mixed.len(), 2);
        assert_eq!(
            mixed.get("lamp", Property::LightRadius),
            Some(Value::Float(3.0))
        );
        assert!(close(
            mixed.get("", Property::Scale),
            Value::Vec2(Vec2::splat(1.5))
        ));
    }

    #[test]
    fn many_inputs_weigh_in_and_bad_weights_count_as_none() {
        let a = pose(&[("", Property::LightIntensity, Value::Float(0.0))]);
        let b = pose(&[("", Property::LightIntensity, Value::Float(3.0))]);
        let c = pose(&[("", Property::LightIntensity, Value::Float(6.0))]);
        let mixed = blend(&[(&a, 1.0), (&b, 1.0), (&c, 2.0), (&a, f32::NAN)]);
        assert!(close(
            mixed.get("", Property::LightIntensity),
            Value::Float(3.75)
        ));
        let none = blend(&[(&a, 0.0), (&c, -1.0)]);
        assert!(none.is_empty(), "weighing nothing is absent");
        assert!(blend(&[]).is_empty());
        assert_eq!(mix(Property::Rotation, &[(Value::Int(1), 1.0)]), None);
    }
}
