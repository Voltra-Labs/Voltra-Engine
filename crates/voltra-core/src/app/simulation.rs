//! Whether a frame simulates, and the one-off requests an editor can make.
//!
//! Deliberately not called "play mode". Play, pause and stop are an editor's
//! vocabulary and a shipped game has one mode; all the platform layer ever
//! needs to know is whether this frame steps. Keeping the editor's concept out
//! of here is what stops a game binary carrying it forever.

use crate::app::App;

/// The simulation switch, plus the requests that override it for one frame.
#[derive(Debug, Default)]
pub(crate) struct Simulation {
    /// Whether each frame runs the fixed steps it owes.
    running: bool,
    /// Fixed steps to run on the next frame regardless of `running`.
    ///
    /// Additive: two requests in one frame run two steps, rather than one.
    pending_steps: u32,
    /// Whether to forget the impulses and the clock's debt before stepping.
    pending_reset: bool,
}

impl Simulation {
    pub(crate) fn is_running(&self) -> bool {
        self.running
    }

    pub(crate) fn set_running(&mut self, running: bool) {
        self.running = running;
    }

    pub(crate) fn request_steps(&mut self, count: u32) {
        // Saturating rather than wrapping: a caller that asked for four billion
        // steps has a bug, and wrapping to zero would hide it.
        self.pending_steps = self.pending_steps.saturating_add(count);
    }

    pub(crate) fn request_reset(&mut self) {
        self.pending_reset = true;
    }

    /// The steps owed, consumed. A request is honoured exactly once.
    fn take_steps(&mut self) -> u32 {
        std::mem::take(&mut self.pending_steps)
    }

    /// Whether a reset is owed, consumed.
    fn take_reset(&mut self) -> bool {
        std::mem::take(&mut self.pending_reset)
    }
}

impl App {
    /// Runs the fixed steps this frame owes, plus any asked for outright.
    ///
    /// The contacts kept are the last step's — the state the frame ends in,
    /// which is what the debug overlay should show. Earlier steps' contacts
    /// describe positions nothing was ever drawn at.
    ///
    /// **One frame of latency, by construction.** A UI callback runs *after*
    /// [`App::update`], so a step or a switch asked for from a panel first
    /// takes effect on the next frame. At 60 Hz nobody can see it, and the
    /// alternative — the UI reaching back into the frame that already ran — is
    /// worse. Do not "fix" it.
    pub(super) fn step_physics(&mut self, delta: f32) {
        // Before any step: a restored scene must never have the previous
        // session's impulses warm-starting its first contact, nor its debt.
        if self.simulation.take_reset() {
            self.physics_world.reset();
        }

        // Requested steps run unconditionally: a Step press while paused must
        // advance the world by exactly one step, and the clock would owe zero.
        // They are counted in with the owed ones rather than looped separately
        // so there is one place a step runs from, and so the game's fixed tick
        // cannot be forgotten on one of two paths.
        let requested = self.simulation.take_steps();
        let owed = if self.simulation.is_running() {
            // Only while running: a stopped world must not bank the frames it
            // spent stopped and run them all at once when Play is pressed.
            self.physics_world.owed_steps(delta)
        } else {
            0
        };

        let step = self.physics_world.step();
        for _ in 0..requested.saturating_add(owed) {
            // Before the step, not after: a force the tick applies is meant to
            // act during the step that follows it, which is the whole reason
            // the hook is per step rather than per frame.
            self.run_fixed_update(step);
            self.physics_world.step_once(&mut self.world, self.gravity);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use voltra_ecs::Entity;
    use voltra_render::glam::Vec2;
    use voltra_scene::{Collider, RigidBody, Transform};

    const G: Vec2 = Vec2::new(0.0, -10.0);
    const FRAME: f32 = 1.0 / 60.0;

    /// An app holding one dynamic body over a static floor, with no window.
    ///
    /// `App::default` never touches the event loop, so everything the switch
    /// decides is reachable without a device or a surface.
    fn app_with_a_ball() -> (App, Entity) {
        let mut app = App {
            gravity: G,
            ..Default::default()
        };

        let floor = app.world.spawn();
        app.world
            .insert(floor, Transform::from_translation(Vec2::new(0.0, -2.0)));
        app.world.insert(
            floor,
            Collider::Box {
                half_extents: Vec2::new(10.0, 0.5),
            },
        );
        app.world.insert(floor, RigidBody::new_static());

        let ball = app.world.spawn();
        app.world.insert(ball, Transform::default());
        app.world.insert(ball, Collider::Circle { radius: 0.5 });
        app.world.insert(ball, RigidBody::new_dynamic(1.0));

        (app, ball)
    }

    fn height(app: &App, entity: Entity) -> f32 {
        app.world
            .get::<Transform>(entity)
            .expect("the transform is there")
            .translation
            .y
    }

    #[test]
    fn a_frame_does_not_step_while_simulation_is_off() {
        // The whole point of the switch: an editor in `Editing` must be able to
        // place a body without it falling out from under the cursor.
        let (mut app, ball) = app_with_a_ball();

        app.step_physics(FRAME);

        assert_eq!(height(&app, ball), 0.0);
    }

    #[test]
    fn a_requested_step_runs_while_simulation_is_off() {
        let (mut app, ball) = app_with_a_ball();

        app.request_steps(1);
        app.step_physics(0.0);

        assert!(height(&app, ball) < 0.0, "the Step button must advance it");
    }

    #[test]
    fn two_requested_steps_run_two_steps() {
        // Additive, so two presses in one frame are two steps rather than one.
        let (mut app_one, ball_one) = app_with_a_ball();
        app_one.request_steps(1);
        app_one.step_physics(0.0);

        let (mut app_two, ball_two) = app_with_a_ball();
        app_two.request_steps(1);
        app_two.request_steps(1);
        app_two.step_physics(0.0);

        assert!(
            height(&app_two, ball_two) < height(&app_one, ball_one),
            "two steps must fall further than one"
        );
    }

    #[test]
    fn requested_steps_are_consumed_once() {
        let (mut app, ball) = app_with_a_ball();
        app.request_steps(1);
        app.step_physics(0.0);
        let after_one = height(&app, ball);

        app.step_physics(0.0);

        assert_eq!(
            height(&app, ball),
            after_one,
            "a request must not run again on the next frame"
        );
    }

    #[test]
    fn a_requested_reset_forgets_the_contact_impulses() {
        // What Stop needs: the restored scene must not be warm-started with the
        // played scene's forces.
        let (mut app, _) = app_with_a_ball();
        app.set_simulating(true);
        for _ in 0..300 {
            app.step_physics(FRAME);
        }
        assert!(app.physics_mut().cached_pairs() > 0, "it should be resting");

        app.set_simulating(false);
        app.simulation.request_reset();
        app.step_physics(FRAME);

        assert_eq!(app.physics_mut().cached_pairs(), 0);
    }

    #[test]
    fn with_simulation_starts_the_switch_on() {
        // It keeps its name and becomes the *initial* value: a game says it
        // once, the editor says nothing and starts in `Editing`.
        assert!(!App::default().is_simulating());
        assert!(App::default().with_simulation().is_simulating());
    }
}
