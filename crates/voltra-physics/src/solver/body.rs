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
    /// Radians per second, counter-clockwise.
    pub angular_velocity: f32,
    /// How far this body has turned since the step began, in radians. The
    /// rotational twin of `delta_position`, and what the solve rotates a
    /// contact's anchors by to track its separation through the sub-steps.
    pub delta_rotation: f32,
    /// `0.0` for anything a contact cannot push: static geometry, a kinematic
    /// platform, or a dynamic body given a mass of zero.
    pub inverse_mass: f32,
    /// `1 / I`, where `I` is the body's rotational inertia about its centre.
    ///
    /// Derived from the mass and the collider by [`SolverBodies::gather`], not
    /// stored on a component: it is a function of two values a user edits, and
    /// caching it would need an invalidation this ECS has no mechanism for —
    /// dragging the collider's size in the inspector would leave a body
    /// spinning like the shape it used to be.
    ///
    /// `0.0` means *infinitely* resistant to torque, the same reading
    /// `inverse_mass` gives zero: a locked, immovable or collider-less body
    /// does not turn.
    pub inverse_inertia: f32,
    pub body_type: BodyType,
    pub gravity_scale: f32,
    pub linear_damping: f32,
    pub angular_damping: f32,
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
            angular_velocity: 0.0,
            delta_rotation: 0.0,
            inverse_mass: 0.0,
            inverse_inertia: 0.0,
            body_type: BodyType::Static,
            gravity_scale: 0.0,
            linear_damping: 0.0,
            angular_damping: 0.0,
        }
    }

    fn from_body(entity: Entity, body: &RigidBody, shape: Option<(&Collider, &Transform)>) -> Self {
        let (velocity, angular_velocity, inverse_mass) = match body.body_type {
            // A static body's velocity is ignored rather than trusted: the
            // component is writable and a stale value would drag the world.
            BodyType::Static => (Vec2::ZERO, 0.0, 0.0),
            // Kinematic moves under its own velocity and is immovable by
            // contacts, which is exactly what a moving platform is.
            BodyType::Kinematic => (body.velocity, body.angular_velocity, 0.0),
            BodyType::Dynamic => (
                body.velocity,
                body.angular_velocity,
                body.inverse_mass.max(0.0),
            ),
        };

        Self {
            entity,
            velocity,
            delta_position: Vec2::ZERO,
            angular_velocity: if body.lock_rotation {
                0.0
            } else {
                angular_velocity
            },
            delta_rotation: 0.0,
            inverse_mass,
            inverse_inertia: if body.lock_rotation {
                0.0
            } else {
                inverse_inertia(inverse_mass, shape)
            },
            body_type: body.body_type,
            gravity_scale: body.gravity_scale,
            linear_damping: body.linear_damping,
            angular_damping: body.angular_damping,
        }
    }
}

