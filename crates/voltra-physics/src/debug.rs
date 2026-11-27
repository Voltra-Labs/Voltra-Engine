//! Colliders, joints and contacts, as line segments.
//!
//! This is what makes shipping detection without a solver worth doing. A wrong
//! normal is invisible in a number and obvious as a line pointing the wrong
//! way, so the next stage's solver is built against contacts someone has
//! already looked at.

use voltra_ecs::World;
use voltra_render::glam::Vec2;
use voltra_render::LineBatch;
use voltra_scene::{Collider2D, Sensor, Transform2D};

use crate::narrow::Contact;

mod joints;

/// Collider outlines.
const SHAPE_COLOR: [f32; 4] = [0.35, 0.9, 0.45, 0.9];

/// Sensor outlines, in their own colour.
///
/// A trigger is an invisible rectangle by design, so the only way to author
/// one is to see that it is there *and* that nothing will stand on it. Amber
/// rather than a dashed green line: a dash pattern is a per-segment decision
/// this line pipeline does not have, and the colour says the same thing.
const SENSOR_COLOR: [f32; 4] = [1.0, 0.75, 0.2, 0.9];

/// Contacts, in the colour that already means "look here" in this engine.
const CONTACT_COLOR: [f32; 4] = [1.0, 0.2, 1.0, 1.0];

/// Outline thickness, in pixels.
const WIDTH: f32 = 1.5;

/// Segments used to approximate a full turn of a round shape.
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

/// Pushes every collider outline, every joint, and every contact, into
/// `lines`.
pub fn draw(world: &World, contacts: &[Contact], lines: &mut LineBatch) {
    joints::draw(world, lines);
    for (entity, collider) in world.query::<Collider2D>() {
        let Some(transform) = world.get::<Transform2D>(entity) else {
            // No transform means no position, so there is nothing to draw and
            // nothing collided with it either — `candidate_pairs` skips it too.
            continue;
        };
        let color = if world.get::<Sensor>(entity).is_some() {
            SENSOR_COLOR
        } else {
            SHAPE_COLOR
        };
        // The shape the narrow phase collides — a box's turned corners, not
        // the world AABB the broad phase compares, which would grow as it
        // rotates. Nothing for a shape with no area: there is no outline to
        // trust, and the narrow phase ignores it too.
        if !collider.is_degenerate(transform) {
            outline(lines, &collider.outline(transform, CIRCLE_SEGMENTS), color);
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
fn outline(lines: &mut LineBatch, corners: &[Vec2], color: [f32; 4]) {
    for i in 0..corners.len() {
        lines.push(corners[i], corners[(i + 1) % corners.len()], WIDTH, color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::narrow::{Manifold, ManifoldPoint};
    use voltra_ecs::Entity;

    fn box_at(world: &mut World, at: Vec2) -> Entity {
        let entity = world.spawn();
        world.insert(entity, Transform2D::from_translation(at));
        world.insert(
            entity,
            Collider2D::Box {
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
        world.insert(entity, Transform2D::default());
        world.insert(entity, Collider2D::Circle { radius: 1.0 });
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
            Transform2D::default().with_rotation(std::f32::consts::FRAC_PI_4),
        );
        let mut lines = LineBatch::default();

        draw(&world, &[], &mut lines);

        assert_eq!(lines.len(), 4);
        let corners = Collider2D::Box {
            half_extents: Vec2::splat(1.0),
        }
        .corners(world.get::<Transform2D>(entity).expect("the transform"));
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
    fn a_sensor_is_drawn_in_its_own_colour() {
        let mut world = World::new();
        box_at(&mut world, Vec2::ZERO);
        let trigger = box_at(&mut world, Vec2::new(5.0, 0.0));
        world.insert(trigger, Sensor);
        let mut lines = LineBatch::default();

        draw(&world, &[], &mut lines);

        let colors: Vec<_> = lines.vertices().iter().map(|v| v.color).collect();
        assert!(colors.contains(&SENSOR_COLOR), "the trigger");
        assert!(colors.contains(&SHAPE_COLOR), "and the solid box beside it");
    }

    #[test]
    fn a_collider_without_a_transform_is_not_drawn() {
        let mut world = World::new();
        let floating = world.spawn();
        world.insert(floating, Collider2D::Circle { radius: 1.0 });
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
        world.insert(entity, Transform2D::default());
        world.insert(entity, Collider2D::Circle { radius: 0.0 });
        let mut lines = LineBatch::default();

        draw(&world, &[], &mut lines);

        assert!(lines.is_empty());
    }

    #[test]
    fn a_capsule_and_a_polygon_are_drawn_as_their_outlines() {
        let mut world = World::new();
        let capsule = world.spawn();
        world.insert(capsule, Transform2D::default());
        world.insert(
            capsule,
            Collider2D::Capsule {
                half_height: 1.0,
                radius: 0.5,
            },
        );
        let mut lines = LineBatch::default();
        draw(&world, &[], &mut lines);
        let expected = Collider2D::Capsule {
            half_height: 1.0,
            radius: 0.5,
        }
        .outline(&Transform2D::default(), CIRCLE_SEGMENTS)
        .len();
        assert_eq!(lines.len(), expected);

        let mut world = World::new();
        let triangle = world.spawn();
        world.insert(triangle, Transform2D::default());
        world.insert(
            triangle,
            Collider2D::Polygon {
                points: voltra_scene::ConvexPolygon::new(&[Vec2::ZERO, Vec2::X, Vec2::Y])
                    .expect("a triangle"),
                radius: 0.0,
            },
        );
        let mut lines = LineBatch::default();
        draw(&world, &[], &mut lines);
        assert_eq!(lines.len(), 3, "one segment an edge");
    }
}
