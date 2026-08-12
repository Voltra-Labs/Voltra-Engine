//! Colliders and contacts, as line segments.
//!
//! This is what makes shipping detection without a solver worth doing. A wrong
//! normal is invisible in a number and obvious as a line pointing the wrong
//! way, so the next stage's solver is built against contacts someone has
//! already looked at.

use voltra_ecs::World;
use voltra_render::glam::Vec2;
use voltra_render::LineBatch;
use voltra_scene::{Collider, Transform};

use crate::narrow::Contact;

/// Collider outlines.
const SHAPE_COLOR: [f32; 4] = [0.35, 0.9, 0.45, 0.9];

/// Contacts, in the colour that already means "look here" in this engine.
const CONTACT_COLOR: [f32; 4] = [1.0, 0.2, 1.0, 1.0];

/// Outline thickness, in pixels.
const WIDTH: f32 = 1.5;

/// Segments used to approximate a circle.
///
/// Twenty-four is where the corners stop being visible at the sizes a collider
/// is drawn at. It is a debug overlay, not geometry — doubling it doubles the
/// vertex count to remove something nobody can see.
const CIRCLE_SEGMENTS: usize = 24;

/// How far a contact's normal is drawn, in world units.
///
/// A fixed length rather than the penetration depth: penetration is often a
/// fraction of a pixel, and an arrow that short conveys nothing about the
/// direction, which is the part worth checking.
const NORMAL_LENGTH: f32 = 0.5;

/// Pushes every collider outline, and every contact, into `lines`.
pub fn draw(world: &World, contacts: &[Contact], lines: &mut LineBatch) {
    for (entity, collider) in world.query::<Collider>() {
        let Some(transform) = world.get::<Transform>(entity) else {
            // No transform means no position, so there is nothing to draw and
            // nothing collided with it either — `candidate_pairs` skips it too.
            continue;
        };
        match collider {
            Collider::Box { .. } => {
                // The four turned corners, not the world AABB: the AABB is what
                // the broad phase compares, and drawing it would show a box
                // that grows as it rotates — the outline has to be the shape
                // the narrow phase actually collides.
                outline(lines, &collider.corners(transform), SHAPE_COLOR);
            }
            Collider::Circle { .. } => {
                circle(
                    lines,
                    transform.translation,
                    collider.world_radius(transform),
                    SHAPE_COLOR,
                );
            }
        }
    }

    // One arrow per manifold point, not per contact: a resting box touches its
    // floor at two, and drawing one would hide exactly the case the second
    // point exists for.
    for contact in contacts {
        for point in contact.points() {
            lines.push(
                point.point,
                point.point + contact.normal() * NORMAL_LENGTH,
                WIDTH,
                CONTACT_COLOR,
            );
        }
    }
}

/// A closed polyline through `corners`.
fn outline(lines: &mut LineBatch, corners: &[Vec2; 4], color: [f32; 4]) {
    for i in 0..corners.len() {
        lines.push(corners[i], corners[(i + 1) % corners.len()], WIDTH, color);
    }
}

