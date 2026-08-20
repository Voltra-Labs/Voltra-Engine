//! What a game is handed when the loop gives it a turn.
//!
//! The editor gets [`UiFrame`](super::UiFrame); a game gets this. Both exist
//! for the same reason: the callback needs several of `App`'s fields at once,
//! and handing out `&mut App` would make every field of the platform layer
//! part of the engine's public surface.
//!
//! There is one context type for both hooks. Unity's `Update` and
//! `FixedUpdate` take no arguments and read `Time.deltaTime` or
//! `Time.fixedDeltaTime`; Godot's `_process(delta)` and
//! `_physics_process(delta)` take the one that applies. The second shape is
//! the better one — a tick that is told its own step cannot read the wrong
//! clock — and once `delta` is a field, the two hooks want exactly the same
//! things and a second type would differ only in its name.

use voltra_ecs::World;
use voltra_physics::{CollisionEvent, Contact};

use crate::app::App;
use crate::input::Input;

/// A game's turn at the world.
///
/// Fields rather than accessors: this is a bag of borrows with no invariant to
/// protect, and a game reaching for `tick.world` should not have to go through
/// a method that does nothing.
pub struct Tick<'a> {
    /// The scene, to read and to change. Spawning and despawning are both fine
    /// here; nothing downstream in the frame holds an entity across the call.
    pub world: &'a mut World,
    /// Keyboard and mouse for this frame.
    ///
    /// Edge-triggered reads — [`Input::was_key_pressed`] and its relatives —
    /// are per *frame*, not per fixed step. A fixed tick that runs twice in
    /// one frame therefore sees the same press twice, and one that runs zero
    /// times misses it. Unity's manual says the same thing about reading input
    /// in `FixedUpdate`: hold-state belongs in the fixed tick, edges belong in
    /// the per-frame one.
    pub input: &'a Input,
    /// Seconds this tick covers: the frame's delta in the per-frame hook, the
    /// fixed step in the fixed one.
    pub delta: f32,
    /// What the last physics step found overlapping.
    ///
    /// State, not a stream: the same resting contact is here on every step it
    /// lasts. Empty until something steps, and sensors are never in it.
    pub contacts: &'a [Contact],
    /// What began and ended touching, sensors included.
    ///
    /// A stream, unlike [`Tick::contacts`]: each hook is handed every event
    /// exactly once. The fixed hook gets the step it is about to follow's
    /// predecessor; the per-frame hook gets everything since its last turn,
    /// however many steps that was. Neither can miss a pickup, and neither can
    /// take it twice.
    ///
    /// `a` is always the lower entity of the pair, so
    /// [`CollisionEvent::other`] is how a game asks what *it* touched.
    pub events: &'a [CollisionEvent],
}

