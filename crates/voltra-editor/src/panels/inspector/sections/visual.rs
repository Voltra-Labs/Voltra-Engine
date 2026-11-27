//! The sections of what an entity is and draws: its prefab, transform, UI
//! box and controls, sprite, tilemap, text, particles, shapes, skin and
//! bones, parallax, light, joint, animation, camera and sound.

use voltra_core::egui::Ui;
use voltra_core::UiFrame;
use voltra_ecs::Entity;
use voltra_render::glam::Vec2;
use voltra_scene::{
    hierarchy, ui, AudioSource, Camera, Parallax, ParticleEmitter, Sprite, Text, Tilemap,
    Transform, UiButton, UiLayout, UiRect, UiStack,
};

use super::super::{
    animation, animation_player, animator, audio_source, camera, camera_rig, camera_view,
    global_light, joint, light, parallax, particles, prefab, shapes2d, skeleton, skinned, sprite,
    sprite_material, text, tilemap, transform, ui_button, ui_controls, ui_rect, ui_slice, ui_stack,
};
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
    prefab::show(editor, ui, frame, entity);
    if let Some(transform) = frame.world.get_mut::<Transform>(entity) {
        claim = claim.or(transform::show(ui, transform));
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
    claim = claim.or(ui_controls::show(ui, frame.world, entity));
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
    if let Some(sprite) = frame.world.get_mut::<Sprite>(entity) {
        let materials = frame.materials.as_deref_mut();
        claim = claim.or(sprite_material::show(ui, &mut sprite.material, materials));
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
                window: &mut editor.tile_palette.open,
            },
        ));
    }
    if let Some(map) = frame.world.get_mut::<Tilemap>(entity) {
        let materials = frame.materials.as_deref_mut();
        claim = claim.or(sprite_material::show(ui, &mut map.material, materials));
    }
    if let Some(text) = frame.world.get_mut::<Text>(entity) {
        ui.separator();
        claim = claim.or(text::show(ui, text, frame.fonts, on_screen));
    }
    if let Some(emitter) = frame.world.get_mut::<ParticleEmitter>(entity) {
        ui.separator();
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
    if let Some(line) = frame.world.get_mut::<voltra_scene::Line2D>(entity) {
        ui.separator();
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
    if let Some(polygon) = frame.world.get_mut::<voltra_scene::Polygon2D>(entity) {
        ui.separator();
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
    if let Some(trail) = frame.world.get_mut::<voltra_scene::Trail2D>(entity) {
        ui.separator();
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
    claim = claim.or(skinned::show(
        ui,
        frame.world,
        skinned::Context {
            entity,
            paint: &mut editor.weight_paint,
            textures: frame.textures,
            silhouettes: frame.silhouettes.as_deref_mut(),
            device: frame.device,
            queue: frame.queue,
        },
    ));
    let asked = skeleton::show(ui, frame.world, entity);
    claim = claim.or(asked.claim);
    if asked.set_rest {
        // A rest copies the transform: the authored one, never
        // a recorded pose.
        editor
            .timeline
            .end_record(frame.world, frame.animation_clips);
        let pose = frame.world.get::<Transform>(entity).copied();
        if let (Some(pose), Some(bone)) =
            (pose, frame.world.get_mut::<voltra_scene::Bone2D>(entity))
        {
            bone.rest = pose;
            claim = Some("Set bone rest pose");
        }
    }
    if asked.child_bone {
        crate::spawn::record(editor, frame, "Add child bone", |frame| {
            crate::spawn::child_bone(frame, entity)
        });
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
    if frame.world.get::<Camera>(entity).is_some() {
        claim = claim.or(camera_view::show(ui, frame.world, entity));
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
