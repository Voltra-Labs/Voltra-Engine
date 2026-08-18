//! What each tool's gizmo looks like, as world-space line segments.
//!
//! Pure, and separated from the drawing for exactly that: the property that
//! matters — a gizmo the same size on screen at every zoom — is a statement
//! about these numbers, and testing it through egui and a GPU would be testing
//! it nowhere.
//!
//! Every shape is laid out in **pixels** and converted back to world units at
//! the end. Any other order gives a gizmo that grows and shrinks with the
//! camera, which is the one thing a manipulator must never do: its handles are
//! targets for a cursor that does not change size.

use voltra_render::glam::Vec2;
use voltra_render::Camera2D;
use voltra_scene::Transform;

use super::handle::{Axes, AXIS_LENGTH, CENTRE_HALF, RING_RADIUS, TIP_HALF};
use crate::tool::Tool;

/// x red, y green — the axis convention every editor shares.
const X_COLOR: [f32; 4] = [0.90, 0.25, 0.25, 1.0];
const Y_COLOR: [f32; 4] = [0.35, 0.85, 0.35, 1.0];
const CENTRE_COLOR: [f32; 4] = [0.95, 0.95, 0.95, 1.0];

/// The rotation ring. Blue, because a 2D rotation is the one transform whose
/// axis is not on screen, and blue is where every editor puts that axis.
const RING_COLOR: [f32; 4] = [0.35, 0.55, 0.95, 1.0];

/// How many straight segments stand in for the ring.
///
/// 48 puts a vertex every 7.5°, which at a 60-pixel radius leaves the flat
/// sides under a pixel of chord error — smaller than the line is wide.
const RING_SEGMENTS: usize = 48;

/// One drawn segment: two world-space endpoints and a colour.
pub type Segment = (Vec2, Vec2, [f32; 4]);

/// The gizmo `tool` draws over an entity at `transform`.
pub fn segments(
    tool: Tool,
    camera: &Camera2D,
    viewport: Vec2,
    transform: &Transform,
) -> Vec<Segment> {
    let origin = camera.world_to_viewport(transform.translation, viewport);
    let to_world = |point: Vec2| camera.viewport_to_world(point, viewport);

    match tool {
        // World axes, matching Unity's Global handle orientation: a move is
        // asked for in the direction it is drawn, whatever the entity's facing.
        Tool::Translate => arms(origin, Axes::WORLD, false, &to_world),
        Tool::Rotate => ring(origin, &to_world),
        // Local axes: a scale is applied in the entity's own frame, so an arm
        // drawn along a world axis would grow the entity along a different one.
        Tool::Scale => arms(
            origin,
            Axes::from_rotation(transform.rotation),
            true,
            &to_world,
        ),
    }
}

/// Two arms out of `origin`, with a square at the centre and optional tips.
fn arms(origin: Vec2, axes: Axes, tips: bool, to_world: &impl Fn(Vec2) -> Vec2) -> Vec<Segment> {
    let mut out = Vec::new();

    for (direction, color) in [(axes.x, X_COLOR), (axes.y, Y_COLOR)] {
        let tip = origin + direction * AXIS_LENGTH;
        out.push((to_world(origin), to_world(tip), color));
        if tips {
            // The scale tool's cubes, flattened to squares. Unity draws them to
            // say "this handle ends here and does not translate"; the shape is
            // the only thing telling the two arm gizmos apart at a glance.
            out.extend(square(tip, TIP_HALF, color, to_world));
        }
    }

    out.extend(square(origin, CENTRE_HALF, CENTRE_COLOR, to_world));
    out
}

/// The rotation ring, as a closed polygon.
fn ring(origin: Vec2, to_world: &impl Fn(Vec2) -> Vec2) -> Vec<Segment> {
    let point = |step: usize| {
        let angle = step as f32 / RING_SEGMENTS as f32 * std::f32::consts::TAU;
        let (sin, cos) = angle.sin_cos();
        to_world(origin + Vec2::new(cos, sin) * RING_RADIUS)
    };

    (0..RING_SEGMENTS)
        .map(|step| (point(step), point(step + 1), RING_COLOR))
        .collect()
}

