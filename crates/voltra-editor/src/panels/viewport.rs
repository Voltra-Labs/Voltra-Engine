//! Central panel: the rendered scene image.
//!
//! Layout only. What the pointer and keyboard do with it is
//! [`crate::camera::ViewportCamera`]'s job.

use voltra_assets::AssetPath;
use voltra_core::egui::{self, Ui};
use voltra_core::UiFrame;

use crate::drag;
use crate::editor::Editor;
use crate::play::PlayState;
use crate::spawn;
use crate::tool::Tool;
use crate::view;

/// What the viewport image is multiplied by while the editor is not editing.
///
/// Unity's play-mode tint, and for Unity's reason: the oldest complaint about
/// that editor is work lost because play mode was not obvious. Applied through
/// egui's image tint, so it is a UI-level effect — nothing in the render path
/// or in the scene knows that play mode exists.
const PLAY_TINT: egui::Color32 = egui::Color32::from_rgb(150, 175, 225);

pub fn show(editor: &mut Editor, ui: &mut Ui, frame: &mut UiFrame<'_>) {
    egui::CentralPanel::default()
        // The scene brings its own background; the panel's would only show
        // as a border around the image.
        .frame(egui::Frame::NONE)
        .show(ui, |ui| {
            let available = ui.available_size();
            // egui lays out in logical points while the target is sized in
            // physical pixels. Missing this conversion renders the scene at
            // half resolution on a 200% display.
            let scale = ui.ctx().pixels_per_point();
            frame.request_viewport_size((available.x * scale) as u32, (available.y * scale) as u32);

            let mut image =
                egui::Image::new(egui::load::SizedTexture::new(frame.viewport(), available))
                    // Without this the image is inert decoration and the
                    // pointer never reaches the camera or the selection.
                    .sense(egui::Sense::click_and_drag());
            if editor.play.state() != PlayState::Editing {
                image = image.tint(PLAY_TINT);
            }
            let scene = ui.add(image);

            // The game view is the scene's camera looking at the scene, and
            // nothing else: no navigation, no tools, no handles and no drop
            // target, because none of those exist in a running game. Everything
            // below this is the editor looking at the scene instead.
            if editor.view.is_game() {
                if !editor.view.show_game_camera(frame.world, frame.camera) {
                    no_camera(ui, &scene);
                }
                return;
            }

            // `W`, `E` and `R` pick the tool. Scoped the same way the camera
            // scopes its own keys — hovered, and egui not wanting the keyboard
            // — so typing a `w` into the inspector cannot switch tools.
            if scene.hovered() && !ui.ctx().egui_wants_keyboard_input() {
                ui.input(|i| {
                    for tool in Tool::ALL {
                        if i.key_pressed(tool.key()) {
                            editor.tool = tool;
                        }
                    }
                });
            }

            // The gizmo gets first refusal, for the same reason egui gets it
            // before the scene does: a handle drawn over a sprite has to be
            // grabbable, and a drag on one must not also re-select.
            let outcome = editor
                .gizmo
                .update(&scene, frame, editor.selected, editor.tool);

            // Every frame the drag holds an entity, so the history keeps one
            // entry open for the whole drag instead of one per frame. The entry
            // closes on the release frame, which is the first frame no claim
            // arrives.
            //
            // The label comes off the drag rather than off `editor.tool`: a key
            // pressed mid-drag arms the next grab, and an entry that changed its
            // name halfway through would be undone under a verb it never did.
            if outcome.dragging.is_some() {
                let tool = editor.gizmo.active_tool().unwrap_or(editor.tool);
                editor.history.claim(tool.label());
            }

            // Before the click and the camera: a release that carries an asset
            // is a drop, not a selection and not the end of a pan.
            drop_asset(editor, frame, ui, &scene);

            // Before navigation: a click and a drag are mutually exclusive in
            // egui, so this cannot swallow a pan.
            if !outcome.consumed && scene.clicked() {
                editor.selected = crate::picking::clicked_entity(&scene, frame);
            }

            editor.camera.navigate(ui, &scene, frame.camera);

            // After navigation, so the gizmo is laid out against the camera
            // this frame ends with rather than the one it started with.
            editor
                .gizmo
                .draw(&scene, frame, editor.selected, editor.tool);

            // What the selected camera would show, if it is one. After the
            // gizmo, whose handles are what the pointer is aiming at, and
            // before the colliders, which are the noisiest layer of the three.
            if let Some(entity) = editor.selected.filter(|e| frame.world.is_alive(*e)) {
                // Read before the split borrow below, which takes the frame
                // mutably and would rule out reaching the camera.
                let aspect = frame.camera.aspect;
                let (world, lines) = frame.world_and_lines();
                view::overlay::draw(world, entity, aspect, lines);
            }

            // Last of all: the overlay is drawn in the order it is pushed, and
            // a collider outline must not cover the handle being dragged over
            // it.
            if editor.show_colliders {
                let contacts = frame.contacts();
                let (world, lines) = frame.world_and_lines();
                voltra_physics::debug::draw(world, contacts, lines);
            }
        });
}

/// Says the game view has nothing to show, over the frame it cannot draw.
///
/// The last framing stays on screen underneath rather than being blacked out:
/// it is the more useful of the two pictures while the author works out which
/// camera to add or enable, and the words are what say it is not the game's.
/// Unity writes the same sentence into its Game view for the same reason.
fn no_camera(ui: &mut Ui, scene: &egui::Response) {
    ui.put(
        scene.rect,
        egui::Label::new(
            egui::RichText::new("No active camera")
                .heading()
                .color(egui::Color32::WHITE)
                .background_color(egui::Color32::from_black_alpha(160)),
        )
        .selectable(false),
    );
}

/// Adds a sprite where an asset was dropped, as one undo entry.
///
/// Unity, Unreal and Godot all instantiate on drop rather than opening a
/// dialogue, and all three put the new object where the pointer released rather
/// than at the origin — the drop *is* the placement, and a sprite that appears
/// somewhere else has to be moved for no reason.
///
/// Refused while playing, like every other edit: the scene the history is
/// addressed against is the authored one, and Stop would throw the sprite away
/// without saying so.
fn drop_asset(editor: &mut Editor, frame: &mut UiFrame<'_>, ui: &Ui, scene: &egui::Response) {
    if editor.play.state() != PlayState::Editing {
        return;
    }

    // What the drop would do, before the release. A viewport that accepts a
    // drag silently and only reacts once it is too late to change your mind is
    // the complaint every content browser gets.
    if scene.dnd_hover_payload::<AssetPath>().is_some() {
        drag::outline(ui, scene.rect, ui.visuals().selection.bg_fill);
    }

    let Some(path) = scene.dnd_release_payload::<AssetPath>() else {
        return;
    };
    // `Response::interact_pointer_pos` is `None` on the release frame of a drag
    // that started in another widget, which is every drop this handles.
    let Some(pointer) = ui.ctx().pointer_interact_pos() else {
        return;
    };
    let at = crate::picking::world_at(scene, pointer, frame);
    // Which of the two the drop means is the file it names: a slicing brings
    // its own sheet with it, a bare image is the sheet.
    let sliced = voltra_assets::atlas::is_atlas(path.as_str());
    spawn::record(editor, frame, "Add sprite", |frame| {
        if sliced {
            spawn::atlas_sprite(frame, at, &path)
        } else {
            spawn::textured_sprite(frame, at, &path)
        }
    });
}
