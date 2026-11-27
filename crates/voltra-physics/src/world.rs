//! What a simulated world owns between frames.

use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;

use crate::clock::PhysicsClock;
use crate::events::{CollisionEvent, JointBroken, Touching};
use crate::narrow::Contact;
use crate::one_way::OneWayPasses;
use crate::sleep::Sleep;
use crate::solver::{ImpulseCache, SolverParams};
use crate::step::step_sleeping;

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
    broken: Vec<JointBroken>,
    sleep: Sleep,
    passes: OneWayPasses,
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

    /// The solver's parameters.
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

    /// The joints the last step that ran broke, already removed from the
    /// world. A stream like [`PhysicsWorld::events`], read the same way:
    /// from a game's fixed tick, or gathered across a frame's steps.
    pub fn broken_joints(&self) -> &[JointBroken] {
        &self.broken
    }

    /// Whether `entity` is asleep: at rest, and left out of the solve until
    /// something disturbs it.
    pub fn is_asleep(&self, entity: Entity) -> bool {
        self.sleep.is_asleep(entity)
    }

    /// How many bodies are asleep.
    pub fn asleep_count(&self) -> usize {
        self.sleep.asleep_count()
    }

    /// Wakes `entity`, and its island with it on the next step. Setting a
    /// velocity or moving its transform does the same on its own; this is
    /// for a game that wants a body to notice something it did not touch.
    pub fn wake(&mut self, entity: Entity) {
        self.sleep.wake(entity);
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
        self.broken.clear();
        self.sleep.clear();
        self.passes.clear();
        self.clock.reset();
    }

    /// Runs exactly one fixed step, whatever the clock has banked.
    ///
    /// What an editor's Step button asks for: a paused world must advance by
    /// one step and no more, and [`PhysicsWorld::advance`] would run zero.
    pub fn step_once(&mut self, world: &mut World, gravity: Vec2) -> &[Contact] {
        let dt = self.clock.step();
        let stepped = step_sleeping(
            world,
            &mut self.impulses,
            &self.params,
            gravity,
            dt,
            &mut self.sleep,
            &mut self.passes,
        );
        self.events = self.touching.update(stepped.overlaps.pairs());
        self.contacts = stepped.overlaps.contacts;
        self.broken = stepped.broken;
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
mod tests;
