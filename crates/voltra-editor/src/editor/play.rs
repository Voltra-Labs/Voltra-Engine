//! Entering, pausing, stepping and leaving play: the transport's actions.

use voltra_core::UiFrame;

use crate::editor::Editor;
use crate::play::{PlayContext, PlayState};

impl Editor {
    /// Enters play, snapshotting the scene the first time — and, the first
    /// time, starting a fresh game from the loaded library, whose turns run
    /// until Stop. A paused session resumes the game it had.
    pub(crate) fn start_play(&mut self, frame: &mut UiFrame<'_>) {
        let from_editing = self.play.state() == PlayState::Editing;
        self.play.play(
            PlayContext {
                registry: &self.registry,
                selected: &mut self.selection,
                gizmo: &mut self.gizmo,
            },
            frame,
        );
        if from_editing && self.play.state() == PlayState::Playing {
            frame.set_game(self.game.new_game());
            crate::netplay::begin(self, frame);
        }
    }

    /// Stops stepping and leaves the scene where it is.
    pub(crate) fn pause_play(&mut self, frame: &mut UiFrame<'_>) {
        self.play.pause(frame);
    }

    /// Runs exactly one fixed step, while paused.
    pub(crate) fn step_play(&mut self, frame: &mut UiFrame<'_>) {
        self.play.step(frame);
    }

    /// Leaves play and puts the snapshot back. Returns whether it had to.
    ///
    /// `Scene ▸ Open`, `Scene ▸ Clear` and `Scene ▸ Save` all call this first.
    /// Each of them means something incoherent mid-play — Open discards the
    /// world the snapshot belongs to, Save writes a mid-flight scene as if it
    /// were the authored one — and stopping first is both Unity's answer and
    /// cheaper than three special cases.
    pub(crate) fn stop_play(&mut self, frame: &mut UiFrame<'_>) -> bool {
        if self.play.state() == PlayState::Editing {
            return false;
        }
        // The clients and the game first: they belong to the session the
        // restore discards.
        crate::netplay::end(self, frame);
        frame.set_game(None);
        self.play.stop(
            PlayContext {
                registry: &self.registry,
                selected: &mut self.selection,
                gizmo: &mut self.gizmo,
            },
            frame,
        );
        true
    }
}
