//! Right panel: the components of the selected entity.
//!
//! One file per component's widget, under [`show`], which is only the list of
//! them and the rule that turns an edit into an undo entry.

mod animation;
mod animation_player;
mod animator;
mod audio_source;
mod camera;
mod camera_rig;
mod character;
mod collider;
mod collision;
mod delete;
mod game_components;
mod global_light;
mod joint;
mod light;
mod light_layers;
mod name;
mod parallax;
mod particles;
mod path_field;
mod prefab;
mod shadow_caster;
mod sorting_layer;
mod sprite;
mod text;
mod tilemap;
mod transform;
mod ui_button;
mod ui_rect;
mod ui_slice;
mod ui_stack;

use voltra_core::egui::{self, RichText, Ui};
use voltra_core::UiFrame;
use voltra_render::glam::Vec2;
use voltra_scene::{
    hierarchy, ui, AudioSource, Camera, Collider, Parallax, ParticleEmitter, Sprite, Text, Tilemap,
    Transform, UiButton, UiLayout, UiRect, UiStack,
};

use crate::editor::Editor;

pub fn show(editor: &mut Editor, ui: &mut Ui, frame: &mut UiFrame<'_>) {
    egui::Panel::right("inspector")
        .default_size(240.0)
        .show(ui, |ui| {
            ui.heading("Inspector");
            ui.separator();

            // A stale handle is normal, not a bug: the entity may have been
            // despawned since it was selected.
            let Some(entity) = editor.selection.live_primary(frame.world) else {
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
                // One entity is edited at a time; with more selected, say so,
                // so an edit here is not taken for an edit to all of them.
                let more = match editor.selection.len() {
                    0 | 1 => String::new(),
                    n => format!(" — the last picked of {n}"),
                };
                ui.label(format!(
                    "Entity {} (gen {}){more}",
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
                prefab::show(editor, ui, frame, entity);
                if let Some(transform) = frame.world.get_mut::<Transform>(entity) {
                    claim = transform::show(ui, transform);
                }
                let parent_rect = ui_parent_rect(ui, frame, entity);
                let stacked = hierarchy::parent_of(frame.world, entity)
                    .is_some_and(|parent| frame.world.get::<UiStack>(parent).is_some());
                if let Some(rect) = frame.world.get_mut::<UiRect>(entity) {
                    claim = claim.or(ui_rect::show(ui, rect, parent_rect, stacked));
                }
                let shown = frame.world.get::<Sprite>(entity).map_or(0, |s| s.frame);
                if let Some(button) = frame.world.get_mut::<UiButton>(entity) {
                    claim = claim.or(ui_button::show(ui, button, shown));
                }
                claim = claim.or(ui_stack::show(ui, frame.world, entity));
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
                claim = claim.or(ui_slice::show(ui, frame.world, entity));
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
                    claim = claim.or(light::show(
                        ui,
                        lamp,
                        frame.textures,
                        frame.device,
                        frame.queue,
                    ));
                }
                if frame.world.get::<voltra_scene::Joint2D>(entity).is_some() {
                    let ends = joint::ends(frame.world, entity);
                    if let Some(j) = frame.world.get_mut::<voltra_scene::Joint2D>(entity) {
                        ui.separator();
                        claim = claim.or(joint::show(ui, j, &ends));
                    }
                }
                claim = claim.or(animation_player::show(ui, frame.world, entity));
                claim = claim.or(animator::show(
                    ui,
                    frame.world,
                    frame.state_machines,
                    entity,
                ));
                if let Some(glow) = frame.world.get_mut::<voltra_scene::GlobalLight2D>(entity) {
                    ui.separator();
                    claim = claim.or(global_light::show(ui, glow));
                }
                if let Some(camera) = frame.world.get_mut::<Camera>(entity) {
                    ui.separator();
                    claim = claim.or(camera::show(ui, camera));
                }
                claim = claim.or(camera_rig::show(ui, frame.world, entity));
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
                let has_collider = frame.world.get::<Collider>(entity).is_some();
                let textured = frame
                    .world
                    .get::<Sprite>(entity)
                    .is_some_and(|sprite| sprite.texture.is_some());
                if let Some(shape) = frame.world.get_mut::<Collider>(entity) {
                    ui.separator();
                    let outcome = collider::show(ui, shape, textured);
                    claim = claim.or(outcome.claim);
                    if outcome.fit_to_sprite {
                        claim = claim.or(fit_to_sprite(frame, entity));
                    }
                }
                if has_collider || solid_layer {
                    ui.separator();
                    claim = claim.or(collision::show(ui, frame.world, entity));
                }
                claim = claim.or(character::show(ui, frame.world, entity));
                claim = claim.or(game_components::show(
                    ui,
                    frame.world,
                    entity,
                    &editor.registry,
                ));
                // Keyed off the shape it would take — a collider, a solid
                // layer's boxes, a sprite's quad — or off the caster itself.
                let has_sprite = frame.world.get::<Sprite>(entity).is_some();
                let casts = frame
                    .world
                    .get::<voltra_scene::ShadowCaster2D>(entity)
                    .is_some();
                if has_collider || solid_layer || has_sprite || casts {
                    // Read before the world is borrowed mutably below: the
                    // sprite's quad is resolved against the frame's sheets.
                    let starting = voltra_scene::shadow_caster::starting_polygon(
                        frame.world,
                        entity,
                        frame.sheets(),
                    );
                    ui.separator();
                    claim = claim.or(shadow_caster::show(
                        ui,
                        frame.world,
                        entity,
                        shadow_caster::Context {
                            has_collider: has_collider || solid_layer,
                            starting,
                            textured_sprite: frame
                                .world
                                .get::<Sprite>(entity)
                                .is_some_and(|sprite| sprite.texture.is_some()),
                            shape: &mut editor.shape,
                        },
                    ));
                }
                // Every drawable's named layer, under its own section's
                // fields: one rule, one widget, wherever the component is.
                claim = claim.or(sorting_layer::show_for(
                    ui,
                    frame.world,
                    entity,
                    frame.sorting_layers,
                ));
                // A field held down or focused keeps one entry open for the
                // whole edit, the way a gizmo drag does. Collected and applied
                // here rather than pushed from inside the widget helpers: they
                // hold a `&mut` borrowed out of the world, and the history has
                // to read the world.
                if let Some(label) = claim {
                    editor.history.claim(label);
                }

                ui.separator();
                delete::show(editor, ui, frame);
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
/// Shapes `entity`'s collider as the convex hull of its sprite's opaque
/// pixels, grown to eight corners at most; a sprite with nothing opaque, or
/// silhouettes not loaded, leaves it as it was and says why.
fn fit_to_sprite(frame: &mut UiFrame<'_>, entity: voltra_ecs::Entity) -> Option<&'static str> {
    let points: Vec<Vec2> = frame
        .sprite_silhouette(entity)
        .into_iter()
        .flatten()
        .collect();
    match voltra_scene::ConvexPolygon::enclosing(&points) {
        Ok(points) => {
            let collider = frame.world.get_mut::<Collider>(entity)?;
            *collider = Collider::Polygon {
                points,
                radius: 0.0,
            };
            Some("Fit collider to sprite")
        }
        Err(error) => {
            log::warn!("cannot fit the collider to the sprite: {error} in its silhouette");
            None
        }
    }
}

pub(super) fn active(response: &egui::Response, label: &'static str) -> Option<&'static str> {
    (response.dragged() || response.has_focus()).then_some(label)
}
