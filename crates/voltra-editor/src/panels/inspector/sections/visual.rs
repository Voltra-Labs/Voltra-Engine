//! The sections of what an entity is and draws: its prefab, transform (2D
//! or 3D), UI
//! box and controls, sprite, tilemap, text, particles and shapes.

use voltra_core::egui::Ui;
use voltra_core::UiFrame;
use voltra_ecs::Entity;
use voltra_render::glam::Vec2;
use voltra_scene::{
    hierarchy, ui, ParticleEmitter, Sprite2D, Text, Tilemap, Transform2D, Transform3D, UiButton,
    UiLayout, UiRect, UiStack,
};

use super::super::mixed::{self, Fields};
use super::super::{
    animation, mesh_instance3d, particles, shapes2d, sprite, sprite_material, text, tilemap,
    transform, transform3d, ui_button, ui_controls, ui_rect, ui_slice, ui_stack,
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
    if let Some(transform) = gate(
        frame.world.get_mut::<Transform2D>(entity),
        multi,
        Transform2D::NAME,
    ) {
        mixed::marker(ui, multi, &[Transform2D::NAME]);
        let fields = Fields::of(multi, Transform2D::NAME);
        claim = claim.or(transform::show(ui, transform, fields));
    }
    editor.euler_hints.forget_dead(frame.world);
    if let Some(transform) = gate(
        frame.world.get_mut::<Transform3D>(entity),
        multi,
        Transform3D::NAME,
    ) {
        mixed::marker(ui, multi, &[Transform3D::NAME]);
        let fields = Fields::of(multi, Transform3D::NAME);
        claim = claim.or(transform3d::show(
            ui,
            entity,
            transform,
            &mut editor.euler_hints,
            fields,
        ));
    }
    if let Some(instance) = gate(
        frame.world.get_mut::<voltra_scene::MeshInstance3D>(entity),
        multi,
        voltra_scene::MeshInstance3D::NAME,
    ) {
        ui.separator();
        mixed::marker(ui, multi, &[voltra_scene::MeshInstance3D::NAME]);
        claim = claim.or(mesh_instance3d::show(ui, instance));
    }
    let parent_rect = ui_parent_rect(ui, frame, entity);
    let stacked = hierarchy::parent_of(frame.world, entity)
        .is_some_and(|parent| frame.world.get::<UiStack>(parent).is_some());
    if let Some(rect) = gate(frame.world.get_mut::<UiRect>(entity), multi, UiRect::NAME) {
        mixed::marker(ui, multi, &[UiRect::NAME]);
        claim = claim.or(ui_rect::show(ui, rect, parent_rect, stacked));
    }
    let shown = frame.world.get::<Sprite2D>(entity).map_or(0, |s| s.frame);
    if let Some(button) = gate(
        frame.world.get_mut::<UiButton>(entity),
        multi,
        UiButton::NAME,
    ) {
        mixed::marker(ui, multi, &[UiButton::NAME]);
        claim = claim.or(ui_button::show(ui, button, shown));
    }
    if multi.editable(&[UiStack::NAME]) {
        mixed::marker(ui, multi, &[UiStack::NAME]);
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
        .get::<Sprite2D>(entity)
        .and_then(|sprite| sprite.texture_handle)
        .and_then(|handle| frame.thumbnail(handle));
    let frames_in_atlas = frame
        .world
        .get::<Sprite2D>(entity)
        .and_then(|sprite| sprite.atlas_handle)
        .and_then(|handle| frame.atlases.try_get(handle))
        .map(|atlas| atlas.len())
        .unwrap_or(0);

    let sprite_shared = multi.editable(&[Sprite2D::NAME]);
    if let Some(sprite) = frame
        .world
        .get_mut::<Sprite2D>(entity)
        .filter(|_| sprite_shared)
    {
        ui.separator();
        mixed::marker(ui, multi, &[Sprite2D::NAME]);
        claim = claim.or(sprite::show(
            ui,
            sprite,
            Fields::of(multi, Sprite2D::NAME),
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
        .get_mut::<Sprite2D>(entity)
        .filter(|_| sprite_shared)
    {
        let materials = frame.materials.as_deref_mut();
        claim = claim.or(sprite_material::show(ui, &mut sprite.material, materials));
    }
    if multi.editable(&[voltra_scene::UiSlice::NAME]) {
        mixed::marker(ui, multi, &[voltra_scene::UiSlice::NAME]);
        claim = claim.or(ui_slice::show(ui, frame.world, entity));
    }
    if frame.world.get::<Sprite2D>(entity).is_some()
        && multi.editable(&[Sprite2D::NAME, voltra_scene::SpriteAnimation::NAME])
    {
        ui.separator();
        mixed::marker(ui, multi, &[voltra_scene::SpriteAnimation::NAME]);
        claim = claim.or(animation::show(ui, frame.world, entity, frames_in_atlas));
    }
    let sheet = frame
        .world
        .get::<Tilemap>(entity)
        .and_then(|map| map.texture_handle)
        .and_then(|handle| frame.thumbnail(handle));
    let map_shared = multi.editable(&[Tilemap::NAME]);
    if let Some(map) = frame
        .world
        .get_mut::<Tilemap>(entity)
        .filter(|_| map_shared)
    {
        ui.separator();
        mixed::marker(ui, multi, &[Tilemap::NAME]);
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
    if let Some(text) = gate(frame.world.get_mut::<Text>(entity), multi, Text::NAME) {
        ui.separator();
        mixed::marker(ui, multi, &[Text::NAME]);
        claim = claim.or(text::show(ui, text, frame.fonts, on_screen));
    }
    if let Some(emitter) = gate(
        frame.world.get_mut::<ParticleEmitter>(entity),
        multi,
        ParticleEmitter::NAME,
    ) {
        ui.separator();
        mixed::marker(ui, multi, &[ParticleEmitter::NAME]);
        let (edited, preset) = particles::show(
            ui,
            emitter,
            particles::Stores {
                textures: frame.textures,
                atlases: frame.atlases,
                device: frame.device,
                queue: frame.queue,
            },
        );
        claim = claim.or(edited);
        if let Some(preset) = preset {
            // Each from its own emitter: a preset keeps the texture,
            // flipbook and layers, which copying the primary's would not.
            for each in multi.all().collect::<Vec<_>>() {
                if particles::apply_preset(frame.world, each, preset) {
                    claim = Some("Apply particle preset");
                }
            }
            multi.done_for_each(ParticleEmitter::NAME);
        }
    }
    if let Some(line) = gate(
        frame.world.get_mut::<voltra_scene::Line2D>(entity),
        multi,
        voltra_scene::Line2D::NAME,
    ) {
        ui.separator();
        mixed::marker(ui, multi, &[voltra_scene::Line2D::NAME]);
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
        voltra_scene::Polygon2D::NAME,
    ) {
        ui.separator();
        mixed::marker(ui, multi, &[voltra_scene::Polygon2D::NAME]);
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
        voltra_scene::Trail2D::NAME,
    ) {
        ui.separator();
        mixed::marker(ui, multi, &[voltra_scene::Trail2D::NAME]);
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
