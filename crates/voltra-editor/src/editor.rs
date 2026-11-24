//! The editor's shared state and its frame layout.

use voltra_core::egui::Ui;
use voltra_core::{UiFrame, VoiceId};
use voltra_scene::ComponentRegistry;

use crate::camera::ViewportCamera;
use crate::document::SceneDocument;
use crate::gizmo::Gizmo;
use crate::marquee::Marquee;
use crate::paint::Paint;
use crate::panels;
use crate::panels::assets::AssetBrowser;
use crate::panels::profiler::ProfilerPanel;
use crate::play::{Play, PlayContext, PlayState};
use crate::scene_dialogs::SceneDialogs;
use crate::selection::Selection;
use crate::shape_edit::ShapeEdit;
use crate::timeline::Timeline;
use crate::tool::Tool;
use crate::undo::{shortcut, History, SceneView, UndoContext};
use crate::view::View;

/// Editor state that outlives a frame.
///
/// egui is immediate mode and remembers nothing between frames, so anything the
/// editor still needs next frame — the selection — has to be kept here.
#[derive(Default)]
pub struct Editor {
    /// What is selected, the primary last. See [`Selection`].
    pub(crate) selection: Selection,
    pub(crate) camera: ViewportCamera,
    /// Which transform a viewport drag performs. Persistent, Unity-style.
    pub(crate) tool: Tool,
    pub(crate) gizmo: Gizmo,
    /// The box being drawn over the viewport to select, if any.
    pub(crate) marquee: Marquee,
    /// The armed paint tool and its brush. While armed with a layer selected,
    /// it has the viewport instead of the gizmo.
    pub(crate) paint: Paint,
    /// The shadow caster whose polygon the viewport is editing, if any.
    /// While on, it has the viewport instead of the gizmo, as painting does.
    pub(crate) shape: ShapeEdit,
    /// Whether to outline every collider and draw every contact.
    ///
    /// Off by default: a scene full of green outlines is noise while sprites
    /// are being placed, and the overlay is a debugging aid rather than part
    /// of the picture being authored. Unity, Unreal and Godot all hide their
    /// collision shapes behind a toggle for the same reason.
    pub(crate) show_colliders: bool,
    /// A streamed world's region bounds over the scene, when it is part of
    /// one: Scene ▸ Show region bounds.
    pub(crate) regions: crate::view::regions::RegionOverlay,
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
    /// The sound the inspector's Play button started, so the next press or
    /// Stop can end it.
    pub(crate) audition: Option<VoiceId>,
    /// The timeline's playhead, selected key and preview pose.
    pub(crate) timeline: Timeline,
    /// The bridge a tool drives the editor through, when started with
    /// `--bridge`; see [`crate::bridge`].
    pub(crate) bridge: Option<voltra_bridge::BridgeServer>,
    /// Which file the scene came from, and whether it changed since.
    pub(crate) document: SceneDocument,
    /// The scene windows — "save changes?", Open, Save As — and a scene
    /// the command line asked for, opened on the first frame.
    pub(crate) dialogs: SceneDialogs,
    /// Whether the Profiler window is up.
    pub(crate) profiler: ProfilerPanel,
    /// The grid, and what snaps to it.
    pub(crate) snap: crate::snap::Snap,
    /// The log, kept for the Console window and the bridge.
    pub(crate) console: crate::console::Console,
    /// The Project Settings window.
    pub(crate) settings: panels::project_settings::ProjectSettingsWindow,
    /// The Tile Palette window, which picks what the paint tools paint.
    pub(crate) tile_palette: panels::tile_palette::TilePaletteWindow,
    /// The Atlas Editor window and the atlas it has open.
    pub(crate) atlas_editor: panels::atlas_editor::AtlasEditorWindow,
    /// The project's game, loaded from its library; see [`crate::game`].
    pub(crate) game: crate::game::GameHost,
    /// The scenes set aside while a prefab is edited on its own; see
    /// [`crate::prefab_mode`].
    pub(crate) prefab_mode: crate::prefab_mode::PrefabMode,
    /// What the hierarchy's search box holds; see [`crate::search`].
    pub(crate) hierarchy_search: String,
    pub(crate) netplay: crate::netplay::NetPlay,
    /// The selected emitter, played while authoring.
    pub(crate) particle_preview: crate::particle_preview::ParticlePreview,
    /// The skinned sprite whose weights the viewport is painting, if any.
    pub(crate) weight_paint: crate::weight_paint::WeightPaint,
}

