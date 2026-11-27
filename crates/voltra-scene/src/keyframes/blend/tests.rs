//! The mixer's rules: halves, angles, discrete values, absent inputs.

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
    let Some(Value::Float(mid)) = blend(&[(&out, 0.5), (&into, 0.5)]).get("", Property::Rotation)
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
