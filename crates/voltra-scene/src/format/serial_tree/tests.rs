use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use voltra_render::glam::Vec2;

use super::*;
use crate::{Collider, Sprite, Transform};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
enum Shape {
    Dot,
    Circle { radius: f32 },
    Capsule { radius: f32, height: f32 },
    Pair(f32, f32),
    Wrapped(Vec2),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Label(String);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Thing {
    at: Vec2,
    shape: Shape,
    tag: Option<Label>,
    points: Vec<Vec2>,
    weights: BTreeMap<String, f32>,
    seen: (bool, char),
}

fn thing() -> Thing {
    Thing {
        at: Vec2::new(1.0, 2.0),
        shape: Shape::Circle { radius: 0.5 },
        tag: Some(Label("coin".into())),
        points: vec![Vec2::ZERO, Vec2::ONE],
        weights: BTreeMap::from([("a".into(), 1.0), ("b".into(), 2.0)]),
        seen: (true, 'x'),
    }
}

fn tree<T: Serialize>(value: &T) -> SerialTree {
    SerialTree::record(value).expect("records")
}

#[test]
fn a_replayed_tree_writes_exactly_what_the_type_writes() {
    for value in [
        thing(),
        Thing {
            shape: Shape::Dot,
            tag: None,
            ..thing()
        },
        Thing {
            shape: Shape::Pair(1.0, f32::NAN),
            ..thing()
        },
        Thing {
            shape: Shape::Wrapped(Vec2::X),
            ..thing()
        },
    ] {
        let raw = tree(&value).to_raw().expect("writes");
        let direct = ron::value::RawValue::from_rust(&value).expect("writes");
        assert_eq!(raw.get_ron(), direct.get_ron());
    }
    // And the engine's own, enums included, load back as themselves.
    let collider = Collider::Capsule {
        radius: 0.25,
        half_height: 1.0,
    };
    let back: Collider = tree(&collider)
        .to_raw()
        .expect("writes")
        .into_rust()
        .expect("reads as a collider");
    assert_eq!(back, collider);
}

#[test]
fn equal_values_have_no_differences() {
    assert!(differences(&tree(&thing()), &tree(&thing())).is_empty());
}

#[test]
fn a_nested_field_that_differs_is_named_alone() {
    let mut other = thing();
    other.at.y = 3.0;
    other.points[1].x = 9.0;
    assert_eq!(
        differences(&tree(&thing()), &tree(&other)),
        vec![
            vec![Step::Field("at"), Step::Index(1)],
            vec![Step::Field("points"), Step::Index(1), Step::Index(0)],
        ]
    );
    assert_eq!(
        describe(&[Step::Field("points"), Step::Index(1)]),
        "points[1]"
    );
}

#[test]
fn a_changed_variant_is_the_place_and_the_same_variant_is_walked_into() {
    let same = Thing {
        shape: Shape::Circle { radius: 2.0 },
        ..thing()
    };
    assert_eq!(
        differences(&tree(&thing()), &tree(&same)),
        vec![vec![
            Step::Field("shape"),
            Step::Variant("Circle"),
            Step::Field("radius")
        ]]
    );
    let other = Thing {
        shape: Shape::Capsule {
            radius: 0.5,
            height: 1.0,
        },
        ..thing()
    };
    assert_eq!(
        differences(&tree(&thing()), &tree(&other)),
        vec![vec![Step::Field("shape")]]
    );
}

#[test]
fn options_maps_and_lists_are_walked_while_their_shape_agrees() {
    let mut other = thing();
    other.tag = Some(Label("gem".into()));
    other.weights.insert("b".into(), 5.0);
    assert_eq!(
        differences(&tree(&thing()), &tree(&other)),
        vec![
            vec![Step::Field("tag"), Step::Inner, Step::Inner],
            vec![
                Step::Field("weights"),
                Step::Key(SerialTree::Str("b".into()))
            ],
        ]
    );
    let mut grown = thing();
    grown.points.push(Vec2::X);
    grown.tag = None;
    assert_eq!(
        differences(&tree(&thing()), &tree(&grown)),
        vec![vec![Step::Field("tag")], vec![Step::Field("points")]]
    );
}

#[test]
fn floats_compare_exactly_and_every_nan_is_the_same() {
    assert_eq!(tree(&0.1_f32), tree(&0.1_f32));
    assert_ne!(tree(&0.1_f32), tree(&0.100_000_01_f32));
    assert_ne!(tree(&0.0_f32), tree(&-0.0_f32), "a file writes them apart");
    assert_eq!(tree(&f32::NAN), tree(&-f32::NAN));
    assert_eq!(tree(&f64::NAN), tree(&f64::NAN));
    assert_ne!(tree(&f32::NAN), tree(&1.0_f32));
}

#[test]
fn a_value_is_written_at_a_place_only_where_the_tree_has_it() {
    let mut circle = tree(&Shape::Circle { radius: 1.0 });
    let path = [Step::Variant("Circle"), Step::Field("radius")];
    assert!(circle.set(&path, tree(&4.0_f32)));
    let back: Shape = circle.to_raw().expect("w").into_rust().expect("r");
    assert_eq!(back, Shape::Circle { radius: 4.0 });

    // A capsule has a radius too, but not the circle's.
    let mut capsule = tree(&Shape::Capsule {
        radius: 1.0,
        height: 2.0,
    });
    let before = capsule.clone();
    assert!(!capsule.set(&path, tree(&4.0_f32)));
    assert_eq!(capsule, before);

    let sprite = tree(&Sprite::new([1.0, 0.0, 0.0, 1.0]));
    assert_eq!(
        sprite.at(&[Step::Field("color"), Step::Index(0)]),
        Some(&tree(&1.0_f32))
    );
    assert_eq!(sprite.at(&[Step::Field("no_such_field")]), None);
    let transform = tree(&Transform::default());
    assert_eq!(transform.at(&[]), Some(&transform));
}
