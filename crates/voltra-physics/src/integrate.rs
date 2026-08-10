//! Moving bodies forward by one fixed step.

use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;
use voltra_scene::{BodyType, RigidBody, Transform};

/// Advances every body by `dt`.
///
/// Semi-implicit (symplectic) Euler: velocity is updated from the acceleration
/// *first*, then position from the new velocity. Explicit Euler — position from
/// the old velocity — adds energy to the system, and a stack of boxes slowly
/// climbs. The correct order costs nothing but being written down.
pub fn integrate(world: &mut World, gravity: Vec2, dt: f32) {
    // Collected before applying because `voltra-ecs` has `query` and
    // `query_mut` over one component and `query2` over two immutably, but no
    // way to hold two components mutably at once. The alternative is two
    // passes over the world; one pass and a small `Vec` is the cheaper shape,
    // and this list is one entry per body, not per entity.
    let moved: Vec<(Entity, Vec2)> = world
        .query::<RigidBody>()
        .filter_map(|(entity, body)| {
            let velocity = match body.body_type {
                BodyType::Static => return None,
                // No gravity, no damping: a kinematic body moves exactly as it
                // was told to, which is what makes it usable as a platform.
                BodyType::Kinematic => body.velocity,
                BodyType::Dynamic => {
                    let accelerated = body.velocity + gravity * body.gravity_scale * dt;
                    // Clamped at zero: `1 - damping·dt` goes negative once the
                    // damping is large enough, which turns a brake into a
                    // catapult. A scene file can contain any number.
                    let retained = (1.0 - body.linear_damping * dt).max(0.0);
                    accelerated * retained
                }
            };
            Some((entity, velocity))
        })
        .collect();

    for (entity, velocity) in moved {
        if let Some(body) = world.get_mut::<RigidBody>(entity) {
            body.velocity = velocity;
        }
        // A body with no transform integrates its velocity and moves nothing
        // visible. Legitimate: not everything simulated is drawn.
        if let Some(transform) = world.get_mut::<Transform>(entity) {
            transform.translation += velocity * dt;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use voltra_scene::BodyType;

    const DT: f32 = 1.0 / 60.0;
    const G: Vec2 = Vec2::new(0.0, -10.0);

    /// A world holding one body at the origin, and its entity.
    fn world_with(body: RigidBody) -> (World, Entity) {
        let mut world = World::new();
        let entity = world.spawn();
        world.insert(entity, Transform::default());
        world.insert(entity, body);
        (world, entity)
    }

    fn translation(world: &World, entity: Entity) -> Vec2 {
        world
            .get::<Transform>(entity)
            .expect("the transform is there")
            .translation
    }

    fn velocity(world: &World, entity: Entity) -> Vec2 {
        world
            .get::<RigidBody>(entity)
            .expect("the body is there")
            .velocity
    }

    #[test]
    fn a_dynamic_body_falls() {
        let (mut world, entity) = world_with(RigidBody::new_dynamic(1.0));

        integrate(&mut world, G, DT);

        assert!(translation(&world, entity).y < 0.0);
    }

    #[test]
    fn a_static_body_never_moves() {
        let mut body = RigidBody::new_static();
        body.velocity = Vec2::new(100.0, 100.0);
        let (mut world, entity) = world_with(body);

        integrate(&mut world, G, DT);

        assert_eq!(translation(&world, entity), Vec2::ZERO);
    }

    #[test]
    fn a_kinematic_body_moves_but_ignores_gravity() {
        let body = RigidBody {
            body_type: BodyType::Kinematic,
            velocity: Vec2::new(2.0, 0.0),
            ..Default::default()
        };
        let (mut world, entity) = world_with(body);

        integrate(&mut world, G, DT);

        assert_eq!(translation(&world, entity).x, 2.0 * DT);
        assert_eq!(translation(&world, entity).y, 0.0);
        assert_eq!(
            velocity(&world, entity).y,
            0.0,
            "gravity must not touch a kinematic body's velocity"
        );
    }

    #[test]
    fn velocity_is_updated_before_position() {
        // Semi-implicit Euler. Explicit Euler — position from the *old*
        // velocity — injects energy, and a stack of boxes slowly climbs. After
        // one step from rest the two differ by exactly g·dt², and explicit
        // Euler would leave the body at y = 0.
        let (mut world, entity) = world_with(RigidBody::new_dynamic(1.0));

        integrate(&mut world, G, DT);

        let expected = G.y * DT * DT;
        let actual = translation(&world, entity).y;
        assert!(
            (actual - expected).abs() < 1e-9,
            "got {actual}, expected {expected} — explicit Euler would give 0"
        );
    }

    #[test]
    fn gravity_scale_zero_makes_a_body_float() {
        let body = RigidBody {
            gravity_scale: 0.0,
            ..RigidBody::new_dynamic(1.0)
        };
        let (mut world, entity) = world_with(body);

        integrate(&mut world, G, DT);

        assert_eq!(translation(&world, entity), Vec2::ZERO);
    }

    #[test]
    fn damping_reduces_speed_without_reversing_it() {
        let body = RigidBody {
            velocity: Vec2::new(10.0, 0.0),
            gravity_scale: 0.0,
            linear_damping: 0.5,
            ..RigidBody::new_dynamic(1.0)
        };
        let (mut world, entity) = world_with(body);

        integrate(&mut world, G, DT);

        let v = velocity(&world, entity).x;
        assert!(v < 10.0 && v > 0.0, "got {v}");
    }

    #[test]
    fn enormous_damping_stops_a_body_rather_than_reversing_it() {
        // `v *= 1 - damping·dt` goes negative once damping·dt passes one, which
        // turns a brake into a catapult. A scene file can say 10000.
        let body = RigidBody {
            velocity: Vec2::new(10.0, 0.0),
            gravity_scale: 0.0,
            linear_damping: 10_000.0,
            ..RigidBody::new_dynamic(1.0)
        };
        let (mut world, entity) = world_with(body);

        integrate(&mut world, G, DT);

        let v = velocity(&world, entity).x;
        assert!(v >= 0.0, "damping reversed the velocity: {v}");
    }

    #[test]
    fn a_body_without_a_transform_is_skipped() {
        let mut world = World::new();
        let entity = world.spawn();
        world.insert(entity, RigidBody::new_dynamic(1.0));

        integrate(&mut world, G, DT);

        assert!(world.get::<Transform>(entity).is_none());
        assert!(
            velocity(&world, entity).y < 0.0,
            "it should still accelerate; only the move is skipped"
        );
    }

    #[test]
    fn an_empty_world_integrates_to_nothing() {
        let mut world = World::new();
        integrate(&mut world, G, DT);
        assert_eq!(world.query::<RigidBody>().count(), 0);
    }
}
