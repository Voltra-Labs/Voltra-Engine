use super::*;

#[test]
fn a_rotated_box_is_bounded_by_a_larger_aabb() {
    use std::f32::consts::{FRAC_PI_4, SQRT_2};

    let collider = Collider2D::Box {
        half_extents: Vec2::splat(1.0),
    };
    let transform = Transform2D::default().with_rotation(FRAC_PI_4);

    let (min, max) = collider.world_aabb(&transform);

    // A unit square turned 45° spans its diagonal, √2 either side.
    assert!((max.x - SQRT_2).abs() < 1e-5, "{max:?}");
    assert!((min.y + SQRT_2).abs() < 1e-5, "{min:?}");
}

#[test]
fn the_corners_follow_the_rotation_counter_clockwise() {
    use std::f32::consts::FRAC_PI_2;

    let collider = Collider2D::Box {
        half_extents: Vec2::splat(1.0),
    };
    let corners = collider.corners(&Transform2D::default().with_rotation(FRAC_PI_2));

    // (+1,+1) turned a quarter turn is (-1,+1).
    assert!(
        (corners[0] - Vec2::new(-1.0, 1.0)).length() < 1e-5,
        "{corners:?}"
    );
    // Counter-clockwise, so the shoelace sum is positive. The clipping in
    // the narrow phase depends on this winding.
    let area: f32 = (0..4)
        .map(|i| {
            let (p, q) = (corners[i], corners[(i + 1) % 4]);
            p.x * q.y - q.x * p.y
        })
        .sum();
    assert!(area > 0.0, "{corners:?}");
}

#[test]
fn the_corners_follow_the_translation_and_scale() {
    let collider = Collider2D::Box {
        half_extents: Vec2::splat(1.0),
    };
    let transform = Transform2D::from_translation(Vec2::new(5.0, 0.0)).with_scale(Vec2::splat(2.0));

    let corners = collider.corners(&transform);

    assert!(
        (corners[0] - Vec2::new(7.0, 2.0)).length() < 1e-5,
        "{corners:?}"
    );
    assert!(
        (corners[2] - Vec2::new(3.0, -2.0)).length() < 1e-5,
        "{corners:?}"
    );
}

#[test]
fn a_circle_has_no_corners_but_still_answers() {
    let collider = Collider2D::Circle { radius: 2.0 };
    let transform = Transform2D::from_translation(Vec2::new(1.0, 1.0));

    assert_eq!(collider.corners(&transform), [transform.translation; 4]);
}

#[test]
fn a_box_spans_twice_its_half_extents() {
    let collider = Collider2D::Box {
        half_extents: Vec2::new(2.0, 3.0),
    };
    let (min, max) = collider.world_aabb(&Transform2D::default());

    assert_eq!(min, Vec2::new(-2.0, -3.0));
    assert_eq!(max, Vec2::new(2.0, 3.0));
}

#[test]
fn a_box_follows_the_transform_translation() {
    let collider = Collider2D::Box {
        half_extents: Vec2::splat(1.0),
    };
    let transform = Transform2D::from_translation(Vec2::new(10.0, -4.0));
    let (min, max) = collider.world_aabb(&transform);

    assert_eq!(min, Vec2::new(9.0, -5.0));
    assert_eq!(max, Vec2::new(11.0, -3.0));
}

#[test]
fn a_collider_is_scaled_by_its_transform() {
    // A scaled sprite with an unscaled collider is a bug that looks like a
    // physics bug: the outline and the picture disagree and neither is
    // obviously the wrong one.
    let collider = Collider2D::Box {
        half_extents: Vec2::splat(1.0),
    };
    let transform = Transform2D::default().with_scale(Vec2::new(3.0, 5.0));
    let (min, max) = collider.world_aabb(&transform);

    assert_eq!(min, Vec2::new(-3.0, -5.0));
    assert_eq!(max, Vec2::new(3.0, 5.0));
}

#[test]
fn a_negative_scale_still_gives_a_min_below_its_max() {
    // A mirrored sprite is a negative scale, and an AABB whose min exceeds
    // its max overlaps nothing — the collider would silently stop working.
    let collider = Collider2D::Box {
        half_extents: Vec2::splat(2.0),
    };
    let transform = Transform2D::default().with_scale(Vec2::new(-1.0, 1.0));
    let (min, max) = collider.world_aabb(&transform);

    assert!(min.x <= max.x && min.y <= max.y, "got {min:?}..{max:?}");
}

#[test]
fn a_circle_takes_the_larger_axis_of_a_non_uniform_scale() {
    // A true ellipse is a different shape, not a parameter. Taking the
    // larger axis keeps the collider covering the sprite rather than
    // cutting into it.
    let collider = Collider2D::Circle { radius: 1.0 };
    let transform = Transform2D::default().with_scale(Vec2::new(2.0, 5.0));

    assert_eq!(collider.world_radius(&transform), 5.0);
}

#[test]
fn a_circles_aabb_is_its_bounding_square() {
    let collider = Collider2D::Circle { radius: 2.0 };
    let (min, max) = collider.world_aabb(&Transform2D::default());

    assert_eq!(min, Vec2::splat(-2.0));
    assert_eq!(max, Vec2::splat(2.0));
}

#[test]
fn a_zero_or_negative_shape_is_degenerate() {
    let t = Transform2D::default();

    assert!(Collider2D::Circle { radius: 0.0 }.is_degenerate(&t));
    assert!(Collider2D::Circle { radius: -1.0 }.is_degenerate(&t));
    assert!(Collider2D::Box {
        half_extents: Vec2::ZERO
    }
    .is_degenerate(&t));
    assert!(!Collider2D::Circle { radius: 1.0 }.is_degenerate(&t));
}