/// `1 / I` for a body of this mass wearing this shape.
///
/// A box turns about its centre with `I = m(hx² + hy²)/3` and a disc with
/// `I = m·r²/2`, both standard and both in world units, so a scaled collider
/// resists exactly as much as it looks like it should.
///
/// Zero — infinite inertia — whenever the answer would be an infinity or a
/// division by zero: a body that cannot be pushed, a body with no collider to
/// give it an extent, or a shape scaled to nothing. An infinite *inverse*
/// inertia would spin a body out of the world on the first contact it took.
fn inverse_inertia(inverse_mass: f32, shape: Option<(&Collider, &Transform)>) -> f32 {
    if inverse_mass <= 0.0 {
        return 0.0;
    }

    let Some((collider, transform)) = shape else {
        return 0.0;
    };

    let denominator = match collider {
        Collider::Box { .. } => {
            let half = collider.world_half_extents(transform);
            (half.x * half.x + half.y * half.y) / 3.0
        }
        Collider::Circle { .. } => {
            let radius = collider.world_radius(transform);
            radius * radius / 2.0
        }
    };

    if denominator > 0.0 {
        inverse_mass / denominator
    } else {
        0.0
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
            // The shape is what gives a mass its inertia, so it is read here
            // rather than stored: a body with no collider never takes a
            // contact and has nothing to turn about.
            let shape = world
                .get::<Collider>(entity)
                .zip(world.get::<Transform>(entity));
            index.insert(entity, bodies.len());
            bodies.push(SolverBody::from_body(entity, body, shape));
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
                component.angular_velocity = body.angular_velocity;
            }

            if let Some(transform) = world.get_mut::<Transform>(body.entity) {
                transform.translation += body.delta_position;
                transform.rotation += body.delta_rotation;
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
    const MAX_ROTATION: f32 = 0.25 * std::f32::consts::PI;

    fn spawn(world: &mut World, body: Option<RigidBody>) -> Entity {
        let entity = world.spawn();
        world.insert(entity, Transform::default());
        if let Some(body) = body {
            world.insert(entity, body);
        }
        entity
    }

    /// One dynamic body of `mass`, wearing `collider`, gathered.
    fn gather_one(collider: Collider, mass: f32) -> SolverBodies {
        let mut world = World::new();
        let entity = spawn(&mut world, Some(RigidBody::new_dynamic(mass)));
        world.insert(entity, collider);
        SolverBodies::gather(&world)
    }

    #[test]
    fn a_box_body_has_the_inertia_of_a_box() {
        // I = m(hx² + hy²)/3, so its inverse is 3·inv_mass/(hx² + hy²).
        let bodies = gather_one(
            Collider::Box {
                half_extents: Vec2::new(2.0, 1.0),
            },
            4.0,
        );

        let expected = 3.0 * 0.25 / (4.0 + 1.0);
        assert!(
            (bodies.get(0).inverse_inertia - expected).abs() < 1e-6,
            "{}",
            bodies.get(0).inverse_inertia
        );
    }

    #[test]
    fn a_circle_body_has_the_inertia_of_a_disc() {
        // I = m·r²/2.
        let bodies = gather_one(Collider::Circle { radius: 2.0 }, 4.0);

        let expected = 2.0 * 0.25 / 4.0;
        assert!(
            (bodies.get(0).inverse_inertia - expected).abs() < 1e-6,
            "{}",
            bodies.get(0).inverse_inertia
        );
    }

    #[test]
    fn the_collider_scale_reaches_the_inertia() {
        // Inertia goes as the square of the extent, so twice the scale is four
        // times as hard to turn. A body that resists like the unscaled shape is
        // a bug that looks like a mass bug.
        let mut world = World::new();
        let entity = spawn(&mut world, Some(RigidBody::new_dynamic(1.0)));
        world.insert(
            entity,
            Collider::Box {
                half_extents: Vec2::splat(1.0),
            },
        );
        world.insert(entity, Transform::default().with_scale(Vec2::splat(2.0)));

        let scaled = SolverBodies::gather(&world).get(0).inverse_inertia;
        let unscaled = gather_one(
            Collider::Box {
                half_extents: Vec2::splat(1.0),
            },
            1.0,
        )
        .get(0)
        .inverse_inertia;

        assert!(
            (scaled - unscaled / 4.0).abs() < 1e-6,
            "{scaled} {unscaled}"
        );
    }

    #[test]
    fn an_immovable_body_has_no_inverse_inertia() {
        // Zero reads as infinite inertia here, exactly as it does for mass.
        let bodies = gather_one(Collider::Circle { radius: 1.0 }, 0.0);
        assert_eq!(bodies.get(0).inverse_inertia, 0.0);

        let mut world = World::new();
        let wall = spawn(&mut world, Some(RigidBody::new_static()));
        world.insert(wall, Collider::Circle { radius: 1.0 });
        assert_eq!(SolverBodies::gather(&world).get(0).inverse_inertia, 0.0);
    }

    #[test]
    fn a_locked_body_has_no_inverse_inertia_and_no_spin() {
        let mut world = World::new();
        let entity = spawn(
            &mut world,
            Some(RigidBody {
                lock_rotation: true,
                angular_velocity: 5.0,
                ..RigidBody::new_dynamic(1.0)
            }),
        );
        world.insert(entity, Collider::Circle { radius: 1.0 });

        let bodies = SolverBodies::gather(&world);

        assert_eq!(bodies.get(0).inverse_inertia, 0.0);
        assert_eq!(bodies.get(0).angular_velocity, 0.0);
    }

    #[test]
    fn a_body_without_a_collider_has_no_inverse_inertia() {
        let mut world = World::new();
        spawn(&mut world, Some(RigidBody::new_dynamic(1.0)));

        assert_eq!(SolverBodies::gather(&world).get(0).inverse_inertia, 0.0);
    }

    #[test]
    fn a_degenerate_collider_does_not_divide_by_zero() {
        // 3·inv_mass/0 is an infinity, and an infinite inverse inertia spins a
        // body out of the world on the first contact it takes.
        let bodies = gather_one(
            Collider::Box {
                half_extents: Vec2::ZERO,
            },
            1.0,
        );

        assert_eq!(bodies.get(0).inverse_inertia, 0.0);
    }

    #[test]
    fn a_spin_is_scattered_back_to_the_component_and_the_transform() {
        let mut world = World::new();
        let entity = spawn(&mut world, Some(RigidBody::new_dynamic(1.0)));
        world.insert(entity, Collider::Circle { radius: 1.0 });

        let mut bodies = SolverBodies::gather(&world);
        for body in bodies.iter_mut() {
            body.angular_velocity = 2.0;
        }
        integrate_positions(&mut bodies, 0.5);
        bodies.scatter(&mut world);

        let rotation = world
            .get::<Transform>(entity)
            .expect("the transform")
            .rotation;
        let spin = world
            .get::<RigidBody>(entity)
            .expect("the body")
            .angular_velocity;

        assert!((rotation - 1.0).abs() < 1e-6, "{rotation}");
        assert!((spin - 2.0).abs() < 1e-6, "{spin}");
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

        integrate_velocities(&mut bodies, G, H, MAX_ROTATION);
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
        integrate_velocities(&mut bodies, G, H, MAX_ROTATION);
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
        integrate_velocities(&mut bodies, G, H, MAX_ROTATION);
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
        integrate_velocities(&mut bodies, G, H, MAX_ROTATION);
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
        integrate_velocities(&mut bodies, Vec2::ZERO, H, MAX_ROTATION);

        let index = bodies.index_of(entity).expect("gathered");
        assert!(bodies.get(index).velocity.x >= 0.0);
    }

    /// One free dynamic body with the given spin and angular damping.
    fn spinning(spin: f32, angular_damping: f32) -> (World, Entity) {
        let mut world = World::new();
        let entity = spawn(
            &mut world,
            Some(RigidBody {
                angular_velocity: spin,
                gravity_scale: 0.0,
                angular_damping,
                ..RigidBody::new_dynamic(1.0)
            }),
        );
        world.insert(entity, Collider::Circle { radius: 1.0 });
        (world, entity)
    }

    #[test]
    fn a_spinning_body_accumulates_a_delta_rotation() {
        let (world, entity) = spinning(2.0, 0.0);
        let mut bodies = SolverBodies::gather(&world);

        integrate_positions(&mut bodies, 0.5);

        let index = bodies.index_of(entity).expect("gathered");
        assert!((bodies.get(index).delta_rotation - 1.0).abs() < 1e-6);
    }

    #[test]
    fn gravity_applies_no_torque() {
        // Gravity acts through the centre of mass. A body that starts spinning
        // when it is dropped is a sign the arm is being taken from the wrong
        // origin.
        let (world, entity) = spinning(0.0, 0.0);
        let mut bodies = SolverBodies::gather(&world);

        integrate_velocities(&mut bodies, G, H, MAX_ROTATION);

        let index = bodies.index_of(entity).expect("gathered");
        assert_eq!(bodies.get(index).angular_velocity, 0.0);
    }

    #[test]
    fn angular_damping_slows_a_spin() {
        let (world, entity) = spinning(4.0, 2.0);
        let mut bodies = SolverBodies::gather(&world);

        integrate_velocities(&mut bodies, Vec2::ZERO, H, MAX_ROTATION);

        let index = bodies.index_of(entity).expect("gathered");
        let spin = bodies.get(index).angular_velocity;
        assert!(spin < 4.0 && spin > 0.0, "{spin}");
    }

    #[test]
    fn a_large_angular_damping_does_not_reverse_the_spin() {
        // The linear twin of this bug is already pinned: `1 − damping·h` goes
        // negative and the brake becomes a catapult, in the other direction.
        let (world, entity) = spinning(4.0, 10_000.0);
        let mut bodies = SolverBodies::gather(&world);

        integrate_velocities(&mut bodies, Vec2::ZERO, H, MAX_ROTATION);

        let index = bodies.index_of(entity).expect("gathered");
        assert!(bodies.get(index).angular_velocity >= 0.0);
    }

    #[test]
    fn a_kinematic_body_keeps_the_spin_it_was_given() {
        let mut world = World::new();
        let entity = spawn(
            &mut world,
            Some(RigidBody {
                body_type: BodyType::Kinematic,
                angular_velocity: 3.0,
                angular_damping: 5.0,
                ..RigidBody::default()
            }),
        );
        world.insert(entity, Collider::Circle { radius: 1.0 });

        let mut bodies = SolverBodies::gather(&world);
        integrate_velocities(&mut bodies, G, H, MAX_ROTATION);

        let index = bodies.index_of(entity).expect("gathered");
        assert_eq!(bodies.get(index).angular_velocity, 3.0);
    }

    #[test]
    fn an_absurd_spin_is_capped_to_a_quarter_turn_per_sub_step() {
        // Past this a body turns its contact face out from under the normal the
        // whole step is solved against.
        let (world, entity) = spinning(1_000.0, 0.0);
        let mut bodies = SolverBodies::gather(&world);

        integrate_velocities(&mut bodies, Vec2::ZERO, H, MAX_ROTATION);

        let index = bodies.index_of(entity).expect("gathered");
        let spin = bodies.get(index).angular_velocity;
        assert!((spin - MAX_ROTATION / H).abs() < 1e-3, "{spin}");
        assert!(spin * H <= MAX_ROTATION + 1e-6, "{spin}");
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
        integrate_velocities(&mut bodies, G, H, MAX_ROTATION);
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
        integrate_velocities(&mut bodies, G, H, MAX_ROTATION);
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