impl Editor {
    /// Lays out the whole editor. Called once per frame with the root `Ui`.
    pub fn ui(&mut self, ui: &mut Ui, frame: &mut UiFrame<'_>) {
        // First of all, once: the game names its components before any scene
        // that holds them is read.
        crate::game::load_at_start(self, frame);
        // Then whatever a change to its sources rebuilt, swapped in before
        // anything reads the world this frame.
        crate::game::reload::poll(self, frame);
        // Before anything reads the world: the preview's pose was for the
        // frame just drawn, and every panel and the history see only what
        // was authored.
        self.timeline.lift(frame.world);
        // Only while editing: play steps the world every frame, and none of
        // that is an edit.
        let editing = self.play.state() == PlayState::Editing;

        if editing {
            // The scene as this frame found it, before any panel can write to
            // it. The watched set is the selection, which is all the gizmo or
            // the inspector can reach; everything else — spawn, delete,
            // clear — records itself explicitly.
            self.selection.retain_alive(frame.world);
            let watched = self.selection.ids(frame.world);
            let view = SceneView {
                world: frame.world,
                registry: &self.registry,
                selected: watched.clone(),
            };
            self.history.watch(view, watched);

            // After the watch, so an undo taken this frame is the change the
            // watcher sees rather than one it records a second time.
            match shortcut::poll(ui.ctx()) {
                // Over the Atlas Editor, the keys are its own history's.
                Some(action) if self.atlas_editor.has_pointer(ui.ctx()) => {
                    self.atlas_editor.shortcut(action);
                }
                Some(shortcut::UndoAction::Undo) => self.undo(frame),
                Some(shortcut::UndoAction::Redo) => self.redo(frame),
                None => {}
            }
            // Delete, duplicate and the clipboard, after the undo keys and
            // before any panel, which then draws what they made.
            crate::edit_keys::poll(self, ui.ctx(), frame);
        }

        // After the watch and the keys, before any panel: what a tool asked
        // for is in place before the frame draws.
        // The clients follow the server's frame, before the bridge reads them.
        crate::netplay::step(self, frame);
        crate::bridge::serve(self, frame);
        // After the edits of the frame so far, so the title's `*` and a
        // close request see them; its windows are modal, over everything.
        crate::scene_dialogs::show(self, ui, frame);

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
        // Over the browser, under the viewport it previews into.
        panels::timeline::show(self, ui, frame);
        // Last, so it takes whatever room the docked panels left rather than
        // the other way round.
        panels::viewport::show(self, ui, frame);
        // Floating windows, over the docked panels.
        panels::profiler::show(self, ui, frame);
        panels::network::show(self, ui, frame);
        panels::console::show(self, ui);
        let assets = frame.textures.root().to_path_buf();
        panels::project_settings::show(self, ui, &assets);
        panels::atlas_editor::show(self, ui, frame);
        panels::tile_palette::show(self, ui, frame);

        // Last, so it sees everything every panel did.
        if editing {
            // Before the history captures the frame's after side, so an edit
            // and the override it makes are one entry, undone together.
            crate::instance::refresh_overrides(self, frame);
            let view = SceneView {
                world: frame.world,
                registry: &self.registry,
                selected: self.selection.ids(frame.world),
            };
            self.history.end_frame(view);
            // After the history has its after side, for the frame about to
            // be drawn; lifted again first thing next frame.
            let posed = self.selection.live_primary(frame.world);
            self.timeline
                .pose(frame.world, frame.animation_clips, posed);
            // Live particles only, which no record holds: after the history
            // so it never sees them, and ahead of the draw that shows them.
            let dt = ui.ctx().input(|i| i.stable_dt);
            let gpu = frame.gpu_particles();
            self.particle_preview.update(frame.world, posed, dt, gpu);
        } else {
            self.particle_preview.stop(frame.world);
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
            selected: &mut self.selection,
            gizmo: &mut self.gizmo,
        };
        if !self.history.undo(ctx, frame) {
            log::info!("nothing to undo");
        }
        // A shape drag held through the undo would write the pointer
        // straight back over what it restored.
        self.shape.cancel_drag();
    }

    /// Replays the last undone edit. A no-op outside [`PlayState::Editing`].
    pub(crate) fn redo(&mut self, frame: &mut UiFrame<'_>) {
        if self.play.state() != PlayState::Editing {
            return;
        }
        let ctx = UndoContext {
            registry: &self.registry,
            selected: &mut self.selection,
            gizmo: &mut self.gizmo,
        };
        if !self.history.redo(ctx, frame) {
            log::info!("nothing to redo");
        }
        // A shape drag held through the redo would write the pointer
        // straight back over what it restored.
        self.shape.cancel_drag();
    }

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
