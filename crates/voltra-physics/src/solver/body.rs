//! The bodies one step solves over, gathered out of the ECS and put back.
//!
//! The solver does not read components directly, for two reasons that have the
//! same answer. Sub-stepping needs a per-body *delta position* accumulated
//! across the sub-steps, which no component holds and none should — it is
//! scratch space for one step. And `voltra-ecs` hands out one mutable component
//! at a time, while every impulse touches two bodies at once.
//!
//! Every engine solves this the same way: a dense array of solver bodies,
//! gathered at the start of the step and scattered back at the end. Being dense
//! is also why the constraints can hold indices rather than entity lookups.

use std::collections::HashMap;

use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;
use voltra_scene::{BodyType, Collider, RigidBody, Transform};

/// One body as the solver sees it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SolverBody {
    pub entity: Entity,
    pub velocity: Vec2,
    /// How far this body has moved since the step began. Applied to the
    /// `Transform` once, at scatter, so a contact's separation can be tracked
    /// analytically through the sub-steps without touching the ECS.
    pub delta_position: Vec2,
    /// `0.0` for anything a contact cannot push: static geometry, a kinematic
    /// platform, or a dynamic body given a mass of zero.
    pub inverse_mass: f32,
    pub body_type: BodyType,
    pub gravity_scale: f32,
    pub linear_damping: f32,
}

impl SolverBody {
    /// A body that cannot be pushed and does not move itself.
    ///
    /// What a collider with no `RigidBody` becomes — static level geometry,
    /// which the scene format has always allowed.
    fn immovable(entity: Entity) -> Self {
        Self {
            entity,
            velocity: Vec2::ZERO,
            delta_position: Vec2::ZERO,
            inverse_mass: 0.0,
            body_type: BodyType::Static,
            gravity_scale: 0.0,
            linear_damping: 0.0,
        }
    }

    fn from_body(entity: Entity, body: &RigidBody) -> Self {
        let (velocity, inverse_mass) = match body.body_type {
            // A static body's velocity is ignored rather than trusted: the
            // component is writable and a stale value would drag the world.
            BodyType::Static => (Vec2::ZERO, 0.0),
            // Kinematic moves under its own velocity and is immovable by
            // contacts, which is exactly what a moving platform is.
            BodyType::Kinematic => (body.velocity, 0.0),
            BodyType::Dynamic => (body.velocity, body.inverse_mass.max(0.0)),
        };

        Self {
            entity,
            velocity,
            delta_position: Vec2::ZERO,
            inverse_mass,
            body_type: body.body_type,
            gravity_scale: body.gravity_scale,
            linear_damping: body.linear_damping,
        }
    }
}

/// Every body a step simulates, in a dense array with an index by entity.
#[derive(Debug, Default)]
pub struct SolverBodies {
    bodies: Vec<SolverBody>,
    index: HashMap<Entity, usize>,
}

impl SolverBodies {
    /// Collects every entity the solver has to know about.
    ///
    /// That is everything with a `RigidBody` **or** a `Collider`: a collider
    /// with no body is static geometry, and it has to be present or a contact
    /// against the world would have no second body to push off. Giving it an
    /// entry rather than a special case is why nothing downstream branches on
    /// whether a body exists.
    pub fn gather(world: &World) -> Self {
        let mut bodies = Vec::new();
        let mut index = HashMap::new();

        for (entity, body) in world.query::<RigidBody>() {
            index.insert(entity, bodies.len());
            bodies.push(SolverBody::from_body(entity, body));
        }

        for (entity, _) in world.query::<Collider>() {
            index.entry(entity).or_insert_with(|| {
                bodies.push(SolverBody::immovable(entity));
                bodies.len() - 1
            });
        }

        Self { bodies, index }
    }

    pub fn index_of(&self, entity: Entity) -> Option<usize> {
        self.index.get(&entity).copied()
    }

    pub fn get(&self, index: usize) -> &SolverBody {
        &self.bodies[index]
    }