/// A closed polyline of [`CIRCLE_SEGMENTS`] segments.
fn circle(lines: &mut LineBatch, centre: Vec2, radius: f32, color: [f32; 4]) {
    if radius <= 0.0 {
        return;
    }

    let point = |i: usize| {
        let angle = i as f32 / CIRCLE_SEGMENTS as f32 * std::f32::consts::TAU;
        centre + Vec2::new(angle.cos(), angle.sin()) * radius
    };
    for i in 0..CIRCLE_SEGMENTS {
        lines.push(point(i), point(i + 1), WIDTH, color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::narrow::{Manifold, ManifoldPoint};
    use voltra_ecs::Entity;

    fn box_at(world: &mut World, at: Vec2) -> Entity {
        let entity = world.spawn();
        world.insert(entity, Transform::from_translation(at));
        world.insert(
            entity,
            Collider::Box {
                half_extents: Vec2::splat(1.0),
            },
        );
        entity
    }

    #[test]
    fn an_empty_world_draws_nothing() {
        let world = World::new();
        let mut lines = LineBatch::default();

        draw(&world, &[], &mut lines);

        assert!(lines.is_empty());
    }

    #[test]
    fn a_box_is_four_segments() {
        let mut world = World::new();
        box_at(&mut world, Vec2::ZERO);
        let mut lines = LineBatch::default();

        draw(&world, &[], &mut lines);

        assert_eq!(lines.len(), 4);
    }

    #[test]
    fn a_circle_is_its_segment_count() {
        let mut world = World::new();
        let entity = world.spawn();
        world.insert(entity, Transform::default());
        world.insert(entity, Collider::Circle { radius: 1.0 });
        let mut lines = LineBatch::default();

        draw(&world, &[], &mut lines);

        assert_eq!(lines.len(), CIRCLE_SEGMENTS);
    }

    #[test]
    fn a_contact_adds_exactly_one_segment() {
        let mut world = World::new();
        let a = box_at(&mut world, Vec2::ZERO);
        let b = box_at(&mut world, Vec2::new(1.5, 0.0));
        let contact = Contact::new(
            a,
            b,
            Manifold::new(
                Vec2::X,
                &[ManifoldPoint {
                    point: Vec2::new(0.75, 0.0),
                    separation: -0.5,
                    id: 0,
                }],
            ),
        );
        let mut lines = LineBatch::default();

        draw(&world, &[contact], &mut lines);

        assert_eq!(lines.len(), 4 + 4 + 1);
    }

    #[test]
    fn a_rotated_box_is_drawn_turned_rather_than_bounded() {
        // The AABB of a box turned 45° is √2 wide; the box itself is not. An
        // overlay that draws the bound makes every rotated collider look wrong.
        let mut world = World::new();
        let entity = box_at(&mut world, Vec2::ZERO);
        world.insert(
            entity,
            Transform::default().with_rotation(std::f32::consts::FRAC_PI_4),
        );
        let mut lines = LineBatch::default();

        draw(&world, &[], &mut lines);

        assert_eq!(lines.len(), 4);
        let corners = Collider::Box {
            half_extents: Vec2::splat(1.0),
        }
        .corners(world.get::<Transform>(entity).expect("the transform"));
        let (min, max) = (corners[2], corners[0]);
        assert!(
            (max.y - min.y - 2.0 * std::f32::consts::SQRT_2).abs() < 1e-4,
            "the diagonal is what the AABB would have drawn"
        );
    }

    #[test]
    fn a_two_point_contact_draws_two_normals() {
        let mut world = World::new();
        let a = box_at(&mut world, Vec2::ZERO);
        let b = box_at(&mut world, Vec2::new(1.5, 0.0));
        let contact = Contact::new(
            a,
            b,
            Manifold::new(
                Vec2::X,
                &[
                    ManifoldPoint {
                        point: Vec2::new(0.75, 0.5),
                        separation: -0.5,
                        id: 1,
                    },
                    ManifoldPoint {
                        point: Vec2::new(0.75, -0.5),
                        separation: -0.5,
                        id: 2,
                    },
                ],
            ),
        );
        let mut lines = LineBatch::default();

        draw(&world, &[contact], &mut lines);

        assert_eq!(lines.len(), 4 + 4 + 2);
    }

    #[test]
    fn a_collider_without_a_transform_is_not_drawn() {
        let mut world = World::new();
        let floating = world.spawn();
        world.insert(floating, Collider::Circle { radius: 1.0 });
        let mut lines = LineBatch::default();

        draw(&world, &[], &mut lines);

        assert!(lines.is_empty());
    }

    #[test]
    fn a_degenerate_circle_draws_nothing() {
        // `LineBatch::push` drops a zero-length segment anyway, but a radius of
        // zero would still emit 24 of them and cost the walk.
        let mut world = World::new();
        let entity = world.spawn();
        world.insert(entity, Transform::default());
        world.insert(entity, Collider::Circle { radius: 0.0 });
        let mut lines = LineBatch::default();

        draw(&world, &[], &mut lines);

        assert!(lines.is_empty());
    }
}
