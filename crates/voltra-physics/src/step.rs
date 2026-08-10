//! One fixed step: move everything, then find what overlaps.

use voltra_ecs::World;
use voltra_render::glam::Vec2;
use voltra_scene::{Collider, Transform};

use crate::broad::candidate_pairs;
use crate::integrate::{integrate_positions, integrate_velocities};
use crate::narrow::{contact, Contact};
use crate::solver::SolverBodies;

/// Advances the world by `dt` and returns what is overlapping afterwards.
///
/// Contacts are returned rather than stored. Nothing consumes them yet but the
/// debug draw, and a `Contacts` resource with no reader would be a structure
/// designed for an imagined caller. The solver takes this list as its input,
/// which is why detection is worth shipping without it: a wrong normal is a
/// line pointing the wrong way on screen, not a number nobody ever sees.
pub fn step(world: &mut World, gravity: Vec2, dt: f32) -> Vec<Contact> {
    let mut bodies = SolverBodies::gather(world);
    integrate_velocities(&mut bodies, gravity, dt);
    integrate_positions(&mut bodies, dt);
    bodies.scatter(world);

    candidate_pairs(world)
        .into_iter()
        .filter_map(|(a, b)| {
            let a_shape = (world.get::<Collider>(a)?, world.get::<Transform>(a)?);
            let b_shape = (world.get::<Collider>(b)?, world.get::<Transform>(b)?);
            let (normal, penetration, point) = contact(a_shape, b_shape)?;
            Some(Contact {
                a,
                b,
                normal,
                penetration,
                point,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use voltra_scene::RigidBody;

    const DT: f32 = 1.0 / 60.0;
    const G: Vec2 = Vec2::new(0.0, -10.0);

    #[test]
    fn a_falling_body_eventually_reaches_the_floor() {
        // The whole stage in one test: it falls, and the overlap is reported.
        // It does not *stop* — nothing resolves contacts yet — so this asserts
        // a contact appears, not that the body comes to rest.
        let mut world = World::new();

        let floor = world.spawn();
        world.insert(floor, Transform::from_translation(Vec2::new(0.0, -5.0)));
        world.insert(
            floor,
            Collider::Aabb {
                half_extents: Vec2::new(10.0, 1.0),
            },
        );
        world.insert(floor, RigidBody::new_static());

        let ball = world.spawn();
        world.insert(ball, Transform::default());
        world.insert(ball, Collider::Circle { radius: 0.5 });
        world.insert(ball, RigidBody::new_dynamic(1.0));

        let mut contacts = Vec::new();
        for _ in 0..240 {
            contacts = step(&mut world, G, DT);
            if !contacts.is_empty() {
                break;
            }
        }

        assert_eq!(contacts.len(), 1, "one contact, between ball and floor");
        // The floor is `a` — lower entity index — so its normal points up,
        // away from the ball.
        assert!(
            contacts[0].normal.y < -0.5 || contacts[0].normal.y > 0.5,
            "the contact must separate them vertically, got {:?}",
            contacts[0].normal
        );
    }

    #[test]
    fn a_falling_body_keeps_going_through_the_floor() {
        // This stage's stated limit, pinned so that fixing it in the solver
        // stage is a deliberate change to a test rather than a surprise.
        let mut world = World::new();

        let floor = world.spawn();
        world.insert(floor, Transform::from_translation(Vec2::new(0.0, -2.0)));
        world.insert(
            floor,
            Collider::Aabb {
                half_extents: Vec2::new(10.0, 0.5),
            },
        );

        let ball = world.spawn();
        world.insert(ball, Transform::default());
        world.insert(ball, Collider::Circle { radius: 0.5 });
        world.insert(ball, RigidBody::new_dynamic(1.0));

        for _ in 0..600 {
            step(&mut world, G, DT);
        }

        let y = world
            .get::<Transform>(ball)
            .expect("transform")
            .translation
            .y;
        assert!(y < -3.0, "nothing resolves contacts yet, so it sinks: {y}");
    }

    #[test]
    fn an_empty_world_steps_without_contacts() {
        let mut world = World::new();
        assert!(step(&mut world, G, DT).is_empty());
    }

    #[test]
    fn a_body_with_no_collider_moves_and_never_collides() {
        let mut world = World::new();
        let entity = world.spawn();
        world.insert(entity, Transform::default());
        world.insert(entity, RigidBody::new_dynamic(1.0));

        let contacts = step(&mut world, G, DT);

        assert!(contacts.is_empty());
        assert!(
            world
                .get::<Transform>(entity)
                .expect("transform")
                .translation
                .y
                < 0.0
        );
    }

    #[test]
    fn a_collider_with_no_body_is_static_geometry() {
        let mut world = World::new();
        let wall = world.spawn();
        world.insert(wall, Transform::default());
        world.insert(
            wall,
            Collider::Aabb {
                half_extents: Vec2::splat(1.0),
            },
        );

        let ball = world.spawn();
        world.insert(ball, Transform::from_translation(Vec2::new(0.5, 0.0)));
        world.insert(ball, Collider::Circle { radius: 0.5 });

        assert_eq!(step(&mut world, G, DT).len(), 1);
        assert_eq!(
            world.get::<Transform>(wall).expect("transform").translation,
            Vec2::ZERO,
            "a collider with no body must not move"
        );
    }

    #[test]
    fn the_contact_names_the_entities_it_is_between() {
        let mut world = World::new();
        let a = world.spawn();
        world.insert(a, Transform::default());
        world.insert(a, Collider::Circle { radius: 1.0 });
        let b = world.spawn();
        world.insert(b, Transform::from_translation(Vec2::new(1.0, 0.0)));
        world.insert(b, Collider::Circle { radius: 1.0 });

        let contacts = step(&mut world, G, DT);

        assert_eq!(contacts.len(), 1);
        assert_eq!((contacts[0].a, contacts[0].b), (a, b));
    }

    #[test]
    fn a_scene_of_resting_shapes_reports_the_same_contacts_every_step() {
        // Detection must be a function of the state, not of how many times it
        // has been called. Anything that accumulates here shows up as contacts
        // multiplying while nothing moves.
        let mut world = World::new();
        let a = world.spawn();
        world.insert(a, Transform::default());
        world.insert(a, Collider::Circle { radius: 1.0 });
        let b = world.spawn();
        world.insert(b, Transform::from_translation(Vec2::new(1.0, 0.0)));
        world.insert(b, Collider::Circle { radius: 1.0 });

        let first = step(&mut world, Vec2::ZERO, DT);
        let second = step(&mut world, Vec2::ZERO, DT);

        assert_eq!(first, second);
    }
}
