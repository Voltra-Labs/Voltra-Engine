//! The editor's shared state and its frame layout.

use voltra_core::egui::Ui;
use voltra_core::UiFrame;
use voltra_ecs::Entity;
use voltra_scene::ComponentRegistry;

use crate::camera::ViewportCamera;
use crate::gizmo::Gizmo;
use crate::panels;
use crate::panels::assets::AssetBrowser;
use crate::play::{Play, PlayContext, PlayState};
use crate::tool::Tool;
use crate::undo::{selected_id, shortcut, History, SceneView, UndoContext};
use crate::view::View;

/// Editor state that outlives a frame.
///
/// egui is immediate mode and remembers nothing between frames, so anything the
/// editor still needs next frame — the selection — has to be kept here.
#[derive(Default)]
pub struct Editor {
    pub(crate) selected: Option<Entity>,
    pub(crate) camera: ViewportCamera,
    /// Which transform a viewport drag performs. Persistent, Unity-style.
    pub(crate) tool: Tool,
    pub(crate) gizmo: Gizmo,
    /// Whether to outline every collider and draw every contact.
    ///
    /// Off by default: a scene full of green outlines is noise while sprites
    /// are being placed, and the overlay is a debugging aid rather than part
    /// of the picture being authored. Unity, Unreal and Godot all hide their
    /// collision shapes behind a toggle for the same reason.
    pub(crate) show_colliders: bool,
    /// Whether the editor is authoring the scene or simulating it, and the
    /// snapshot Stop will put back.
    pub(crate) play: Play,
    /// The one registry every save, load, snapshot and restore in this editor
    /// goes through.
    ///
    /// Owned rather than built per call: a snapshot and the restore that undoes
    /// it must agree on which component types exist, and sharing the field is
    /// how that is guaranteed rather than remembered. `ComponentRegistry`'s own
    /// `Default` is `with_defaults`, not an empty registry, so the derive on
    /// `Editor` cannot quietly produce one that persists nothing.
    pub(crate) registry: ComponentRegistry,
    /// Every scene edit that can be undone, and every one that has been.
    pub(crate) history: History,
    /// Where the asset browser is looking, and what it last read there.
    pub(crate) assets: AssetBrowser,
    /// Whether the viewport shows the editor's camera or the scene's, and the
    /// editor camera parked while the scene's has it.
    pub(crate) view: View,
}

impl Editor {
    /// Lays out the whole editor. Called once per frame with the root `Ui`.
    pub fn ui(&mut self, ui: &mut Ui, frame: &mut UiFrame<'_>) {
        // Only while editing: play steps the world every frame, and none of
        // that is an edit.
        let editing = self.play.state() == PlayState::Editing;

        if editing {
            // The scene as this frame found it, before any panel can write to
            // it. The watched set is the selection, which is the only entity
            // the gizmo or the inspector can reach; everything else — spawn,
            // delete, clear — records itself explicitly.
            let watched = selected_id(frame.world, self.selected);
            let view = SceneView {
                world: frame.world,
                registry: &self.registry,
                selected: watched,
            };
            self.history.watch(view, watched);

            // After the watch, so an undo taken this frame is the change the
            // watcher sees rather than one it records a second time.
            match shortcut::poll(ui.ctx()) {
                Some(shortcut::UndoAction::Undo) => self.undo(frame),
                Some(shortcut::UndoAction::Redo) => self.redo(frame),
                None => {}
            }
        }

        panels::menu_bar::show(self, ui, frame);
        // Between the menu bar and everything else, which is where Unity and
        // Unreal both put the transport. The state of the window is the most
        // important thing on screen while it is not `Editing`, and a menu that
        // has to be opened to see it is not a state indicator.
        panels::toolbar::show(self, ui, frame);
        panels::hierarchy::show(self, ui, frame);
        panels::inspector::show(self, ui, frame);
        // After the two side panels, so it is docked between them rather than
        // across the whole window: its drop targets are the viewport above and
        // the inspector beside it, and a browser far from both is a long drag.
        panels::assets::show(self, ui, frame);
        // Last, so it takes whatever room the docked panels left rather than
        // the other way round.
        panels::viewport::show(self, ui, frame);

        // Last, so it sees everything every panel did.
        if editing {
            let view = SceneView {
                world: frame.world,
                registry: &self.registry,
                selected: selected_id(frame.world, self.selected),
            };
            self.history.end_frame(view);
        }
    }

    /// Puts the last edit back. A no-op outside [`PlayState::Editing`].
    ///
    /// Gated rather than allowed and undone on Stop: while playing, the world
    /// is the simulation's, and the snapshot Stop restores is the authored
    /// scene the history's entries are addressed against.
    pub(crate) fn undo(&mut self, frame: &mut UiFrame<'_>) {
        if self.play.state() != PlayState::Editing {
            return;
        }
        let ctx = UndoContext {
            registry: &self.registry,
            selected: &mut self.selected,
            gizmo: &mut self.gizmo,
        };
        if !self.history.undo(ctx, frame) {
            log::info!("nothing to undo");
        }
    }

    /// Replays the last undone edit. A no-op outside [`PlayState::Editing`].
    pub(crate) fn redo(&mut self, frame: &mut UiFrame<'_>) {
        if self.play.state() != PlayState::Editing {
            return;
        }
        let ctx = UndoContext {
            registry: &self.registry,
            selected: &mut self.selected,
            gizmo: &mut self.gizmo,
        };
        if !self.history.redo(ctx, frame) {
            log::info!("nothing to redo");
        }
    }

    /// Enters play, snapshotting the scene the first time.
    pub(crate) fn start_play(&mut self, frame: &mut UiFrame<'_>) {
        self.play.play(
            PlayContext {
                registry: &self.registry,
                selected: &mut self.selected,
                gizmo: &mut self.gizmo,
            },
            frame,
        );
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
        self.play.stop(
            PlayContext {
                registry: &self.registry,
                selected: &mut self.selected,
                gizmo: &mut self.gizmo,
            },
            frame,
        );
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_history_starts_empty() {
        let editor = Editor::default();
        assert!(editor.history.is_empty());
        assert_eq!(editor.history.undo_label(), None);
    }
}