/// A hook the game installs on the loop.
pub(super) type TickFn = Box<dyn FnMut(&mut Tick<'_>)>;

impl App {
    /// Runs the per-frame tick, if there is one and the world is live.
    ///
    /// Gated on the simulation switch for the same reason physics is: while an
    /// editor is authoring, a game's logic moving the entity being placed is
    /// the scene editing itself. Unity draws the line in the same place —
    /// scripts run in play mode, and `[ExecuteInEditMode]` is the opt-in
    /// exception rather than the rule.
    pub(super) fn run_update(&mut self, delta: f32) {
        if !self.simulation.is_running() {
            return;
        }
        let Some(update) = self.update.as_mut() else {
            return;
        };

        let mut tick = Tick {
            world: &mut self.world,
            input: &self.input,
            delta,
            contacts: self.physics_world.contacts(),
            events: &self.pending_events,
        };
        update(&mut tick);
    }

    /// Runs the fixed tick, if there is one. Called before each step.
    ///
    /// Not gated on the switch: the caller is the step loop, and a step that
    /// runs — including the single one an editor's Step button asks for while
    /// paused — is a step the game's logic belongs in front of. "Advance the
    /// world by one step" has to mean the whole world.
    pub(super) fn run_fixed_update(&mut self, delta: f32) {
        let Some(fixed) = self.fixed_update.as_mut() else {
            return;
        };

        let mut tick = Tick {
            world: &mut self.world,
            input: &self.input,
            delta,
            contacts: self.physics_world.contacts(),
            // The step before this one's: the step this tick precedes has not
            // run, so there is nothing newer to hand it.
            events: self.physics_world.events(),
        };
        fixed(&mut tick);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::cell::RefCell;
    use std::rc::Rc;
    use voltra_ecs::Entity;
    use voltra_render::glam::Vec2;
    use voltra_scene::{Collider, RigidBody, Sensor, Transform};

    const FRAME: f32 = 1.0 / 60.0;
    const G: Vec2 = Vec2::new(0.0, -10.0);

    /// A counter a hook can bump and a test can read afterwards.
    fn counter() -> (Rc<Cell<u32>>, Rc<Cell<u32>>) {
        let count = Rc::new(Cell::new(0));
        (count.clone(), count)
    }

    fn app_with_a_falling_body() -> (App, Entity) {
        let mut app = App {
            gravity: G,
            ..Default::default()
        };
        let body = app.world.spawn();
        app.world.insert(body, Transform::default());
        app.world.insert(body, Collider::Circle { radius: 0.5 });
        app.world.insert(body, RigidBody::new_dynamic(1.0));
        (app, body)
    }

    fn height(app: &App, entity: Entity) -> f32 {
        app.world
            .get::<Transform>(entity)
            .expect("the transform is there")
            .translation
            .y
    }

    #[test]
    fn the_per_frame_tick_runs_once_a_frame_while_the_world_is_live() {
        let (count, seen) = counter();
        let mut app = App::default()
            .with_simulation()
            .with_update(move |_| count.set(count.get() + 1));

        app.advance(FRAME);
        app.advance(FRAME);

        assert_eq!(seen.get(), 2);
    }

    #[test]
    fn the_per_frame_tick_does_not_run_while_the_world_is_stopped() {
        // The editor while authoring. Game logic that ran here would edit the
        // scene behind the author's back.
        let (count, seen) = counter();
        let mut app = App::default().with_update(move |_| count.set(count.get() + 1));

        app.advance(FRAME);

        assert_eq!(seen.get(), 0);
    }

    #[test]
    fn the_per_frame_tick_is_told_the_frame_delta() {
        let delta = Rc::new(Cell::new(0.0));
        let seen = delta.clone();
        let mut app = App::default()
            .with_simulation()
            .with_update(move |tick| delta.set(tick.delta));

        app.advance(0.25);

        assert_eq!(seen.get(), 0.25);
    }

    #[test]
    fn the_tick_reads_this_frame_s_input() {
        let pressed = Rc::new(Cell::new(false));
        let seen = pressed.clone();
        let mut app = App::default().with_simulation().with_update(move |tick| {
            pressed.set(tick.input.was_key_pressed(crate::KeyCode::Space));
        });

        app.input.on_key(crate::KeyCode::Space, true, false);
        app.advance(FRAME);

        assert!(seen.get(), "the edge must reach the game");
    }

    #[test]
    fn the_fixed_tick_runs_once_per_step_owed() {
        let (count, seen) = counter();
        let mut app = App::default()
            .with_simulation()
            .with_fixed_update(move |_| count.set(count.get() + 1));

        app.advance(FRAME * 3.0);

        assert_eq!(seen.get(), 3);
    }

    #[test]
    fn the_fixed_tick_is_told_the_fixed_step_not_the_frame() {
        let delta = Rc::new(Cell::new(0.0));
        let seen = delta.clone();
        let mut app = App::default()
            .with_simulation()
            .with_fixed_update(move |tick| delta.set(tick.delta));

        app.advance(FRAME * 2.5);

        assert!(
            (seen.get() - FRAME).abs() < 1e-6,
            "expected the fixed step, got {}",
            seen.get()
        );
    }

    #[test]
    fn a_frame_that_owes_no_step_runs_no_fixed_tick() {
        let (count, seen) = counter();
        let mut app = App::default()
            .with_simulation()
            .with_fixed_update(move |_| count.set(count.get() + 1));

        app.advance(FRAME * 0.5);

        assert_eq!(seen.get(), 0, "half a step owes nothing yet");
    }

    #[test]
    fn a_requested_step_runs_the_fixed_tick_even_while_stopped() {
        // The editor's Step button: one step of the whole world, logic and all.
        let (count, seen) = counter();
        let mut app = App::default().with_fixed_update(move |_| count.set(count.get() + 1));

        app.request_steps(1);
        app.advance(0.0);

        assert_eq!(seen.get(), 1);
    }

    #[test]
    fn what_the_tick_writes_is_simulated_in_the_same_frame() {
        // The reason the tick runs before the steps rather than after: a
        // velocity set from input must not wait a step to be integrated.
        let (app, body) = app_with_a_falling_body();
        let mut app = app.with_simulation().with_update(move |tick| {
            if let Some(rigid) = tick.world.get_mut::<RigidBody>(body) {
                rigid.velocity = Vec2::new(0.0, 5.0);
            }
        });

        app.advance(FRAME);

        assert!(
            height(&app, body) > 0.0,
            "the body should have risen this frame, not next"
        );
    }

    /// A static sensor box at the origin with a dynamic ball sitting inside
    /// it, and no gravity to move either of them.
    fn app_with_a_trigger() -> (App, Entity, Entity) {
        let mut app = App::default();

        let trigger = app.world.spawn();
        app.world.insert(trigger, Transform::default());
        app.world.insert(
            trigger,
            Collider::Box {
                half_extents: Vec2::splat(1.0),
            },
        );
        app.world.insert(trigger, Sensor);

        let ball = app.world.spawn();
        app.world.insert(ball, Transform::default());
        app.world.insert(ball, Collider::Circle { radius: 0.5 });
        app.world.insert(ball, RigidBody::new_dynamic(1.0));

        (app, trigger, ball)
    }

    /// How many events each turn of a hook was handed, in order.
    type Log = Rc<RefCell<Vec<usize>>>;

    fn log() -> (Log, Log) {
        let seen = Rc::new(RefCell::new(Vec::new()));
        (seen.clone(), seen)
    }

    #[test]
    fn the_per_frame_tick_is_handed_each_event_exactly_once() {
        let (app, trigger, ball) = app_with_a_trigger();
        let (record, seen) = log();
        let touched = Rc::new(Cell::new(None));
        let other = touched.clone();
        let mut app = app.with_simulation().with_update(move |tick| {
            record.borrow_mut().push(tick.events.len());
            if let Some(event) = tick.events.first() {
                other.set(event.other(ball));
            }
        });

        app.advance(FRAME);
        app.advance(FRAME);
        app.advance(FRAME * 0.1);

        assert_eq!(
            *seen.borrow(),
            vec![0, 1, 0],
            "nothing has stepped, then the overlap begins, then it is spent"
        );
        assert_eq!(touched.get(), Some(trigger));
    }

    #[test]
    fn the_per_frame_tick_sees_every_step_of_the_frame_not_only_the_last() {
        // A pickup taken on the first of two steps in one frame must not be
        // lost because the second step replaced what physics remembers.
        let (mut app, _, ball) = app_with_a_trigger();
        app.world
            .get_mut::<RigidBody>(ball)
            .expect("it is a body")
            .velocity = Vec2::new(0.0, -100.0);

        let (record, seen) = log();
        let mut app = app
            .with_simulation()
            .with_update(move |tick| record.borrow_mut().push(tick.events.len()));

        app.advance(FRAME * 2.0);
        app.advance(FRAME * 0.1);

        assert_eq!(
            *seen.borrow(),
            vec![0, 2],
            "it began inside and ended outside, both in one frame"
        );
    }

    #[test]
    fn the_fixed_tick_sees_the_events_of_the_step_before_it() {
        let (app, _, _) = app_with_a_trigger();
        let (record, seen) = log();
        let mut app = app
            .with_simulation()
            .with_fixed_update(move |tick| record.borrow_mut().push(tick.events.len()));

        app.advance(FRAME);
        app.advance(FRAME);

        assert_eq!(
            *seen.borrow(),
            vec![0, 1],
            "the first step has nothing behind it; the second follows the overlap"
        );
    }

    #[test]
    fn the_tick_sees_the_contacts_of_the_last_step() {
        // How a game asks whether it is standing on something.
        let (mut app, _) = app_with_a_falling_body();
        let floor = app.world.spawn();
        app.world
            .insert(floor, Transform::from_translation(Vec2::new(0.0, -1.0)));
        app.world.insert(
            floor,
            Collider::Box {
                half_extents: Vec2::new(10.0, 0.5),
            },
        );
        app.world.insert(floor, RigidBody::new_static());

        let (count, seen) = counter();
        let mut app = app.with_simulation().with_update(move |tick| {
            if !tick.contacts.is_empty() {
                count.set(count.get() + 1);
            }
        });

        for _ in 0..120 {
            app.advance(FRAME);
        }

        assert!(seen.get() > 0, "the body lands and the tick should see it");
    }
}