#[test]
fn a_shape_scaled_to_nothing_is_degenerate() {
    let flat = Transform2D::default().with_scale(Vec2::new(1.0, 0.0));

    assert!(Collider2D::Box {
        half_extents: Vec2::splat(1.0)
    }
    .is_degenerate(&flat));
}

#[test]
fn a_collider_round_trips_through_ron() {
    for collider in [
        Collider2D::Box {
            half_extents: Vec2::new(1.0, 2.0),
        },
        Collider2D::Circle { radius: 3.0 },
    ] {
        let text = ron::to_string(&collider).expect("serialise");
        let back: Collider2D = ron::from_str(&text).expect("deserialise");
        assert_eq!(back, collider);
    }
}

fn triangle() -> Collider2D {
    Collider2D::Polygon {
        points: ConvexPolygon::new(&[
            Vec2::new(-1.0, -1.0),
            Vec2::new(1.0, -1.0),
            Vec2::new(0.0, 1.0),
        ])
        .expect("a triangle"),
        radius: 0.0,
    }
}

#[test]
fn a_capsule_is_two_cap_centres_and_a_radius() {
    let capsule = Collider2D::Capsule {
        half_height: 1.0,
        radius: 0.25,
    };
    let shape = capsule.world_shape(&Transform2D::from_translation(Vec2::new(2.0, 0.0)));

    assert_eq!(
        shape.vertices(),
        &[Vec2::new(2.0, -0.75), Vec2::new(2.0, 0.75)]
    );
    assert_eq!(shape.radius(), 0.25);
    let (min, max) = capsule.world_aabb(&Transform2D::from_translation(Vec2::new(2.0, 0.0)));
    assert!((min - Vec2::new(1.75, -1.0)).length() < 1e-6, "{min:?}");
    assert!((max - Vec2::new(2.25, 1.0)).length() < 1e-6, "{max:?}");
}

#[test]
fn a_capsule_turns_and_scales_across_and_along() {
    use std::f32::consts::FRAC_PI_2;

    let capsule = Collider2D::Capsule {
        half_height: 1.0,
        radius: 0.5,
    };
    let turned = Transform2D::default()
        .with_rotation(FRAC_PI_2)
        .with_scale(Vec2::new(2.0, 3.0));
    let shape = capsule.world_shape(&turned);

    // Radius 0.5·2, height 1·3: the caps' centres 2 from the middle, along
    // the turned y, which is world -x.
    assert!((shape.radius() - 1.0).abs() < 1e-6);
    assert!(
        (shape.vertices()[0] - Vec2::new(2.0, 0.0)).length() < 1e-5,
        "{shape:?}"
    );
}

#[test]
fn a_capsule_no_taller_than_wide_is_a_circle() {
    let capsule = Collider2D::Capsule {
        half_height: 0.2,
        radius: 0.5,
    };
    let shape = capsule.world_shape(&Transform2D::default());

    assert_eq!(shape.vertices().len(), 1);
    assert!(!capsule.is_degenerate(&Transform2D::default()));
}

#[test]
fn a_polygon_is_placed_by_its_transform() {
    let at = Transform2D::from_translation(Vec2::new(0.0, 5.0)).with_scale(Vec2::splat(2.0));
    let shape = triangle().world_shape(&at);

    assert!(shape.vertices().contains(&Vec2::new(0.0, 7.0)), "{shape:?}");
    let (min, max) = triangle().world_aabb(&at);
    assert_eq!((min, max), (Vec2::new(-2.0, 3.0), Vec2::new(2.0, 7.0)));
}

#[test]
fn a_mirrored_polygon_still_winds_counter_clockwise() {
    let mirrored = Transform2D::default().with_scale(Vec2::new(-1.0, 1.0));
    let shape = triangle().world_shape(&mirrored);
    let v = shape.vertices();
    let area: f32 = (0..v.len())
        .map(|i| v[i].perp_dot(v[(i + 1) % v.len()]))
        .sum();

    assert!(area > 0.0, "{v:?}");
}

#[test]
fn new_shapes_are_degenerate_when_they_have_no_area() {
    let t = Transform2D::default();
    assert!(Collider2D::Capsule {
        half_height: 1.0,
        radius: 0.0
    }
    .is_degenerate(&t));
    assert!(triangle().is_degenerate(&t.with_scale(Vec2::new(1.0, 0.0))));
    assert!(Collider2D::Polygon {
        points: ConvexPolygon::rectangle(Vec2::ONE).expect("a square"),
        radius: -1.0
    }
    .is_degenerate(&t));
    assert!(!triangle().is_degenerate(&t));
}

#[test]
fn new_shapes_round_trip_through_ron_and_old_text_still_loads() {
    for collider in [
        Collider2D::Capsule {
            half_height: 1.0,
            radius: 0.5,
        },
        triangle(),
    ] {
        let text = ron::to_string(&collider).expect("serialise");
        let back: Collider2D = ron::from_str(&text).expect("deserialise");
        assert_eq!(back, collider);
    }

    // Written before polygons had a rounding, and before capsules existed.
    let sharp: Collider2D = ron::from_str("Polygon(points:[(-1,-1),(1,-1),(0,1)])").expect("loads");
    assert_eq!(sharp, triangle());
    let old: Collider2D = ron::from_str("Box(half_extents:(0.5,0.5))").expect("loads");
    assert_eq!(old, Collider2D::default());
}
