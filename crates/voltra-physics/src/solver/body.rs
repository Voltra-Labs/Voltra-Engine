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
    /// The entity the body belongs to.
    pub entity: Entity,
    /// Linear velocity, in world units per second.
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
    /// How the body moves: static, kinematic or dynamic.
    pub body_type: BodyType,
    /// Multiplier on the world's gravity for this body.
    pub gravity_scale: f32,
    /// How quickly linear velocity decays.
    pub linear_damping: f32,
    /// How quickly angular velocity decays.
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

    /// The solver's view of `body`: its velocities, and the mass and inertia
    /// a contact pushes against — also what a character's push reads.
    pub(crate) fn from_body(
        entity: Entity,
        body: &RigidBody,
        shape: Option<(&Collider, &Transform)>,
    ) -> Self {
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
/// `I` per unit of mass comes from the shape ([`super::inertia`]), in world
/// units, so a scaled collider resists exactly as much as it looks like it
/// should.
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

    let denominator = super::inertia::unit_inertia(collider, transform);

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

    /// Treats `entity` as immovable for this step: a sleeping body is not
    /// integrated, is not pushed, and is not written back.
    pub fn freeze(&mut self, entity: Entity) {
        if let Some(&index) = self.index.get(&entity) {
            self.bodies[index] = SolverBody::immovable(entity);
        }
    }

    /// The index of `entity`'s body, if it has one.
    pub fn index_of(&self, entity: Entity) -> Option<usize> {
        self.index.get(&entity).copied()
    }

    /// The body at `index`.
    pub fn get(&self, index: usize) -> &SolverBody {
        &self.bodies[index]
    }

    /// The body at `index`, mutably.
    pub fn get_mut(&mut self, index: usize) -> &mut SolverBody {
        &mut self.bodies[index]
    }

    /// How many bodies there are.
    pub fn len(&self) -> usize {
        self.bodies.len()
    }

    /// Whether there are no bodies.
    pub fn is_empty(&self) -> bool {
        self.bodies.is_empty()
    }

    /// Every body, mutably, in index order.
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
mod tests;
