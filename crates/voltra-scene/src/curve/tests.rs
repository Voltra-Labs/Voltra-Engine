use super::*;

#[test]
fn an_empty_curve_changes_nothing() {
    assert_eq!(Curve::default().sample(0.3), 1.0);
    assert_eq!(Curve::default().range(), (1.0, 1.0));
}

#[test]
fn a_curve_is_linear_between_keys_and_flat_past_them() {
    let curve = Curve::new(vec![CurveKey::new(0.25, 2.0), CurveKey::new(0.75, 4.0)]);
    assert_eq!(curve.sample(0.0), 2.0);
    assert_eq!(curve.sample(0.5), 3.0);
    assert_eq!(curve.sample(1.0), 4.0);
    assert_eq!(curve.sample(f32::NAN), 2.0);
}

#[test]
fn keys_are_read_in_order_and_broken_ones_skipped() {
    let curve = Curve::new(vec![
        CurveKey::new(1.0, 0.0),
        CurveKey::new(f32::NAN, 99.0),
        CurveKey::new(0.0, 1.0),
    ]);
    assert_eq!(curve.sample(0.5), 0.5);
    assert_eq!(curve.sorted().len(), 2);
}

#[test]
fn a_key_s_ease_shapes_the_segment_leaving_it() {
    let mut curve = Curve::linear(0.0, 1.0);
    curve.keys[0].ease = Ease::Step;
    assert_eq!(curve.sample(0.9), 0.0);
    assert_eq!(curve.sample(1.0), 1.0);
}

#[test]
fn a_table_spans_both_ends() {
    let table = Curve::linear(0.0, 1.0).table(5);
    assert_eq!(table, vec![0.0, 0.25, 0.5, 0.75, 1.0]);
}

#[test]
fn a_curve_round_trips_as_a_list_of_keys() {
    let curve = Curve::new(vec![CurveKey::new(0.0, 1.0), CurveKey::new(1.0, 0.0)]);
    let text = ron::to_string(&curve).expect("written");
    assert_eq!(text, "[(t:0.0,value:1.0),(t:1.0,value:0.0)]");
    assert_eq!(ron::from_str::<Curve>(&text).expect("read"), curve);
}
