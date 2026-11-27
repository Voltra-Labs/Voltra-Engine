//! The camera's maths: projection, conversions, zoom.

use super::*;

/// Projecting a world point and dividing by w, as the GPU does.
fn project(camera: &Camera2D, point: Vec2) -> Vec2 {
    let clip = camera.view_projection() * glam::Vec4::new(point.x, point.y, 0.0, 1.0);
    Vec2::new(clip.x / clip.w, clip.y / clip.w)
}

#[test]
fn default_camera_is_identity_over_clip_space() {
    let camera = Camera2D::default();
    // At zoom 1 and aspect 1 the visible region is exactly clip space, so
    // geometry authored before the camera existed must not move.
    assert!((project(&camera, Vec2::new(0.0, 0.5)) - Vec2::new(0.0, 0.5)).length() < 1e-6);
    assert!((project(&camera, Vec2::new(-0.5, -0.5)) - Vec2::new(-0.5, -0.5)).length() < 1e-6);
}

#[test]
fn moving_the_camera_shifts_geometry_the_other_way() {
    let camera = Camera2D::new(Vec2::new(1.0, 0.0), 1.0, 1.0);
    // The camera moved right, so the origin must appear to the left.
    let origin = project(&camera, Vec2::ZERO);
    assert!(
        origin.x < -0.9,
        "expected origin off to the left, got {origin}"
    );
}

#[test]
fn zoom_shrinks_the_visible_region() {
    let camera = Camera2D::new(Vec2::ZERO, 2.0, 1.0);
    assert_eq!(camera.half_extents(), Vec2::new(0.5, 0.5));
    // A point at the old edge is now past the edge of the screen.
    assert!(project(&camera, Vec2::new(0.0, 1.0)).y > 1.0);
}

#[test]
fn aspect_widens_the_visible_region_horizontally() {
    let camera = Camera2D::new(Vec2::ZERO, 1.0, 2.0);
    assert_eq!(camera.half_extents(), Vec2::new(2.0, 1.0));
    // Twice as wide means x is compressed by half in clip space.
    let p = project(&camera, Vec2::new(1.0, 0.0));
    assert!((p.x - 0.5).abs() < 1e-6, "got {p}");
}

#[test]
fn uniform_is_the_matrix_in_column_major_order() {
    let camera = Camera2D::new(Vec2::new(3.0, 4.0), 1.5, 1.25);
    let uniform = CameraUniform::from(&camera);
    assert_eq!(
        uniform.view_proj,
        camera.view_projection().to_cols_array_2d()
    );
    assert_eq!(size_of::<CameraUniform>(), 16 * size_of::<f32>());
}

/// A viewport that is not square and not a round number, so an axis mix-up
/// cannot pass by coincidence.
fn viewport() -> Vec2 {
    Vec2::new(800.0, 600.0)
}

fn camera_for(viewport: Vec2) -> Camera2D {
    Camera2D::new(Vec2::new(3.0, -2.0), 1.5, viewport.x / viewport.y)
}

#[test]
fn viewport_centre_is_the_camera_position() {
    let viewport = viewport();
    let camera = camera_for(viewport);
    let world = camera.viewport_to_world(viewport * 0.5, viewport);
    assert!(
        (world - camera.position).length() < 1e-5,
        "centre mapped to {world}, expected {}",
        camera.position
    );
}

#[test]
fn viewport_top_left_is_up_and_to_the_left() {
    let viewport = viewport();
    let camera = camera_for(viewport);
    let half = camera.half_extents();
    // Top-left in a Y-down viewport is minimum x and *maximum* y in world
    // space. Getting this flip wrong is the classic silent bug.
    let expected = camera.position + Vec2::new(-half.x, half.y);
    let world = camera.viewport_to_world(Vec2::ZERO, viewport);
    assert!(
        (world - expected).length() < 1e-5,
        "got {world}, want {expected}"
    );
}

