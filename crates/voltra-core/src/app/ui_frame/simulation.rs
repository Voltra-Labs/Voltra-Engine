//! What a panel steers of the simulation: the run switch, the game's code,
//! single steps and a reset.

use super::UiFrame;

impl UiFrame<'_> {
    /// Whether each frame runs the physics steps it owes.
    pub fn simulating(&self) -> bool {
        self.simulation.is_running()
    }

    /// Turns per-frame stepping on or off, from the next frame onwards.
    ///
    /// One frame of latency by construction: `App::update` runs before this
    /// callback, so a switch thrown here first applies on the next frame. At
    /// 60 Hz nobody can see it, and the alternative — the UI reaching back into
    /// the frame that already ran — is worse.
    pub fn set_simulating(&mut self, simulating: bool) {
        self.simulation.set_running(simulating);
    }

    /// Runs `game` from the next turn on, in place of whatever ran before,
    /// which is dropped here; `None` runs none. The game starts afresh: its
    /// [`start`](crate::Game::start) comes before its first turn.
    ///
    /// Registers nothing: a game loaded at run time has its components named
    /// in [`UiFrame::registry`] before any scene holding them is read, which
    /// is earlier than the play that runs it.
    pub fn set_game(&mut self, game: Option<Box<dyn crate::Game>>) {
        *self.game = game.map(crate::game::Running::new);
    }

    /// Whether a game is running its turns.
    pub fn has_game(&self) -> bool {
        self.game.is_some()
    }

    /// Runs `count` fixed steps on the next frame regardless of the switch.
    ///
    /// Additive, so two requests in one frame run two steps.
    pub fn request_steps(&mut self, count: u32) {
        self.simulation.request_steps(count);
    }

    /// Forgets the accumulated impulses and the clock's banked time, before
    /// the next frame steps anything.
    ///
    /// For a world that is no longer the same world: an editor's Stop, which
    /// despawns and respawns every entity the scene holds.
    pub fn reset_physics(&mut self) {
        self.simulation.request_reset();
    }
}