    pub fn len(&self) -> usize {
        self.bodies.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bodies.is_empty()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut SolverBody> {
        self.bodies.iter_mut()
    }

    /// Both ends of a contact, mutably.
    ///
    /// The borrow checker will not hand out two `&mut` from one slice, and the
    /// pair is what every impulse needs. `split_at_mut` is the way round it
    /// that stays safe.
    ///
    /// # Panics
    ///
    /// If `a == b`. The broad phase starts its inner loop at `i + 1`, so a body
    /// is never paired with itself; a panic here would mean that invariant
    /// broke.
    pub fn pair_mut(&mut self, a: usize, b: usize) -> (&mut SolverBody, &mut SolverBody) {
        assert_ne!(a, b, "a body cannot be in contact with itself");

        if a < b {
            let (left, right) = self.bodies.split_at_mut(b);
            (&mut left[a], &mut right[0])
        } else {
            let (left, right) = self.bodies.split_at_mut(a);
            (&mut right[0], &mut left[b])
        }
    }

    /// Writes the solved velocities and moves back into the world.
    ///
    /// A static body is skipped on both counts. A body with no `Transform`
    /// still keeps its velocity and moves nothing visible, which is legitimate:
    /// not everything simulated is drawn.
    pub fn scatter(self, world: &mut World) {
        for body in self.bodies {
            if body.body_type == BodyType::Static {
                continue;
            }

            if let Some(component) = world.get_mut::<RigidBody>(body.entity) {
                component.velocity = body.velocity;
            }

            if let Some(transform) = world.get_mut::<Transform>(body.entity) {
                transform.translation += body.delta_position;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::integrate::{integrate_positions, integrate_velocities};

    const H: f32 = 1.0 / 60.0;
    const G: Vec2 = Vec2::new(0.0, -10.0);

    fn spawn(world: &mut World, body: Option<RigidBody>) -> Entity {
        let entity = world.spawn();
        world.insert(entity, Transform::default());
        if let Some(body) = body {
            world.insert(entity, body);
        }
        entity
    }

    #[test]
    fn a_collider_without_a_body_is_gathered_as_immovable() {
        let mut world = World::new();
        let wall = world.spawn();
        world.insert(wall, Transform::default());
        world.insert(wall, Collider::Circle { radius: 1.0 });

        let bodies = SolverBodies::gather(&world);
        let index = bodies.index_of(wall).expect("gathered");

        assert_eq!(bodies.get(index).inverse_mass, 0.0);
        assert_eq!(bodies.get(index).velocity, Vec2::ZERO);
    }

    #[test]
    fn a_body_with_a_collider_is_gathered_once() {
        let mut world = World::new();
        let entity = spawn(&mut world, Some(RigidBody::new_dynamic(1.0)));
        world.insert(entity, Collider::Circle { radius: 1.0 });

        let bodies = SolverBodies::gather(&world);

        assert_eq!(bodies.len(), 1);
        assert!(
            bodies.get(0).inverse_mass > 0.0,
            "the body won, not the hull"
        );
    }

    #[test]
    fn a_kinematic_body_moves_itself_but_cannot_be_pushed() {
        let mut world = World::new();
        let platform = spawn(
            &mut world,
            Some(RigidBody {
                body_type: BodyType::Kinematic,
                velocity: Vec2::new(2.0, 0.0),
                ..Default::default()
            }),
        );

        let mut bodies = SolverBodies::gather(&world);
        let index = bodies.index_of(platform).expect("gathered");
        assert_eq!(bodies.get(index).inverse_mass, 0.0, "immovable by contacts");

        integrate_velocities(&mut bodies, G, H);
        integrate_positions(&mut bodies, H);

        assert_eq!(
            bodies.get(index).velocity,
            Vec2::new(2.0, 0.0),
            "gravity must not touch a kinematic body"
        );
        assert!(bodies.get(index).delta_position.x > 0.0, "it still moves");
    }

    #[test]
    fn a_static_body_is_never_moved_by_integration() {
        let mut world = World::new();
        let mut body = RigidBody::new_static();
        body.velocity = Vec2::new(100.0, 100.0);
        let entity = spawn(&mut world, Some(body));

        let mut bodies = SolverBodies::gather(&world);
        integrate_velocities(&mut bodies, G, H);
        integrate_positions(&mut bodies, H);
        bodies.scatter(&mut world);

        assert_eq!(
            world
                .get::<Transform>(entity)
                .expect("transform")
                .translation,
            Vec2::ZERO
        );
    }

    #[test]
    fn velocity_is_integrated_before_position() {
        // Semi-implicit Euler, unchanged from 11b-1: after one step from rest
        // the body has moved by exactly g·h², and explicit Euler would leave it
        // at zero. The difference is the order of two lines and a stack of
        // boxes climbing on its own.
        let mut world = World::new();
        let entity = spawn(&mut world, Some(RigidBody::new_dynamic(1.0)));

        let mut bodies = SolverBodies::gather(&world);
        integrate_velocities(&mut bodies, G, H);
        integrate_positions(&mut bodies, H);

        let index = bodies.index_of(entity).expect("gathered");
        let expected = G.y * H * H;
        let actual = bodies.get(index).delta_position.y;
        assert!(
            (actual - expected).abs() < 1e-9,
            "got {actual}, expected {expected} — explicit Euler would give 0"
        );
    }

    #[test]
    fn gravity_scale_zero_makes_a_body_float() {
        let mut world = World::new();
        let entity = spawn(
            &mut world,
            Some(RigidBody {
                gravity_scale: 0.0,
                ..RigidBody::new_dynamic(1.0)
            }),
        );

        let mut bodies = SolverBodies::gather(&world);
        integrate_velocities(&mut bodies, G, H);
        integrate_positions(&mut bodies, H);

        let index = bodies.index_of(entity).expect("gathered");
        assert_eq!(bodies.get(index).delta_position, Vec2::ZERO);
    }

    #[test]
    fn enormous_damping_stops_a_body_rather_than_reversing_it() {
        // `v *= 1 − damping·h` goes negative once damping·h passes one, which
        // turns a brake into a catapult. A scene file can say 10 000.
        let mut world = World::new();
        let entity = spawn(
            &mut world,
            Some(RigidBody {
                velocity: Vec2::new(10.0, 0.0),
                gravity_scale: 0.0,
                linear_damping: 10_000.0,
                ..RigidBody::new_dynamic(1.0)
            }),
        );

        let mut bodies = SolverBodies::gather(&world);
        integrate_velocities(&mut bodies, Vec2::ZERO, H);

        let index = bodies.index_of(entity).expect("gathered");
        assert!(bodies.get(index).velocity.x >= 0.0);
    }

    #[test]
    fn a_negative_inverse_mass_cannot_be_pushed_backwards() {
        // A scene file is external input, and a negative inverse mass would
        // send a body *towards* whatever pushed it.
        let mut world = World::new();
        let entity = spawn(
            &mut world,
            Some(RigidBody {
                inverse_mass: -1.0,
                ..RigidBody::new_dynamic(1.0)
            }),
        );

        let bodies = SolverBodies::gather(&world);
        let index = bodies.index_of(entity).expect("gathered");

        assert_eq!(bodies.get(index).inverse_mass, 0.0);
    }

    #[test]
    fn scatter_writes_velocity_and_the_accumulated_move_back() {
        let mut world = World::new();
        let entity = spawn(&mut world, Some(RigidBody::new_dynamic(1.0)));

        let mut bodies = SolverBodies::gather(&world);
        integrate_velocities(&mut bodies, G, H);
        integrate_positions(&mut bodies, H);
        bodies.scatter(&mut world);

        assert!(
            world
                .get::<Transform>(entity)
                .expect("transform")
                .translation
                .y
                < 0.0,
            "it should have moved down"
        );
        assert!(world.get::<RigidBody>(entity).expect("body").velocity.y < 0.0);
    }

    #[test]
    fn a_body_without_a_transform_simulates_and_moves_nothing() {
        let mut world = World::new();
        let entity = world.spawn();
        world.insert(entity, RigidBody::new_dynamic(1.0));

        let mut bodies = SolverBodies::gather(&world);
        integrate_velocities(&mut bodies, G, H);
        integrate_positions(&mut bodies, H);
        bodies.scatter(&mut world);

        assert!(world.get::<Transform>(entity).is_none());
        assert!(
            world.get::<RigidBody>(entity).expect("body").velocity.y < 0.0,
            "it should still accelerate; only the move is skipped"
        );
    }

    #[test]
    fn an_empty_world_gathers_nothing() {
        assert!(SolverBodies::gather(&World::new()).is_empty());
    }

    #[test]
    fn a_pair_hands_out_both_ends() {
        let mut world = World::new();
        let first = spawn(&mut world, Some(RigidBody::new_dynamic(1.0)));
        let second = spawn(&mut world, Some(RigidBody::new_dynamic(2.0)));

        let mut bodies = SolverBodies::gather(&world);
        let (a, b) = bodies.pair_mut(bodies_index(&bodies, first), bodies_index(&bodies, second));

        assert_eq!(a.entity, first);
        assert_eq!(b.entity, second);
    }

    #[test]
    fn a_pair_in_either_order_names_the_same_bodies() {
        let mut world = World::new();
        let first = spawn(&mut world, Some(RigidBody::new_dynamic(1.0)));
        let second = spawn(&mut world, Some(RigidBody::new_dynamic(2.0)));

        let mut bodies = SolverBodies::gather(&world);
        let (i, j) = (bodies_index(&bodies, first), bodies_index(&bodies, second));
        let (a, b) = bodies.pair_mut(j, i);

        assert_eq!(a.entity, second);
        assert_eq!(b.entity, first);
    }

    fn bodies_index(bodies: &SolverBodies, entity: Entity) -> usize {
        bodies.index_of(entity).expect("gathered")
    }
}
