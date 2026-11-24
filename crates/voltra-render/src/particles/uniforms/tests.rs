use super::*;

#[test]
fn nonsense_becomes_nothing() {
    let step = SimStep {
        dt: -1.0,
        spawn_start: 10,
        spawn_count: 99,
        speed: (f32::NAN, 1.0),
        origin: Vec2::new(f32::INFINITY, 0.0),
        rotation: (f32::NAN, 1.0),
        ..SimStep::default()
    };
    let u = SimUniform::new(&step, 8, 0, 0);
    assert_eq!(u.origin_dt[..3], [0.0, 0.0, 0.0]);
    assert_eq!(u.ranges[0], 0.0);
    assert_eq!(u.spin[..2], [0.0, 1.0]);
    assert_eq!(u.spawn[..3], [2, 8, 8], "wrapped and capped to the ring");
}

#[test]
fn an_empty_ring_does_not_divide_by_zero() {
    let u = SimUniform::new(&SimStep::default(), 0, 0, 0);
    assert_eq!(u.spawn[..3], [0, 0, 0]);
}

#[test]
fn shapes_are_tidied_and_an_outline_needs_two_corners() {
    let ring = SimStep {
        shape: SpawnShape::Ring {
            inner: 2.0,
            outer: -1.0,
        },
        ..SimStep::default()
    };
    let u = SimUniform::new(&ring, 8, 0, 0);
    assert_eq!((u.counts[1], u.shape[..2].to_vec()), (1, vec![1.0, 2.0]));
    let outline = SimStep {
        shape: SpawnShape::Outline,
        ..SimStep::default()
    };
    assert_eq!(SimUniform::new(&outline, 8, 0, 1).counts[1], 0);
    assert_eq!(SimUniform::new(&outline, 8, 0, 4).counts[1..3], [4, 4]);
}

#[test]
fn a_space_scaled_to_nothing_tests_no_collider() {
    let flat = SimStep {
        to_world: Mat3::from_scale(Vec2::new(1.0, 0.0)),
        ..SimStep::default()
    };
    assert_eq!(SimUniform::new(&flat, 8, 3, 0).counts[0], 0);
    let moved = SimStep {
        to_world: Mat3::from_translation(Vec2::new(2.0, 0.0)),
        ..SimStep::default()
    };
    let u = SimUniform::new(&moved, 8, 3, 0);
    assert_eq!(u.counts[0], 3);
    assert_eq!(u.from_world_t[0], -2.0, "the inverse goes back");
}

#[test]
fn a_broken_matrix_places_at_the_origin() {
    let look = ParticleLook {
        place: Mat3::from_cols_array(&[f32::NAN; 9]),
        ..ParticleLook::default()
    };
    assert_eq!(LookUniform::new(&look, 0).place_x, [1.0, 0.0, 0.0, 0.0]);
}

#[test]
fn a_flipbook_without_frames_draws_the_whole_texture() {
    let look = ParticleLook {
        frames: FrameTiming::PerSecond { fps: 12.0 },
        ..ParticleLook::default()
    };
    assert_eq!(LookUniform::new(&look, 0).frames, [0.0; 4]);
    assert_eq!(LookUniform::new(&look, 4).frames, [4.0, 2.0, 12.0, 0.0]);
}