#[test]
fn viewport_and_world_round_trip() {
    let viewport = viewport();
    for zoom in [Camera2D::MIN_ZOOM, 0.25, 1.0, 7.5, Camera2D::MAX_ZOOM] {
        let camera = Camera2D::new(Vec2::new(-4.0, 9.0), zoom, viewport.x / viewport.y);
        for point in [
            Vec2::ZERO,
            viewport,
            viewport * 0.5,
            Vec2::new(123.0, 456.0),
        ] {
            let back =
                camera.world_to_viewport(camera.viewport_to_world(point, viewport), viewport);
            // A twentieth of a point, not a thousandth. `world_to_viewport`
            // subtracts two world coordinates of magnitude ~9 to get a
            // difference of ~0.008 at MAX_ZOOM, then divides by an equally
            // small half-extent. f32 carries about seven digits, so that
            // cancellation costs real precision — the observed error at the
            // ceiling is ~0.006 points. This is inherent to f32 world
            // coordinates, not to the algorithm, and a twentieth of a pixel
            // is far below anything visible.
            assert!(
                (back - point).length() < 0.05,
                "zoom {zoom}: {point} round-tripped to {back}"
            );
        }
    }
}

#[test]
fn zoom_around_keeps_the_anchor_world_point_still() {
    let viewport = viewport();
    let mut camera = camera_for(viewport);
    // Off-centre on both axes: a centred zoom would pass a centred anchor.
    let anchor = Vec2::new(150.0, 500.0);
    let before = camera.viewport_to_world(anchor, viewport);

    camera.zoom_around(anchor, viewport, 1.7);

    let after = camera.viewport_to_world(anchor, viewport);
    assert!(
        (after - before).length() < 1e-4,
        "anchor drifted from {before} to {after}"
    );
    assert!(
        camera.zoom() > 1.5,
        "zoom did not increase: {}",
        camera.zoom()
    );
}

#[test]
fn set_zoom_clamps_both_ends() {
    let mut camera = Camera2D::default();
    camera.set_zoom(f32::INFINITY);
    assert_eq!(camera.zoom(), Camera2D::MAX_ZOOM);
    camera.set_zoom(0.0);
    assert_eq!(camera.zoom(), Camera2D::MIN_ZOOM);
    camera.set_zoom(-5.0);
    assert_eq!(camera.zoom(), Camera2D::MIN_ZOOM);
}

#[test]
fn set_zoom_ignores_nan() {
    let mut camera = Camera2D::default();
    camera.set_zoom(2.0);
    camera.set_zoom(f32::NAN);
    assert_eq!(camera.zoom(), 2.0, "NaN must leave the previous zoom alone");
}

#[test]
fn zoom_around_at_the_clamp_does_not_drift() {
    let viewport = viewport();
    let mut camera = Camera2D::new(Vec2::ZERO, Camera2D::MAX_ZOOM, viewport.x / viewport.y);
    let anchor = Vec2::new(10.0, 20.0);

    camera.zoom_around(anchor, viewport, 2.0);

    // Zoom was already at the ceiling, so nothing may move. Without this
    // the camera slides sideways every notch once the user hits the limit.
    assert_eq!(camera.zoom(), Camera2D::MAX_ZOOM);
    assert!(
        camera.position.length() < 1e-6,
        "drifted to {}",
        camera.position
    );
}

#[test]
fn new_clamps_its_zoom() {
    // The constructor is a second door onto the same invariant. Leaving it
    // unclamped means `Camera2D::new(pos, 0.0, 1.0)` produces infinities in
    // half_extents with no warning anywhere.
    assert_eq!(
        Camera2D::new(Vec2::ZERO, 0.0, 1.0).zoom(),
        Camera2D::MIN_ZOOM
    );
    assert_eq!(
        Camera2D::new(Vec2::ZERO, 1e9, 1.0).zoom(),
        Camera2D::MAX_ZOOM
    );
}

#[test]
fn a_degenerate_viewport_stays_finite() {
    let camera = Camera2D::default();
    let world = camera.viewport_to_world(Vec2::ZERO, Vec2::ZERO);
    assert!(world.is_finite(), "got {world}");
}

/// A view turned a quarter turn and a bit, off the origin, on a viewport
/// that is not square: nothing lines up by coincidence.
fn turned() -> Camera2D {
    Camera2D::new(Vec2::new(3.0, -2.0), 1.5, 800.0 / 600.0).with_rotation(1.9)
}

