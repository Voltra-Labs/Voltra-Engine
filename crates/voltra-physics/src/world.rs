//! What a simulated world owns between frames.

use voltra_ecs::World;
use voltra_render::glam::Vec2;

use crate::clock::PhysicsClock;
use crate::narrow::Contact;
use crate::solver::{ImpulseCache, SolverParams};
use crate::step::step;

/// The physics simulation's own state, beside the ECS.
///
/// 11b-1 got away with a free `step` function because nothing survived a step.
/// Warm starting ends that: a contact has to be handed the impulse it needed
/// last time or a stack sinks under its own weight. Something must remember,
/// and this is the crate's owner for everything of that kind — the fixed clock
/// and its accumulated debt, the tuning, the impulses, and later the sleeping
/// islands, the joints and the collision events.
#[derive(Debug, Default)]
pub struct PhysicsWorld {
    clock: PhysicsClock,
    params: SolverParams,
    impulses: ImpulseCache,
    contacts: Vec<Contact>,
}

impl PhysicsWorld {
    /// A world stepping at the default rate of 60 Hz.
    pub fn new() -> Self {
        Self::default()
    }

    /// A world stepping at `step` seconds. A non-positive step falls back to
    /// the default, as [`PhysicsClock::new`] does.
    pub fn with_step(step: f32) -> Self {
        Self {
            clock: PhysicsClock::new(step),
            ..Default::default()
        }
    }

    pub fn params(&self) -> &SolverParams {
        &self.params
    }

    /// The tuning, to change. Nothing is recomputed from it until the next
    /// step, so any change takes effect on the next frame rather than midway
    /// through one.
    pub fn params_mut(&mut self) -> &mut SolverParams {
        &mut self.params
    }

    /// What the last step that ran found overlapping.
    ///
    /// A frame that owed no step leaves this alone rather than blanking it: the
    /// contacts are still there, nothing has moved, and an overlay that flashed
    /// off on fast frames would be reporting the frame rate, not the scene.
    pub fn contacts(&self) -> &[Contact] {
        &self.contacts
    }

    /// Pairs currently warm-started. For tests and for a debug panel; the
    /// number is bounded by the last step's contacts.
    pub fn cached_pairs(&self) -> usize {
        self.impulses.len()
    }

    /// Forgets every accumulated impulse and the time the clock has banked.
    ///
    /// For a world that is no longer the same world — a scene load, or the
    /// editor's Stop. Keeping the impulses would warm-start the new scene's
    /// contacts with the old scene's forces for one step; keeping the banked
    /// time would open the next session by stepping the previous one's debt.
    pub fn reset(&mut self) {
        self.impulses.clear();
        self.contacts.clear();
        self.clock.reset();
    }

    /// Runs exactly one fixed step, whatever the clock has banked.
    ///
    /// What an editor's Step button asks for: a paused world must advance by
    /// one step and no more, and [`PhysicsWorld::advance`] would run zero.
    pub fn step_once(&mut self, world: &mut World, gravity: Vec2) -> &[Contact] {
        let dt = self.clock.step();
        self.contacts = step(world, &mut self.impulses, &self.params, gravity, dt);
        &self.contacts
    }

