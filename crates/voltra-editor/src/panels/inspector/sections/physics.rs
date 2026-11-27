//! The sections of how an entity collides and what the game adds: its
//! collider, filter, character body, the game's components, shadow caster
//! and sorting layer.

use voltra_core::egui::Ui;
use voltra_core::UiFrame;
use voltra_ecs::Entity;
use voltra_render::glam::Vec2;
use voltra_scene::{Collider, Sprite, Tilemap};

use super::super::{character, collider, collision, game_components, shadow_caster, sorting_layer};
use crate::editor::Editor;

/// Every section of these `entity` has, top to bottom; what to call the
/// edit one of them is in the middle of, if any.
pub(in crate::panels::inspector) fn show(
    editor: &mut Editor,
    ui: &mut Ui,
    frame: &mut UiFrame<'_>,
    entity: Entity,
) -> Option<&'static str> {
    let mut claim = None;
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
            // Fitted to the frame shown: the authored one.
            editor
                .timeline
                .end_record(frame.world, frame.animation_clips);
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
        let starting =
            voltra_scene::shadow_caster::starting_polygon(frame.world, entity, frame.sheets());
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
                recording: editor.timeline.is_recording(),
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
    claim
}

/// Shapes `entity`'s collider as the convex hull of its sprite's opaque
/// pixels, grown to eight corners at most; a sprite with nothing opaque, or
/// silhouettes not loaded, leaves it as it was and says why.
fn fit_to_sprite(frame: &mut UiFrame<'_>, entity: Entity) -> Option<&'static str> {
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
