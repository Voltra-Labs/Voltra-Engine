//! Layering one pose over another, property by property.

use std::f32::consts::TAU;

use voltra_render::glam::Vec2;

use super::*;

fn pose(values: &[(&'static str, Property, Value)]) -> Pose<'static> {
    let mut pose = Pose::new();
    for &(target, property, value) in values {
        pose.set(target, property, value);
    }
    pose
}

fn float(pose: &Pose, target: &str, property: Property) -> f32 {
    match pose.get(target, property) {
        Some(Value::Float(v)) => v,
        other => panic!("{other:?}"),
    }
}

#[test]
fn an_override_is_the_lower_pose_at_zero_the_layer_at_one_and_continuous_between() {
    let below = pose(&[
        ("", Property::LightIntensity, Value::Float(1.0)),
        ("arm", Property::LightRadius, Value::Float(5.0)),
    ]);
    let layer = pose(&[("", Property::LightIntensity, Value::Float(3.0))]);
    let at = |w| {
        float(
            &composite(&below, &layer, w, LayerBlend::Override),
            "",
            Property::LightIntensity,
        )
    };
    assert_eq!(at(0.0), 1.0);
    assert_eq!(at(1.0), 3.0);
    assert!((at(0.25) - 1.5).abs() < 1e-6);
    assert!((at(1e-3) - 1.002).abs() < 1e-5, "no snap near zero");
    assert_eq!(at(f32::NAN), 1.0, "not a number weighs nothing");
    assert_eq!(at(7.0), 3.0, "clamped");
    let out = composite(&below, &layer, 0.5, LayerBlend::Override);
    assert_eq!(
        float(&out, "arm", Property::LightRadius),
        5.0,
        "the layer's mask kept it"
    );
}

#[test]
fn an_override_turns_the_short_way_and_switches_a_frame_half_way() {
    let below = pose(&[
        ("", Property::Rotation, Value::Float(350f32.to_radians())),
        ("", Property::SpriteFrame, Value::Int(1)),
    ]);
    let layer = pose(&[
        ("", Property::Rotation, Value::Float(10f32.to_radians())),
        ("", Property::SpriteFrame, Value::Int(4)),
    ]);
    let half = composite(&below, &layer, 0.5, LayerBlend::Override);
    let angle = float(&half, "", Property::Rotation).rem_euclid(TAU);
    assert!(angle < 1e-4 || (TAU - angle) < 1e-4, "through 0°: {angle}");
    assert_eq!(half.get("", Property::SpriteFrame), Some(Value::Int(4)));
    let less = composite(&below, &layer, 0.49, LayerBlend::Override);
    assert_eq!(less.get("", Property::SpriteFrame), Some(Value::Int(1)));
    let all = composite(&below, &layer, 1.0, LayerBlend::Override);
    assert!((float(&all, "", Property::Rotation) - 10f32.to_radians()).abs() < 1e-6);
}

#[test]
fn an_additive_layer_adds_its_change_by_weight_and_a_frame_overrides() {
    let below = pose(&[
        ("", Property::Scale, Value::Vec2(Vec2::new(1.0, 2.0))),
        ("", Property::SpriteFrame, Value::Int(1)),
    ]);
    let change = pose(&[
        ("", Property::Scale, Value::Vec2(Vec2::new(0.2, -0.4))),
        ("", Property::SpriteFrame, Value::Int(6)),
    ]);
    let out = composite(&below, &change, 0.5, LayerBlend::Additive);
    let Some(Value::Vec2(scale)) = out.get("", Property::Scale) else {
        panic!("a scale");
    };
    assert!((scale - Vec2::new(1.1, 1.8)).length() < 1e-6, "{scale}");
    assert_eq!(out.get("", Property::SpriteFrame), Some(Value::Int(6)));
    let none = composite(&below, &change, 0.0, LayerBlend::Additive);
    assert_eq!(none, below, "weight zero adds nothing");
}

#[test]
fn a_change_is_taken_from_the_reference_and_padded_with_none() {
    let mut pose_at = pose(&[
        ("", Property::Rotation, Value::Float(1.5)),
        ("", Property::SpriteFrame, Value::Int(3)),
        (
            "arm",
            Property::SpriteColor,
            Value::Color([1.0, 0.5, 0.5, 1.0]),
        ),
    ]);
    let reference = pose(&[
        ("", Property::Rotation, Value::Float(0.5)),
        ("", Property::SpriteFrame, Value::Int(1)),
        (
            "arm",
            Property::SpriteColor,
            Value::Color([1.0, 1.0, 1.0, 1.0]),
        ),
    ]);
    subtract(&mut pose_at, &reference);
    assert_eq!(pose_at.get("", Property::Rotation), Some(Value::Float(1.0)));
    assert_eq!(
        pose_at.get("", Property::SpriteFrame),
        Some(Value::Int(3)),
        "a frame is whole"
    );
    assert_eq!(
        pose_at.get("arm", Property::SpriteColor),
        Some(Value::Color([0.0, -0.5, -0.5, 0.0]))
    );

    let mut other = pose(&[("leg", Property::Rotation, Value::Float(2.0))]);
    pad(&mut other, &pose_at);
    assert_eq!(other.get("", Property::Rotation), Some(Value::Float(0.0)));
    assert_eq!(other.get("", Property::SpriteFrame), None, "not a frame");
    assert_eq!(
        other.get("leg", Property::Rotation),
        Some(Value::Float(2.0))
    );
}