#[test]
fn a_turned_view_picks_what_its_matrix_draws_there() {
    // A point projected by the shader's matrix lands at the viewport pixel
    // `world_to_viewport` names, and `viewport_to_world` takes that pixel
    // back to the point: picking through a turned camera is the inverse of
    // drawing through it.
    let camera = turned();
    let viewport = viewport();
    for point in [
        Vec2::new(3.0, -2.0),
        Vec2::new(4.0, -1.5),
        Vec2::new(2.2, -2.9),
    ] {
        let clip = project(&camera, point);
        let pixel = Vec2::new((clip.x + 1.0) * 0.5, (1.0 - clip.y) * 0.5) * viewport;
        assert!(
            (camera.world_to_viewport(point, viewport) - pixel).length() < 1e-2,
            "{point}"
        );
        let back = camera.viewport_to_world(pixel, viewport);
        assert!(
            (back - point).length() < 1e-4,
            "{point} came back as {back}"
        );
    }
}

#[test]
fn turning_the_view_turns_the_world_the_other_way() {
    // The view turned a quarter turn left, the world turns right on
    // screen: a point to the world's right of the camera shows below the
    // centre.
    let camera = Camera2D::default().with_rotation(std::f32::consts::FRAC_PI_2);
    let on_screen = project(&camera, Vec2::new(0.5, 0.0));
    assert!(
        on_screen.abs_diff_eq(Vec2::new(0.0, -0.5), 1e-6),
        "{on_screen}"
    );
    // And the top-left pixel is the world point up-left of the turned view.
    let viewport = Vec2::splat(100.0);
    let top_left = camera.viewport_to_world(Vec2::ZERO, viewport);
    assert!(
        top_left.abs_diff_eq(Vec2::new(-1.0, -1.0), 1e-5),
        "{top_left}"
    );
}

#[test]
fn a_view_at_45_degrees_sees_the_box_around_its_turned_rect() {
    let camera =
        Camera2D::new(Vec2::new(1.0, 1.0), 1.0, 5.0).with_rotation(std::f32::consts::FRAC_PI_4);
    // Half extents 5 × 1, turned 45°: the box reaches (5 + 1) / √2 each way.
    let (min, max) = camera.visible_bounds();
    let reach = 6.0 / 2f32.sqrt();
    assert!(min.abs_diff_eq(Vec2::splat(1.0 - reach), 1e-5), "{min}");
    assert!(max.abs_diff_eq(Vec2::splat(1.0 + reach), 1e-5), "{max}");
    // Every corner is inside the box, and touches it.
    for corner in camera.corners() {
        assert!(corner.cmpge(min - 1e-5).all() && corner.cmple(max + 1e-5).all());
    }
    let upright = Camera2D::new(Vec2::new(1.0, 1.0), 1.0, 5.0);
    assert_eq!(
        upright.visible_bounds(),
        (Vec2::new(-4.0, 0.0), Vec2::new(6.0, 2.0))
    );
}

#[test]
fn an_upright_view_draws_and_picks_exactly_as_before() {
    let camera = camera_for(viewport());
    let turned_zero = camera.with_rotation(0.0);
    assert_eq!(camera.view_projection(), turned_zero.view_projection());
    let corners = camera.corners();
    let half = camera.half_extents();
    assert_eq!(corners[0], camera.position - half);
    assert_eq!(corners[2], camera.position + half);
}

#[test]
fn a_non_finite_turn_is_read_as_upright() {
    let camera = camera_for(viewport()).with_rotation(f32::NAN);
    assert_eq!(camera.turn(), 0.0);
    assert!(camera.view_projection().is_finite());
    assert!(camera.visible_bounds().0.is_finite());
}

mod screen {
    use super::*;

    fn project(matrix: Mat4, point: Vec2) -> Vec2 {
        let clip = matrix * point.extend(0.0).extend(1.0);
        Vec2::new(clip.x, clip.y) / clip.w
    }

    #[test]
    fn the_top_left_pixel_corner_is_the_top_left_of_clip_space() {
        let m = screen_projection(800.0, 600.0);
        assert!(project(m, Vec2::ZERO).abs_diff_eq(Vec2::new(-1.0, 1.0), 1e-6));
    }

    #[test]
    fn y_runs_down_to_the_bottom_right() {
        let m = screen_projection(800.0, 600.0);
        assert!(project(m, Vec2::new(800.0, 600.0)).abs_diff_eq(Vec2::new(1.0, -1.0), 1e-6));
        assert!(project(m, Vec2::new(400.0, 300.0)).abs_diff_eq(Vec2::ZERO, 1e-6));
    }

    #[test]
    fn a_zero_sized_target_does_not_divide_by_zero() {
        let m = screen_projection(0.0, 0.0);
        assert!(project(m, Vec2::ONE).is_finite());
    }
}
