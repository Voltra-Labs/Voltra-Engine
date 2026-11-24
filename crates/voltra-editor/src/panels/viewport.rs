//! Central panel: the rendered scene image.
//!
//! Layout only. What the pointer and keyboard do with it is
//! [`crate::camera::ViewportCamera`]'s job.

use voltra_core::egui::{self, Ui};
use voltra_core::UiFrame;

use crate::editor::Editor;
use crate::paint::PaintTool;
use crate::play::PlayState;
use crate::tool::Tool;
use crate::view;

mod drop;
mod select;

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

            // `W`, `E` and `R` pick the transform tool, `B`, `U` and `D` the
            // paint tools. Scoped the same way the camera scopes its own keys —
            // hovered, and egui not wanting the keyboard — so typing a `w` into
            // the inspector cannot switch tools. Not while the right button is
            // held either: that is the WASD fly chord, and `D` is in it.
            let selected_layer = editor
                .selection
                .primary()
                .filter(|&e| frame.world.get::<voltra_scene::Tilemap>(e).is_some());
            if scene.hovered() && !ui.ctx().egui_wants_keyboard_input() {
                ui.input(|i| {
                    if i.pointer.secondary_down() {
                        return;
                    }
                    for tool in Tool::ALL {
                        if i.key_pressed(tool.key()) {
                            editor.tool = tool;
                            editor.paint.set_tool(None);
                            editor.shape.set_editing(None);
                        }
                    }
                    // Only with a layer to paint: arming a brush with nothing
                    // to paint on would leave the gizmo hidden for no reason.
                    if selected_layer.is_some() {
                        for tool in PaintTool::ALL {
                            if i.key_pressed(tool.key()) {
                                editor.paint.set_tool(Some(tool));
                                editor.shape.set_editing(None);
                            }
                        }
                    }
                });
            }
            // Selecting something that is not a layer disarms painting, so the
            // toolbar never shows a paint tool pressed over a gizmo.
            if selected_layer.is_none() {
                editor.paint.set_tool(None);
            }

            // While a caster's shape is edited, its handles have the pointer,
            // as a painted layer does below.
            if let Some(caster) = editor.shape.target(frame.world, editor.selection.primary()) {
                editor.gizmo.cancel_drag();
                editor.marquee.cancel();
                if let Some(label) = editor.shape.update(ui, &scene, frame, caster) {
                    editor.history.claim(label);
                }
                drop::drop_asset(editor, frame, ui, &scene);
                editor.camera.navigate(ui, &scene, frame.camera);
                let outlines = voltra_scene::shadow_caster::outlines_of(
                    frame.world,
                    caster,
                    frame.sprite_shapes(),
                );
                view::caster::draw(&outlines, frame.lines());
                editor.shape.draw(&scene, frame, caster);
                if editor.show_colliders {
                    let contacts = frame.contacts();
                    let (world, lines) = frame.world_and_lines();
                    voltra_physics::debug::draw(world, contacts, lines);
                }
                return;
            }

            // While painting, the layer has the pointer: no gizmo, and a click
            // paints rather than selects.
            if let Some(layer) = editor.paint.target(frame.world, editor.selection.primary()) {
                editor.gizmo.cancel_drag();
                editor.marquee.cancel();
                if let Some(label) = editor.paint.update(ui, &scene, frame, layer) {
                    editor.history.claim(label);
                }
                drop::drop_asset(editor, frame, ui, &scene);
                editor.camera.navigate(ui, &scene, frame.camera);
                let (world, lines) = frame.world_and_lines();
                editor.paint.draw(world, layer, lines);
                if editor.show_colliders {
                    let contacts = frame.contacts();
                    let (world, lines) = frame.world_and_lines();
                    voltra_physics::debug::draw(world, contacts, lines);
                }
                return;
            }

            // The gizmo gets first refusal, for the same reason egui gets it
            // before the scene does: a handle drawn over a sprite has to be
            // grabbable, and a drag on one must not also re-select.
            let outcome =
                editor
                    .gizmo
                    .update(&scene, frame, &editor.selection, editor.tool, &editor.snap);

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
            drop::drop_asset(editor, frame, ui, &scene);

            // Before navigation: a click and a drag are mutually exclusive in
            // egui, so this cannot swallow a pan.
            select::update(editor, &scene, frame, outcome.consumed);

            editor.camera.navigate(ui, &scene, frame.camera);

            // The grid under everything else drawn over the scene.
            if editor.snap.enabled {
                let camera = *frame.camera;
                editor.snap.draw_grid(&camera, frame.lines());
            }
            // After navigation, so the gizmo is laid out against the camera
            // this frame ends with rather than the one it started with.
            editor
                .gizmo
                .draw(&scene, frame, editor.selection.primary(), editor.tool);

            // What the selected camera would show, if it is one. After the
            // gizmo, whose handles are what the pointer is aiming at, and
            // before the colliders, which are the noisiest layer of the three.
            if let Some(entity) = editor.selection.live_primary(frame.world) {
                // Read before the split borrow below, which takes the frame
                // mutably and would rule out reaching the camera.
                let aspect = frame.camera.aspect;
                let outlines = voltra_scene::shadow_caster::outlines_of(
                    frame.world,
                    entity,
                    frame.sprite_shapes(),
                );
                let (world, lines) = frame.world_and_lines();
                view::overlay::draw(world, entity, aspect, lines);
                view::caster::draw(&outlines, lines);
                view::light::draw(world, entity, lines);
            }
            // Every bone, over the art it moves, the selected one lit.
            let selected = editor.selection.live_primary(frame.world);
            let per_pixel = frame.camera.half_extents().x * 2.0 / scene.rect.width().max(1.0);
            let (world, lines) = frame.world_and_lines();
            view::bones::draw(world, selected, per_pixel, lines);
            select::draw(editor, frame);

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