/// An axis-aligned square of half-side `half` pixels, as four segments.
///
/// Four lines rather than a filled quad: a fill would need the sprite pipeline
/// and a texture, and four lines need nothing the overlay does not already do.
fn square(
    centre: Vec2,
    half: f32,
    color: [f32; 4],
    to_world: &impl Fn(Vec2) -> Vec2,
) -> [Segment; 4] {
    let corner = |x: f32, y: f32| to_world(centre + Vec2::new(x, y) * half);
    let (lo, hi) = (-1.0, 1.0);

    [
        (corner(lo, lo), corner(hi, lo), color),
        (corner(hi, lo), corner(hi, hi), color),
        (corner(hi, hi), corner(lo, hi), color),
        (corner(lo, hi), corner(lo, lo), color),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::FRAC_PI_2;

    const VIEWPORT: Vec2 = Vec2::new(800.0, 600.0);

    fn camera(zoom: f32) -> Camera2D {
        Camera2D::new(Vec2::ZERO, zoom, VIEWPORT.x / VIEWPORT.y)
    }

    /// How long a segment is once projected back to the screen.
    fn screen_length(camera: &Camera2D, a: Vec2, b: Vec2) -> f32 {
        camera
            .world_to_viewport(a, VIEWPORT)
            .distance(camera.world_to_viewport(b, VIEWPORT))
    }

    fn at(translation: Vec2) -> Transform {
        Transform::from_translation(translation)
    }

    #[test]
    fn the_arms_are_the_declared_pixel_length() {
        let camera = camera(1.0);
        let drawn = segments(Tool::Translate, &camera, VIEWPORT, &at(Vec2::ZERO));

        for arm in &drawn[..2] {
            let length = screen_length(&camera, arm.0, arm.1);
            assert!((length - AXIS_LENGTH).abs() < 0.01, "{length} px");
        }
    }

    #[test]
    fn the_arms_keep_their_pixel_length_at_any_zoom() {
        // The whole reason the layout happens in screen space. A gizmo laid out
        // in world units doubles on screen when the camera zooms in.
        for zoom in [0.1, 0.5, 1.0, 4.0, 25.0] {
            let camera = Camera2D::new(Vec2::new(3.0, -2.0), zoom, VIEWPORT.x / VIEWPORT.y);
            let drawn = segments(Tool::Translate, &camera, VIEWPORT, &at(Vec2::new(1.0, 1.0)));

            let length = screen_length(&camera, drawn[0].0, drawn[0].1);
            assert!(
                (length - AXIS_LENGTH).abs() < 0.01,
                "at zoom {zoom} the arm was {length} px, not {AXIS_LENGTH}"
            );
        }
    }

    #[test]
    fn the_x_arm_runs_right_and_the_y_arm_runs_up_on_screen() {
        let camera = camera(1.0);
        let drawn = segments(Tool::Translate, &camera, VIEWPORT, &at(Vec2::ZERO));

        let origin = camera.world_to_viewport(Vec2::ZERO, VIEWPORT);
        let x_tip = camera.world_to_viewport(drawn[0].1, VIEWPORT);
        let y_tip = camera.world_to_viewport(drawn[1].1, VIEWPORT);

        assert!(x_tip.x > origin.x, "the x arm must run right on screen");
        // Screen y grows downward, so up is a smaller y.
        assert!(y_tip.y < origin.y, "the y arm must run up on screen");
    }

    #[test]
    fn the_arms_start_at_the_selection() {
        let camera = Camera2D::new(Vec2::new(-4.0, 7.0), 2.0, VIEWPORT.x / VIEWPORT.y);
        let origin = Vec2::new(2.5, -1.5);
        let drawn = segments(Tool::Translate, &camera, VIEWPORT, &at(origin));

        assert!((drawn[0].0 - origin).length() < 1e-4);
        assert!((drawn[1].0 - origin).length() < 1e-4);
    }

    #[test]
    fn the_centre_square_closes_on_itself() {
        let camera = camera(1.0);
        let drawn = segments(Tool::Translate, &camera, VIEWPORT, &at(Vec2::ZERO));

        // Each side starts where the previous one ended, and the last returns
        // to the first. An open square is four strokes that look like three.
        let sides = &drawn[2..6];
        for pair in sides.windows(2) {
            assert_eq!(pair[0].1, pair[1].0);
        }
        assert_eq!(sides[3].1, sides[0].0);
    }

    #[test]
    fn the_centre_square_is_the_declared_pixel_size() {
        let camera = camera(3.0);
        let drawn = segments(Tool::Translate, &camera, VIEWPORT, &at(Vec2::ZERO));

        // Laid out from the centre, so a side spans twice the half-extent.
        let length = screen_length(&camera, drawn[2].0, drawn[2].1);
        assert!(
            (length - CENTRE_HALF * 2.0).abs() < 0.01,
            "a side was {length} px, not {}",
            CENTRE_HALF * 2.0
        );
    }

    #[test]
    fn the_translate_gizmo_ignores_the_entitys_rotation() {
        // Global orientation: a turned entity still moves along world x when
        // the x arm is dragged, so the arm must still point along world x.
        let camera = camera(1.0);
        let turned = at(Vec2::ZERO).with_rotation(FRAC_PI_2);
        let drawn = segments(Tool::Translate, &camera, VIEWPORT, &turned);

        let origin = camera.world_to_viewport(Vec2::ZERO, VIEWPORT);
        let x_tip = camera.world_to_viewport(drawn[0].1, VIEWPORT);

        assert!((x_tip.y - origin.y).abs() < 0.01, "the x arm left world x");
    }

    #[test]
    fn the_scale_gizmo_follows_the_entitys_rotation() {
        let camera = camera(1.0);
        let turned = at(Vec2::ZERO).with_rotation(FRAC_PI_2);
        let drawn = segments(Tool::Scale, &camera, VIEWPORT, &turned);

        let origin = camera.world_to_viewport(Vec2::ZERO, VIEWPORT);
        let x_tip = camera.world_to_viewport(drawn[0].1, VIEWPORT);

        // A quarter turn puts the local x arm up the screen.
        assert!((x_tip.x - origin.x).abs() < 0.01);
        assert!(x_tip.y < origin.y, "the turned x arm must run up on screen");
    }

    #[test]
    fn the_scale_gizmo_caps_each_arm_with_a_square() {
        let camera = camera(1.0);
        let drawn = segments(Tool::Scale, &camera, VIEWPORT, &at(Vec2::ZERO));

        // Two arms, four sides each, plus the centre square: fourteen.
        assert_eq!(drawn.len(), 2 * (1 + 4) + 4);

        let tip = camera.world_to_viewport(drawn[0].1, VIEWPORT);
        for side in &drawn[1..5] {
            let a = camera.world_to_viewport(side.0, VIEWPORT);
            assert!(
                (a.distance(tip) - TIP_HALF * std::f32::consts::SQRT_2).abs() < 0.01,
                "a tip corner sat {} px from the tip",
                a.distance(tip)
            );
        }
    }

    #[test]
    fn the_translate_gizmo_has_no_tips() {
        // The two gizmos are told apart by their tips; equal shapes would make
        // the tool indicator the only difference and that is off in a corner.
        let camera = camera(1.0);
        let drawn = segments(Tool::Translate, &camera, VIEWPORT, &at(Vec2::ZERO));
        assert_eq!(drawn.len(), 2 + 4);
    }

    #[test]
    fn the_ring_is_the_declared_pixel_radius() {
        let camera = camera(0.75);
        let origin = Vec2::new(1.0, -2.0);
        let drawn = segments(Tool::Rotate, &camera, VIEWPORT, &at(origin));

        let centre = camera.world_to_viewport(origin, VIEWPORT);
        for segment in &drawn {
            let radius = camera
                .world_to_viewport(segment.0, VIEWPORT)
                .distance(centre);
            assert!((radius - RING_RADIUS).abs() < 0.01, "{radius} px");
        }
    }

    #[test]
    fn the_ring_closes_on_itself() {
        let camera = camera(1.0);
        let drawn = segments(Tool::Rotate, &camera, VIEWPORT, &at(Vec2::ZERO));

        assert_eq!(drawn.len(), RING_SEGMENTS);
        for pair in drawn.windows(2) {
            assert_eq!(pair[0].1, pair[1].0);
        }
        assert_eq!(drawn[RING_SEGMENTS - 1].1, drawn[0].0);
    }

    #[test]
    fn the_ring_does_not_turn_with_the_entity() {
        // A circle drawn turned is the same circle. Rotating the polygon would
        // only make its vertices crawl round as the entity turns.
        let camera = camera(1.0);
        let still = segments(Tool::Rotate, &camera, VIEWPORT, &at(Vec2::ZERO));
        let turned = segments(
            Tool::Rotate,
            &camera,
            VIEWPORT,
            &at(Vec2::ZERO).with_rotation(1.0),
        );

        assert_eq!(still, turned);
    }
}
