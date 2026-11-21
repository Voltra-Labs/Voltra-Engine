//! Right panel: the components of the selected entity.
//!
//! One file per component's widget, under [`show`], which is only the list of
//! them and the rule that turns an edit into an undo entry.

mod animation;
mod audio_source;
mod camera;
mod collision;
mod light;
mod name;
mod parallax;
mod particles;
mod path_field;
mod sorting_layer;
mod sprite;
mod text;
mod tilemap;
mod transform;
mod ui_button;
mod ui_rect;

use voltra_core::egui::{self, Color32, RichText, Ui};
use voltra_core::UiFrame;
use voltra_render::glam::Vec2;
use voltra_scene::{
    hierarchy, ui, AudioSource, Camera, Collider, Parallax, ParticleEmitter, SceneId, Sprite, Text,
    Tilemap, Transform, UiButton, UiLayout, UiRect,
};

use crate::editor::Editor;
use crate::undo::SceneView;

pub fn show(editor: &mut Editor, ui: &mut Ui, frame: &mut UiFrame<'_>) {
    egui::Panel::right("inspector")
        .default_size(240.0)
        .show(ui, |ui| {
            ui.heading("Inspector");
            ui.separator();

            // A stale handle is normal, not a bug: the entity may have been
            // despawned since it was selected.
            let Some(entity) = editor.selected.filter(|e| frame.world.is_alive(*e)) else {
                ui.label(RichText::new("nothing selected").italics().weak());
                return;
            };

            // Every `DragValue` below gets its id from layout position, not from
            // `entity`. Without this, selecting a different sprite that lands the
            // same row on screen reuses the previous widget's id, and egui — which
            // still thinks that id is mid-edit — applies the leftover text to the
            // new entity instead of the one the user clicked. Salting the whole
            // body by `entity` makes every id here a function of the entity too, so
            // a selection change is a real id change and egui starts clean.
            ui.push_id(entity, |ui| {
                ui.label(format!(
                    "Entity {} (gen {})",
                    entity.index(),
                    entity.generation()
                ));

                // The name, then what it hangs off. Both are identity rather
                // than geometry, so they sit above the components.
                let mut claim: Option<&'static str> = name::show(ui, frame.world, entity);
                if let Some(parent) = hierarchy::parent_of(frame.world, entity) {
                    ui.label(
                        RichText::new(format!("child of {}", name::of(frame.world, parent))).weak(),
                    );
                }
                ui.separator();
                if let Some(transform) = frame.world.get_mut::<Transform>(entity) {
                    claim = transform::show(ui, transform);
                }
                let parent_rect = ui_parent_rect(ui, frame, entity);
                if let Some(rect) = frame.world.get_mut::<UiRect>(entity) {
                    claim = claim.or(ui_rect::show(ui, rect, parent_rect));
                }
                if let Some(button) = frame.world.get_mut::<UiButton>(entity) {
                    claim = claim.or(ui_button::show(ui, button));
                }
                let on_screen = ui::is_ui(frame.world, entity);
                // Both are read before the world is borrowed mutably below:
                // the thumbnail map and the atlas store hang off `frame`, and
                // a widget holding a component out of `frame.world` rules out
                // asking `frame` anything else.
                let preview = frame
                    .world
                    .get::<Sprite>(entity)
                    .and_then(|sprite| sprite.texture_handle)
                    .and_then(|handle| frame.thumbnail(handle));
                let frames_in_atlas = frame
                    .world
                    .get::<Sprite>(entity)
                    .and_then(|sprite| sprite.atlas_handle)
                    .and_then(|handle| frame.atlases.try_get(handle))
                    .map(|atlas| atlas.len())
                    .unwrap_or(0);

                if let Some(sprite) = frame.world.get_mut::<Sprite>(entity) {
                    ui.separator();
                    claim = claim.or(sprite::show(
                        ui,
                        sprite,
                        frame.textures,
                        frame.atlases,
                        frame.device,
                        frame.queue,
                        preview,
                    ));
                }
                if frame.world.get::<Sprite>(entity).is_some() {
                    ui.separator();
                    claim = claim.or(animation::show(ui, frame.world, entity, frames_in_atlas));
                }
                let sheet = frame
                    .world
                    .get::<Tilemap>(entity)
                    .and_then(|map| map.texture_handle)
                    .and_then(|handle| frame.thumbnail(handle));
                if let Some(map) = frame.world.get_mut::<Tilemap>(entity) {
                    ui.separator();
                    claim = claim.or(tilemap::show(
                        ui,
                        map,
                        frame.textures,
                        frame.atlases,
                        frame.device,
                        frame.queue,
                        tilemap::Palette {
                            sheet,
                            paint: &mut editor.paint,
                        },
                    ));
                }
                if let Some(text) = frame.world.get_mut::<Text>(entity) {
                    ui.separator();
                    claim = claim.or(text::show(ui, text, frame.fonts, on_screen));
                }
                if let Some(emitter) = frame.world.get_mut::<ParticleEmitter>(entity) {
                    ui.separator();
                    claim = claim.or(particles::show(ui, emitter));
                }
                if let Some(layer) = frame.world.get_mut::<Parallax>(entity) {
                    ui.separator();
                    claim = claim.or(parallax::show(ui, layer));
                }
                if let Some(lamp) = frame.world.get_mut::<voltra_scene::PointLight2D>(entity) {
                    ui.separator();
                    claim = claim.or(light::show(ui, lamp));
                }
                if let Some(camera) = frame.world.get_mut::<Camera>(entity) {
                    ui.separator();
                    claim = claim.or(camera::show(ui, camera));
                }
                if let Some(source) = frame.world.get_mut::<AudioSource>(entity) {
                    ui.separator();
                    claim = claim.or(audio_source::show(
                        ui,
                        source,
                        frame.clips,
                        frame.audio,
                        &mut editor.audition,
                    ));
                }
                // Keyed off the collider rather than off the filter itself:
                // both filter components are absent by default, and an entity
                // whose filter cannot be reached until it has one would never
                // get its first.
                // A solid layer too: its generated boxes copy the filter off
                // the layer, so the layer is where it is authored.
                let solid_layer = frame
                    .world
                    .get::<Tilemap>(entity)
                    .is_some_and(|map| map.collision);
                if frame.world.get::<Collider>(entity).is_some() || solid_layer {
                    ui.separator();
                    claim = claim.or(collision::show(ui, frame.world, entity));
                }
                // Every drawable's named layer, under its own section's
                // fields: one rule, one widget, wherever the component is.
                if let Some(sprite) = frame.world.get_mut::<Sprite>(entity) {
                    claim = claim.or(sorting_layer::show(
                        ui,
                        "sprite",
                        &mut sprite.sorting_layer,
                        frame.sorting_layers,
                    ));
                }
                if let Some(map) = frame.world.get_mut::<Tilemap>(entity) {
                    claim = claim.or(sorting_layer::show(
                        ui,
                        "tilemap",
                        &mut map.sorting_layer,
                        frame.sorting_layers,
                    ));
                }
                if let Some(text) = frame.world.get_mut::<Text>(entity) {
                    claim = claim.or(sorting_layer::show(
                        ui,
                        "text",
                        &mut text.sorting_layer,
                        frame.sorting_layers,
                    ));
                }
                if let Some(emitter) = frame.world.get_mut::<ParticleEmitter>(entity) {
                    claim = claim.or(sorting_layer::show(
                        ui,
                        "particles",
                        &mut emitter.sorting_layer,
                        frame.sorting_layers,
                    ));
                }
                // A field held down or focused keeps one entry open for the
                // whole edit, the way a gizmo drag does. Collected and applied
                // here rather than pushed from inside the widget helpers: they
                // hold a `&mut` borrowed out of the world, and the history has
                // to read the world.
                if let Some(label) = claim {
                    editor.history.claim(label);
                }

                ui.separator();
                if ui
                    .button(RichText::new("Delete").color(Color32::LIGHT_RED))
                    .clicked()
                {
                    // Explicit rather than watched: the watcher's `after` is
                    // read at the end of the frame from an entity that no longer
                    // exists, which is right, but the selection has to be
                    // recorded as it was *before* the despawn cleared it.
                    let id = frame.world.get::<SceneId>(entity).copied();
                    // Everything under it goes too, so everything under it has
                    // to be in the entry — an undo that revived the parent
                    // alone would leave its children deleted for good.
                    let going: Vec<SceneId> = id
                        .into_iter()
                        .chain(
                            hierarchy::descendants(frame.world, entity)
                                .into_iter()
                                .filter_map(|e| frame.world.get::<SceneId>(e).copied()),
                        )
                        .collect();
                    let view = SceneView {
                        world: frame.world,
                        registry: &editor.registry,
                        selected: id,
                    };
                    editor.history.begin("Delete", view, going);

                    hierarchy::despawn_recursive(frame.world, entity);
                    editor.selected = None;

                    let view = SceneView {
                        world: frame.world,
                        registry: &editor.registry,
                        selected: None,
                    };
                    editor.history.commit(view);
                }
            });
        });
}

/// The box `entity`'s `UiRect` is anchored in, as the viewport lays it out:
/// its nearest UI ancestor's rect, or the whole viewport.
fn ui_parent_rect(ui: &Ui, frame: &UiFrame<'_>, entity: voltra_ecs::Entity) -> ui::Rect {
    let (width, height) = frame.viewport_size();
    let screen = Vec2::new(width as f32, height as f32) / ui.ctx().pixels_per_point();
    let layout = UiLayout::from_world(frame.world, screen);
    hierarchy::ancestors(frame.world, entity)
        .into_iter()
        .find_map(|ancestor| layout.rect(ancestor))
        .unwrap_or(ui::Rect::screen(screen))
}

/// What to call an edit made through this widget this frame, if it is live.
///
/// Dragged *or* focused: a `DragValue` can also be clicked into and typed in,
/// and an entry that closed between two keystrokes would be one undo per
/// character.
pub(super) fn active(response: &egui::Response, label: &'static str) -> Option<&'static str> {
    (response.dragged() || response.has_focus()).then_some(label)
}
