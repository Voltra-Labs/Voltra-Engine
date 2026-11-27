//! The sections of how an entity collides and what the game adds: its
//! collider, filter, character body, the game's components, shadow caster
//! and sorting layer.

use voltra_core::egui::Ui;
use voltra_core::UiFrame;
use voltra_ecs::Entity;
use voltra_render::glam::Vec2;
use voltra_scene::{Collider, Sprite, Tilemap};

use super::super::mixed;
use super::super::{character, collider, collision, game_components, shadow_caster, sorting_layer};
use super::gate;
use crate::editor::Editor;
use crate::multi_edit::MultiEdit;

/// Every section of these `entity` has, top to bottom; what to call the
/// edit one of them is in the middle of, if any.
pub(in crate::panels::inspector) fn show(
    editor: &mut Editor,
    ui: &mut Ui,
    frame: &mut UiFrame<'_>,
    entity: Entity,
    multi: &mut MultiEdit,
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
    if let Some(shape) = gate(frame.world.get_mut::<Collider>(entity), multi, "Collider") {
        ui.separator();
        mixed::marker(ui, multi, &["Collider"]);
        let outcome = collider::show(ui, shape, textured);
        claim = claim.or(outcome.claim);
        if outcome.fit_to_sprite {
            // Fitted to the frame shown: the authored one.
            editor
                .timeline
                .end_record(frame.world, frame.animation_clips);
            // Each to its own sprite, not the primary's outline copied.
            let fitted: Vec<Entity> = multi.all().collect();
            for each in fitted {
                claim = claim.or(fit_to_sprite(frame, each));
            }
            multi.done_for_each("Collider");
        }
    }
    let filter = ["CollisionLayers", "OneWay", "Sensor"];
    if (has_collider || solid_layer) && multi.editable(&filter) {
        ui.separator();
        mixed::marker(ui, multi, &filter);
        claim = claim.or(collision::show(ui, frame.world, entity));
    }
    let body = ["CharacterBody2D"];
    if multi.editable(&body) {
        mixed::marker(ui, multi, &body);
        claim = claim.or(character::show(ui, frame.world, entity));
    }
    claim = claim.or(game_components::show(
        ui,
        frame.world,
        entity,
        &editor.registry,
        multi,
    ));
    // Keyed off the shape it would take — a collider, a solid
    // layer's boxes, a sprite's quad — or off the caster itself.
    let has_sprite = frame.world.get::<Sprite>(entity).is_some();
    let casts = frame
        .world
        .get::<voltra_scene::ShadowCaster2D>(entity)
        .is_some();
    let caster = ["ShadowCaster2D"];
    if (has_collider || solid_layer || has_sprite || casts) && multi.editable(&caster) {
        // Read before the world is borrowed mutably below: the
        // sprite's quad is resolved against the frame's sheets.
        let starting =
            voltra_scene::shadow_caster::starting_polygon(frame.world, entity, frame.sheets());
        ui.separator();
        mixed::marker(ui, multi, &caster);
        let outcome = shadow_caster::show(
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
        );
        claim = claim.or(outcome);
        if outcome == Some(shadow_caster::OWN_POLYGON) {
            own_polygons_for_rest(frame, multi);
        }
        if !casts
            && frame
                .world
                .get::<voltra_scene::ShadowCaster2D>(entity)
                .is_some()
        {
            add_casters_to_rest(frame, multi);
        }
    }
    // Every drawable's named layer, under its own section's
    // fields: one rule, one widget, wherever the component is.
    claim = claim.or(sorting_layer::show_for(
        ui,
        frame.world,
        entity,
        frame.sorting_layers,
        multi,
    ));
    claim
}

/// Gives every other selected entity the caster ticking the box gives it —
/// each shaped by its own collider or sprite, where copying the primary's
/// would give them all its outline.
fn add_casters_to_rest(frame: &mut UiFrame<'_>, multi: &mut MultiEdit) {
    let rest: Vec<Entity> = multi.all().skip(1).collect();
    for entity in rest {
        if frame
            .world
            .get::<voltra_scene::ShadowCaster2D>(entity)
            .is_some()
        {
            continue;
        }
        let has_collider = frame.world.get::<Collider>(entity).is_some()
            || frame
                .world
                .get::<Tilemap>(entity)
                .is_some_and(|map| map.collision);
        let starting =
            voltra_scene::shadow_caster::starting_polygon(frame.world, entity, frame.sheets());
        let caster = shadow_caster::starting_caster(has_collider, starting);
        frame.world.insert(entity, caster);
    }
    multi.done_for_each("ShadowCaster2D");
}

/// Shapes every other selected caster by its own polygon — its collider's
/// outline or its sprite's quad — as "Use own polygon" did the primary's.
fn own_polygons_for_rest(frame: &mut UiFrame<'_>, multi: &mut MultiEdit) {
    let rest: Vec<Entity> = multi.all().skip(1).collect();
    for entity in rest {
        let starting =
            voltra_scene::shadow_caster::starting_polygon(frame.world, entity, frame.sheets());
        let (Some(points), Some(caster)) = (
            starting,
            frame.world.get_mut::<voltra_scene::ShadowCaster2D>(entity),
        ) else {
            continue;
        };
        caster.shape = voltra_scene::CasterShape::Polygon {
            points,
            closed: true,
        };
    }
    multi.done_for_each("ShadowCaster2D");
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
