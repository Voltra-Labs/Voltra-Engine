//! The sections of what an entity is and draws: its prefab, transform, UI
//! box and controls, sprite, tilemap, text, particles and shapes.

use voltra_core::egui::Ui;
use voltra_core::UiFrame;
use voltra_ecs::Entity;
use voltra_render::glam::Vec2;
use voltra_scene::{
    hierarchy, ui, ParticleEmitter, Sprite, Text, Tilemap, Transform, UiButton, UiLayout, UiRect,
    UiStack,
};

use super::super::mixed::{self, Fields};
use super::super::{
    animation, particles, prefab, shapes2d, sprite, sprite_material, text, tilemap, transform,
    ui_button, ui_controls, ui_rect, ui_slice, ui_stack,
};
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
    prefab::show(editor, ui, frame, entity);
    if let Some(transform) = gate(frame.world.get_mut::<Transform>(entity), multi, "Transform") {
        mixed::marker(ui, multi, &["Transform"]);
        let fields = Fields::of(multi, "Transform");
        claim = claim.or(transform::show(ui, transform, fields));
    }
    let parent_rect = ui_parent_rect(ui, frame, entity);
    let stacked = hierarchy::parent_of(frame.world, entity)
        .is_some_and(|parent| frame.world.get::<UiStack>(parent).is_some());
    if let Some(rect) = gate(frame.world.get_mut::<UiRect>(entity), multi, "UiRect") {
        mixed::marker(ui, multi, &["UiRect"]);
        claim = claim.or(ui_rect::show(ui, rect, parent_rect, stacked));
    }
    let shown = frame.world.get::<Sprite>(entity).map_or(0, |s| s.frame);
    if let Some(button) = gate(frame.world.get_mut::<UiButton>(entity), multi, "UiButton") {
        mixed::marker(ui, multi, &["UiButton"]);
        claim = claim.or(ui_button::show(ui, button, shown));
    }
    if multi.editable(&["UiStack"]) {
        mixed::marker(ui, multi, &["UiStack"]);
        claim = claim.or(ui_stack::show(ui, frame.world, entity));
    }
    claim = claim.or(ui_controls::show(ui, frame.world, entity, multi));
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

    let sprite_shared = multi.editable(&["Sprite"]);
    if let Some(sprite) = frame
        .world
        .get_mut::<Sprite>(entity)
        .filter(|_| sprite_shared)
    {
        ui.separator();
        mixed::marker(ui, multi, &["Sprite"]);
        claim = claim.or(sprite::show(
            ui,
            sprite,
            Fields::of(multi, "Sprite"),
            particles::Stores {
                textures: frame.textures,
                atlases: frame.atlases,
                device: frame.device,
                queue: frame.queue,
            },
            preview,
        ));
    }
    if let Some(sprite) = frame
        .world
        .get_mut::<Sprite>(entity)
        .filter(|_| sprite_shared)
    {
        let materials = frame.materials.as_deref_mut();
        claim = claim.or(sprite_material::show(ui, &mut sprite.material, materials));
    }
    if multi.editable(&["UiSlice"]) {
        mixed::marker(ui, multi, &["UiSlice"]);
        claim = claim.or(ui_slice::show(ui, frame.world, entity));
    }
    if frame.world.get::<Sprite>(entity).is_some() && multi.editable(&["Sprite", "SpriteAnimation"])
    {
        ui.separator();
        mixed::marker(ui, multi, &["SpriteAnimation"]);
        claim = claim.or(animation::show(ui, frame.world, entity, frames_in_atlas));
    }
    let sheet = frame
        .world
        .get::<Tilemap>(entity)
        .and_then(|map| map.texture_handle)
        .and_then(|handle| frame.thumbnail(handle));
    let map_shared = multi.editable(&["Tilemap"]);
    if let Some(map) = frame
        .world
        .get_mut::<Tilemap>(entity)
        .filter(|_| map_shared)
    {
        ui.separator();
        mixed::marker(ui, multi, &["Tilemap"]);
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
                window: &mut editor.tile_palette.open,
            },
        ));
    }
    if let Some(map) = frame
        .world
        .get_mut::<Tilemap>(entity)
        .filter(|_| map_shared)
    {
        let materials = frame.materials.as_deref_mut();
        claim = claim.or(sprite_material::show(ui, &mut map.material, materials));
    }
    if let Some(text) = gate(frame.world.get_mut::<Text>(entity), multi, "Text") {
        ui.separator();
        mixed::marker(ui, multi, &["Text"]);
        claim = claim.or(text::show(ui, text, frame.fonts, on_screen));
    }
    if let Some(emitter) = gate(
        frame.world.get_mut::<ParticleEmitter>(entity),
        multi,
        "ParticleEmitter",
    ) {
        ui.separator();
        mixed::marker(ui, multi, &["ParticleEmitter"]);
        claim = claim.or(particles::show(
            ui,
            emitter,
            particles::Stores {
                textures: frame.textures,
                atlases: frame.atlases,
                device: frame.device,
                queue: frame.queue,
            },
        ));
    }
    if let Some(line) = gate(
        frame.world.get_mut::<voltra_scene::Line2D>(entity),
        multi,
        "Line2D",
    ) {
        ui.separator();
        mixed::marker(ui, multi, &["Line2D"]);
        claim = claim.or(shapes2d::line(
            ui,
            line,
            shapes2d::Context {
                entity,
                shape: &mut editor.shape,
                textures: frame.textures,
                device: frame.device,
                queue: frame.queue,
            },
        ));
    }
    if let Some(polygon) = gate(
        frame.world.get_mut::<voltra_scene::Polygon2D>(entity),
        multi,
        "Polygon2D",
    ) {
        ui.separator();
        mixed::marker(ui, multi, &["Polygon2D"]);
        claim = claim.or(shapes2d::polygon(
            ui,
            polygon,
            shapes2d::Context {
                entity,
                shape: &mut editor.shape,
                textures: frame.textures,
                device: frame.device,
                queue: frame.queue,
            },
        ));
    }
    if let Some(trail) = gate(
        frame.world.get_mut::<voltra_scene::Trail2D>(entity),
        multi,
        "Trail2D",
    ) {
        ui.separator();
        mixed::marker(ui, multi, &["Trail2D"]);
        claim = claim.or(shapes2d::trail(
            ui,
            trail,
            shapes2d::Context {
                entity,
                shape: &mut editor.shape,
                textures: frame.textures,
                device: frame.device,
                queue: frame.queue,
            },
        ));
    }
    claim
}

/// The box `entity`'s `UiRect` is anchored in, as the viewport lays it out:
/// its nearest UI ancestor's rect, or the whole viewport.
fn ui_parent_rect(ui: &Ui, frame: &UiFrame<'_>, entity: Entity) -> ui::Rect {
    let (width, height) = frame.viewport_size();
    let screen = Vec2::new(width as f32, height as f32) / ui.ctx().pixels_per_point();
    let layout = UiLayout::from_world(frame.world, screen);
    hierarchy::ancestors(frame.world, entity)
        .into_iter()
        .find_map(|ancestor| layout.rect(ancestor))
        .unwrap_or(ui::Rect::screen(screen))
}
