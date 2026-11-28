//! The mixer's rules: halves, angles, discrete values, absent inputs.

use super::*;
use voltra_render::glam::Vec2;

/// The angle between two rotations in radians, well conditioned near zero
/// (glam's `angle_between` is an `acos` of a dot product, and off by 10⁻³
/// there in `f32`).
fn apart(a: voltra_render::glam::Quat, b: voltra_render::glam::Quat) -> f32 {
    2.0 * (a * b.conjugate()).xyz().length().min(1.0).asin()
}

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
        ("", Property::PointLight2DIntensity, Value::Float(0.0)),
        ("", Property::Translation2D, Value::Vec2(Vec2::ZERO)),
        (
            "",
            Property::SpriteColor,
            Value::Color([0.0, 0.0, 0.0, 1.0]),
        ),
    ]);
    let into = pose(&[
        ("", Property::PointLight2DIntensity, Value::Float(2.0)),
        (
            "",
            Property::Translation2D,
            Value::Vec2(Vec2::new(4.0, -2.0)),
        ),
        (
            "",
            Property::SpriteColor,
            Value::Color([1.0, 0.5, 0.0, 1.0]),
        ),
    ]);
    let mid = blend(&[(&out, 0.5), (&into, 0.5)]);
    assert!(close(
        mid.get("", Property::PointLight2DIntensity),
        Value::Float(1.0)
    ));
    assert!(close(
        mid.get("", Property::Translation2D),
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
    let out = pose(&[("", Property::Rotation2D, Value::Float(near_turn))]);
    let into = pose(&[("", Property::Rotation2D, Value::Float(10f32.to_radians()))]);
    let Some(Value::Float(mid)) = blend(&[(&out, 0.5), (&into, 0.5)]).get("", Property::Rotation2D)
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
        end.get("", Property::Rotation2D),
        into.get("", Property::Rotation2D)
    );
}

#[test]
fn angles_half_a_turn_apart_sweep_one_way_through_half_way() {
    let out = pose(&[("", Property::Rotation2D, Value::Float(0.0))]);
    let into = pose(&[("", Property::Rotation2D, Value::Float(PI))]);
    let at = |w: f32| match blend(&[(&out, 1.0 - w), (&into, w)]).get("", Property::Rotation2D) {
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
    let base = pose(&[("", Property::PointLight2DIntensity, Value::Float(1.0))]);
    let layer = pose(&[
        ("", Property::PointLight2DIntensity, Value::Float(9.0)),
        ("lamp", Property::PointLight2DRadius, Value::Float(5.0)),
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
        ("", Property::Scale2D, Value::Vec2(Vec2::ONE)),
        ("lamp", Property::PointLight2DRadius, Value::Float(3.0)),
    ]);
    let into = pose(&[("", Property::Scale2D, Value::Vec2(Vec2::splat(3.0)))]);
    let mixed = blend(&[(&out, 0.75), (&into, 0.25)]);
    assert_eq!(mixed.len(), 2);
    assert_eq!(
        mixed.get("lamp", Property::PointLight2DRadius),
        Some(Value::Float(3.0))
    );
    assert!(close(
        mixed.get("", Property::Scale2D),
        Value::Vec2(Vec2::splat(1.5))
    ));
}

#[test]
fn many_inputs_weigh_in_and_bad_weights_count_as_none() {
    let a = pose(&[("", Property::PointLight2DIntensity, Value::Float(0.0))]);
    let b = pose(&[("", Property::PointLight2DIntensity, Value::Float(3.0))]);
    let c = pose(&[("", Property::PointLight2DIntensity, Value::Float(6.0))]);
    let mixed = blend(&[(&a, 1.0), (&b, 1.0), (&c, 2.0), (&a, f32::NAN)]);
    assert!(close(
        mixed.get("", Property::PointLight2DIntensity),
        Value::Float(3.75)
    ));
    let none = blend(&[(&a, 0.0), (&c, -1.0)]);
    assert!(none.is_empty(), "weighing nothing is absent");
    assert!(blend(&[]).is_empty());
    assert_eq!(mix(Property::Rotation2D, &[(Value::Int(1), 1.0)]), None);
}

#[test]
fn a_3d_rotation_mixes_by_slerp_the_short_way_and_a_position_linearly() {
    use voltra_render::glam::{Quat, Vec3};
    let a = Quat::from_rotation_y(170f32.to_radians());
    // -q: the same rotation as -170° about Y, the other sign — the short
    // way is still 20° through 180°.
    let b = -Quat::from_rotation_y(-170f32.to_radians());
    let out = pose(&[
        ("", Property::Rotation3D, Value::Quat(a)),
        ("", Property::Translation3D, Value::Vec3(Vec3::ZERO)),
    ]);
    let into = pose(&[
        ("", Property::Rotation3D, Value::Quat(b)),
        (
            "",
            Property::Translation3D,
            Value::Vec3(Vec3::new(2.0, 4.0, 6.0)),
        ),
    ]);
    let mixed = blend(&[(&out, 0.75), (&into, 0.25)]);
    let Some(Value::Quat(q)) = mixed.get("", Property::Rotation3D) else {
        panic!("a rotation");
    };
    let from_a = apart(q, a).to_degrees();
    assert!((from_a - 5.0).abs() < 1e-3, "a quarter of 20°: {from_a}°");
    assert!((apart(q, b).to_degrees() - 15.0).abs() < 1e-3);
    assert_eq!(
        mixed.get("", Property::Translation3D),
        Some(Value::Vec3(Vec3::new(0.5, 1.0, 1.5)))
    );
    // One input alone is its own value to the bit.
    let alone = blend(&[(&out, 0.3)]);
    assert_eq!(alone.get("", Property::Rotation3D), Some(Value::Quat(a)));

    // Three: equal thirds of 0°, 30° and 60° about X end at 30°.
    let x = |deg: f32| {
        pose(&[(
            "",
            Property::Rotation3D,
            Value::Quat(Quat::from_rotation_x(deg.to_radians())),
        )])
    };
    let (p0, p30, p60) = (x(0.0), x(30.0), x(60.0));
    let three = blend(&[(&p0, 1.0), (&p30, 1.0), (&p60, 1.0)]);
    let Some(Value::Quat(q)) = three.get("", Property::Rotation3D) else {
        panic!("a rotation");
    };
    let off = apart(q, Quat::from_rotation_x(30f32.to_radians()));
    assert!(off < 1e-4, "{off} rad from 30°");
    assert!((q.length() - 1.0).abs() < 1e-5);
}