    /// Runs however many fixed steps a frame of `delta` seconds owes.
    ///
    /// The clock caps that count and drops the excess, so a stalled frame makes
    /// simulated time run slow rather than making the next frame slower still.
    ///
    /// Written in terms of [`PhysicsWorld::step_once`] so there is exactly one
    /// place in the crate where a step happens.
    pub fn advance(&mut self, world: &mut World, gravity: Vec2, delta: f32) -> &[Contact] {
        let owed = self.clock.steps(delta);
        for _ in 0..owed {
            self.step_once(world, gravity);
        }
        &self.contacts
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use voltra_ecs::Entity;
    use voltra_scene::{Collider, RigidBody, Transform};

    const G: Vec2 = Vec2::new(0.0, -10.0);

    fn floor_and_ball() -> (World, Entity) {
        let mut world = World::new();

        let floor = world.spawn();
        world.insert(floor, Transform::from_translation(Vec2::new(0.0, -2.0)));
        world.insert(
            floor,
            Collider::Box {
                half_extents: Vec2::new(10.0, 0.5),
            },
        );
        world.insert(floor, RigidBody::new_static());

        let ball = world.spawn();
        world.insert(ball, Transform::default());
        world.insert(ball, Collider::Circle { radius: 0.5 });
        world.insert(ball, RigidBody::new_dynamic(1.0));

        (world, ball)
    }

    fn height(world: &World, entity: Entity) -> f32 {
        world
            .get::<Transform>(entity)
            .expect("the transform is there")
            .translation
            .y
    }

    #[test]
    fn a_frame_shorter_than_the_step_simulates_nothing_yet() {
        let mut physics = PhysicsWorld::new();
        let (mut world, ball) = floor_and_ball();

        assert!(physics.advance(&mut world, G, 1.0 / 600.0).is_empty());
        assert_eq!(height(&world, ball), 0.0);
    }

    #[test]
    fn a_long_frame_is_capped_rather_than_spiralling() {
        // The clock's eight-step cap, exercised through the solver: a ten
        // second frame must not run six hundred steps.
        let mut physics = PhysicsWorld::new();
        let (mut world, ball) = floor_and_ball();

        physics.advance(&mut world, G, 10.0);

        let y = height(&world, ball);
        assert!(y > -1.5, "at most eight steps of falling, got {y}");
    }

    #[test]
    fn contacts_survive_a_frame_that_owed_no_step() {
        let mut physics = PhysicsWorld::new();
        let (mut world, _) = floor_and_ball();
        for _ in 0..300 {
            physics.advance(&mut world, G, 1.0 / 60.0);
        }
        assert!(!physics.contacts().is_empty(), "it should be resting");

        physics.advance(&mut world, G, 0.0);

        assert!(
            !physics.contacts().is_empty(),
            "a frame with no step must not blank them"
        );
    }

    #[test]
    fn a_body_dropped_through_the_world_comes_to_rest() {
        let mut physics = PhysicsWorld::new();
        let (mut world, ball) = floor_and_ball();

        for _ in 0..300 {
            physics.advance(&mut world, G, 1.0 / 60.0);
        }

        assert!((height(&world, ball) + 1.0).abs() < 0.05);
        assert!(physics.cached_pairs() > 0, "the resting contact stays warm");
    }

    #[test]
    fn resetting_forgets_the_accumulated_impulses() {
        let mut physics = PhysicsWorld::new();
        let (mut world, _) = floor_and_ball();
        for _ in 0..300 {
            physics.advance(&mut world, G, 1.0 / 60.0);
        }
        assert!(physics.cached_pairs() > 0);

        physics.reset();

        assert_eq!(physics.cached_pairs(), 0);
        assert!(physics.contacts().is_empty());
    }

    #[test]
    fn step_once_advances_exactly_one_step() {
        // Same displacement as a frame that owed exactly one step, which is
        // what "one fixed step" has to mean for the editor's Step button.
        let mut once = PhysicsWorld::new();
        let (mut world_a, ball_a) = floor_and_ball();
        once.step_once(&mut world_a, G);

        let mut advanced = PhysicsWorld::new();
        let (mut world_b, ball_b) = floor_and_ball();
        advanced.advance(&mut world_b, G, 1.0 / 60.0);

        assert!(height(&world_a, ball_a) < 0.0, "it must have fallen");
        assert_eq!(height(&world_a, ball_a), height(&world_b, ball_b));
    }

    #[test]
    fn step_once_ignores_the_banked_time() {
        // The accumulator is the clock's business. A Step press must run one
        // step and neither spend nor add to the debt a frame is carrying.
        let mut physics = PhysicsWorld::new();
        let (mut world, ball) = floor_and_ball();
        physics.advance(&mut world, G, 1.0 / 600.0);
        assert_eq!(height(&world, ball), 0.0, "a tenth of a step runs nothing");

        physics.step_once(&mut world, G);
        let after_one = height(&world, ball);

        physics.advance(&mut world, G, 0.0);
        assert_eq!(
            height(&world, ball),
            after_one,
            "the step must not have consumed or topped up the bank"
        );
    }

    #[test]
    fn resetting_also_clears_the_clock() {
        let mut physics = PhysicsWorld::new();
        let (mut world, ball) = floor_and_ball();
        physics.advance(&mut world, G, (1.0 / 60.0) * 0.9);
        assert_eq!(height(&world, ball), 0.0, "0.9 of a step owes nothing yet");

        physics.reset();

        physics.advance(&mut world, G, (1.0 / 60.0) * 0.2);
        assert_eq!(height(&world, ball), 0.0, "the banked 0.9 must be gone");
    }

    #[test]
    fn a_world_stepping_at_zero_falls_back_to_the_default_rate() {
        let mut physics = PhysicsWorld::with_step(0.0);
        let (mut world, ball) = floor_and_ball();

        physics.advance(&mut world, G, 1.0 / 60.0);

        assert!(height(&world, ball) < 0.0, "it should have run one step");
    }
}
