//! What a simulated world owns between frames.

use voltra_ecs::World;
use voltra_render::glam::Vec2;

use crate::clock::PhysicsClock;
use crate::events::{CollisionEvent, Touching};
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
    touching: Touching,
    events: Vec<CollisionEvent>,
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

    /// What began and ended touching in the last step that ran.
    ///
    /// A stream, unlike [`PhysicsWorld::contacts`], which is state: an event
    /// is handed out once and the next step replaces it. Read it from a game's
    /// *fixed* tick, which runs exactly once per step — a per-frame reader
    /// would see the same event on every frame that owed no step, and take the
    /// same pickup twice.
    pub fn events(&self) -> &[CollisionEvent] {
        &self.events
    }

    /// Pairs currently touching, sensors included. For tests and for a debug
    /// panel.
    pub fn touching(&self) -> &Touching {
        &self.touching
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
        // Cleared, not diffed: the next step must not open by ending every
        // pair of a scene that is no longer loaded.
        self.touching.clear();
        self.events.clear();
        self.clock.reset();
    }

    /// Runs exactly one fixed step, whatever the clock has banked.
    ///
    /// What an editor's Step button asks for: a paused world must advance by
    /// one step and no more, and [`PhysicsWorld::advance`] would run zero.
    pub fn step_once(&mut self, world: &mut World, gravity: Vec2) -> &[Contact] {
        let dt = self.clock.step();
        let overlaps = step(world, &mut self.impulses, &self.params, gravity, dt);
        self.events = self.touching.update(overlaps.pairs());
        self.contacts = overlaps.contacts;
        &self.contacts
    }

    /// The fixed step, in seconds: how much time one step covers.
    ///
    /// What a caller tells a game's fixed tick it is being run for. Read off
    /// the clock rather than kept anywhere else, so `with_step` stays the one
    /// place the rate is decided.
    pub fn step(&self) -> f32 {
        self.clock.step()
    }

    /// How many fixed steps a frame of `delta` seconds owes, banked time
    /// included and the debt consumed.
    ///
    /// [`PhysicsWorld::advance`] is this plus the loop. It is separate because
    /// a caller that has to do something *between* steps — run a game's fixed
    /// tick, in `voltra-core` — cannot use `advance` and must not own a second
    /// copy of the clock to work the count out for itself. Asking twice in one
    /// frame returns zero the second time, which is what consuming the debt
    /// means.
    pub fn owed_steps(&mut self, delta: f32) -> u32 {
        self.clock.steps(delta)
    }

    /// Runs however many fixed steps a frame of `delta` seconds owes.
    ///
    /// The clock caps that count and drops the excess, so a stalled frame makes
    /// simulated time run slow rather than making the next frame slower still.
    ///
    /// Written in terms of [`PhysicsWorld::step_once`] so there is exactly one
    /// place in the crate where a step happens.
    pub fn advance(&mut self, world: &mut World, gravity: Vec2, delta: f32) -> &[Contact] {
        let owed = self.owed_steps(delta);
        for _ in 0..owed {
            self.step_once(world, gravity);
        }
        &self.contacts
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::Touch;
    use voltra_ecs::Entity;
    use voltra_scene::{Collider, RigidBody, Sensor, Transform};

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

    fn translation(world: &World, entity: Entity) -> Vec2 {
        world
            .get::<Transform>(entity)
            .expect("the transform is there")
            .translation
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
    fn the_owed_steps_are_consumed_by_asking() {
        // A caller that steps them itself must not be able to ask twice and
        // run the same debt twice.
        let mut physics = PhysicsWorld::new();

        assert_eq!(physics.owed_steps(2.5 / 60.0), 2);
        assert_eq!(
            physics.owed_steps(0.0),
            0,
            "the debt was already handed out"
        );
        assert_eq!(
            physics.owed_steps(0.6 / 60.0),
            1,
            "and the half step left over is still banked"
        );
    }

    #[test]
    fn asking_for_the_owed_steps_is_what_advance_does() {
        let mut counted = PhysicsWorld::new();
        let (mut counted_world, counted_ball) = floor_and_ball();
        for _ in 0..counted.owed_steps(3.0 / 60.0) {
            counted.step_once(&mut counted_world, G);
        }

        let mut advanced = PhysicsWorld::new();
        let (mut advanced_world, advanced_ball) = floor_and_ball();
        advanced.advance(&mut advanced_world, G, 3.0 / 60.0);

        assert_eq!(
            height(&counted_world, counted_ball),
            height(&advanced_world, advanced_ball)
        );
    }

    /// A static box at the origin and a dynamic ball sitting inside it.
    fn overlapping(sensor: bool) -> (PhysicsWorld, World, Entity, Entity) {
        let mut world = World::new();

        let trigger = world.spawn();
        world.insert(trigger, Transform::default());
        world.insert(
            trigger,
            Collider::Box {
                half_extents: Vec2::splat(1.0),
            },
        );
        if sensor {
            world.insert(trigger, Sensor);
        }

        let ball = world.spawn();
        world.insert(ball, Transform::from_translation(Vec2::new(0.25, 0.0)));
        world.insert(ball, Collider::Circle { radius: 0.5 });
        world.insert(ball, RigidBody::new_dynamic(1.0));

        (PhysicsWorld::new(), world, trigger, ball)
    }

    #[test]
    fn a_sensor_is_detected_and_never_solved() {
        let (mut physics, mut world, trigger, ball) = overlapping(true);
        let before = translation(&world, ball);

        for _ in 0..10 {
            physics.step_once(&mut world, Vec2::ZERO);
        }

        assert!(
            physics.contacts().is_empty(),
            "a sensor overlap is not a contact"
        );
        assert_eq!(physics.cached_pairs(), 0, "and never warm starts");
        assert_eq!(translation(&world, ball), before, "nor pushes anything out");
        assert!(physics.touching().contains(trigger, ball));
    }

    #[test]
    fn the_same_overlap_without_the_sensor_is_solved() {
        // The other half of the test above: what changes is the mark, not the
        // scene.
        let (mut physics, mut world, _, ball) = overlapping(false);
        let before = translation(&world, ball);

        for _ in 0..10 {
            physics.step_once(&mut world, Vec2::ZERO);
        }

        assert_eq!(physics.contacts().len(), 1);
        assert!(translation(&world, ball).x > before.x, "it is pushed out");
    }

    #[test]
    fn a_pair_begins_once_and_then_says_nothing() {
        let (mut physics, mut world, trigger, ball) = overlapping(true);

        physics.step_once(&mut world, Vec2::ZERO);
        assert_eq!(
            physics.events(),
            [CollisionEvent {
                a: trigger,
                b: ball,
                touch: Touch::Began,
                sensor: true,
            }]
        );

        physics.step_once(&mut world, Vec2::ZERO);
        assert!(physics.events().is_empty(), "it is still the same overlap");
    }

    #[test]
    fn walking_out_of_a_sensor_ends_the_pair() {
        let (mut physics, mut world, trigger, ball) = overlapping(true);
        physics.step_once(&mut world, Vec2::ZERO);

        world
            .get_mut::<Transform>(ball)
            .expect("the transform is there")
            .translation = Vec2::new(50.0, 0.0);
        physics.step_once(&mut world, Vec2::ZERO);

        assert_eq!(
            physics.events(),
            [CollisionEvent {
                a: trigger,
                b: ball,
                touch: Touch::Ended,
                sensor: true,
            }]
        );
    }

    #[test]
    fn despawning_one_side_ends_the_pair_rather_than_losing_it() {
        // What a pickup does to itself. Silence here would leave a door that
        // opened on Began with nothing to close it.
        let (mut physics, mut world, trigger, ball) = overlapping(true);
        physics.step_once(&mut world, Vec2::ZERO);

        world.despawn(trigger);
        physics.step_once(&mut world, Vec2::ZERO);

        assert_eq!(physics.events().len(), 1);
        assert_eq!(physics.events()[0].touch, Touch::Ended);
        assert_eq!(physics.events()[0].other(ball), Some(trigger));
    }

    #[test]
    fn resetting_ends_nothing() {
        let (mut physics, mut world, _, _) = overlapping(true);
        physics.step_once(&mut world, Vec2::ZERO);
        assert_eq!(physics.touching().len(), 1);

        physics.reset();

        assert!(physics.events().is_empty());
        assert!(physics.touching().is_empty());
        physics.step_once(&mut world, Vec2::ZERO);
        assert_eq!(
            physics.events().len(),
            1,
            "the pair begins again, and never ended"
        );
        assert_eq!(physics.events()[0].touch, Touch::Began);
    }

    #[test]
    fn a_frame_that_owed_no_step_leaves_the_events_alone() {
        // Same rule the contacts have: a fast frame reports the frame rate if
        // it blanks what the last step found.
        let (mut physics, mut world, _, _) = overlapping(true);
        physics.advance(&mut world, Vec2::ZERO, 1.0 / 60.0);
        assert_eq!(physics.events().len(), 1);

        physics.advance(&mut world, Vec2::ZERO, 0.0);

        assert_eq!(physics.events().len(), 1);
    }

    #[test]
    fn a_world_stepping_at_zero_falls_back_to_the_default_rate() {
        let mut physics = PhysicsWorld::with_step(0.0);
        let (mut world, ball) = floor_and_ball();

        physics.advance(&mut world, G, 1.0 / 60.0);

        assert!(height(&world, ball) < 0.0, "it should have run one step");
    }
}
