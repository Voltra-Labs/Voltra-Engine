//! A turned view: drawn, picked and culled the same way round; upright as
//! it always was.

use super::tests::{camera_for, project, viewport};
use super::*;

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
    // Literal values, worked out by hand for the upright maths this stage
    // started from: at (3, -2), zoom 1.5 on 800 × 600 the view reaches 2/3
    // up and down and 4/3 of that either side.
    let camera = camera_for(viewport()).with_rotation(0.0);
    let half = Vec2::new(8.0 / 9.0, 2.0 / 3.0);
    assert!(camera.half_extents().abs_diff_eq(half, 1e-6));
    let expected = Mat4::from_cols_array(&[
        1.0 / half.x,
        0.0,
        0.0,
        0.0,
        0.0,
        1.0 / half.y,
        0.0,
        0.0,
        0.0,
        0.0,
        -0.5,
        0.0,
        -3.0 / half.x,
        2.0 / half.y,
        0.5,
        1.0,
    ]);
    assert!(
        camera.view_projection().abs_diff_eq(expected, 1e-5),
        "{}",
        camera.view_projection()
    );
    let corners = camera.corners();
    assert!(corners[0].abs_diff_eq(Vec2::new(3.0 - half.x, -2.0 - half.y), 1e-6));
    assert!(corners[2].abs_diff_eq(Vec2::new(3.0 + half.x, -2.0 + half.y), 1e-6));
    // The top-left pixel is up and left of the centre; the centre is (3, -2).
    let top_left = camera.viewport_to_world(Vec2::ZERO, viewport());
    assert!(top_left.abs_diff_eq(Vec2::new(3.0 - half.x, -2.0 + half.y), 1e-5));
    let centre = camera.world_to_viewport(Vec2::new(3.0, -2.0), viewport());
    assert!(centre.abs_diff_eq(Vec2::new(400.0, 300.0), 1e-3));
}

#[test]
fn a_non_finite_turn_is_read_as_upright() {
    let camera = camera_for(viewport()).with_rotation(f32::NAN);
    assert_eq!(camera.turn(), 0.0);
    assert!(camera.view_projection().is_finite());
    assert!(camera.visible_bounds().0.is_finite());
}
