//! Putting a new entity into the scene, as one undo entry.
//!
//! Here rather than in the panel that first needed it: the menu bar spawns from
//! a click on a button, the asset browser spawns from a texture dropped on the
//! viewport, and both have to produce the same entity and the same single
//! history entry. A second copy of "spawn, select, commit" would drift.

use voltra_assets::AssetPath;
use voltra_core::UiFrame;
use voltra_ecs::Entity;
use voltra_render::glam::Vec2;
use voltra_scene::{Collider, Name, RigidBody, SceneId, Sprite, Transform};

use crate::editor::Editor;
use crate::undo::{selected_id, SceneView};

/// How big a freshly spawned sprite is, in world units.
///
/// Small enough that a few fit on screen at the default zoom, and not so small
/// that the gizmo's handles cover it.
pub(crate) const DEFAULT_SCALE: f32 = 0.4;

/// Runs `spawn`, selects what it made, and records the whole thing as one entry.
///
/// A spawn has no id to name when the edit opens, so the id can only be handed
/// to the commit — which is what `commit_including` is for, and what makes the
/// undo of a spawn a despawn.
pub(crate) fn record(
    editor: &mut Editor,
    frame: &mut UiFrame<'_>,
    label: &'static str,
    spawn: impl FnOnce(&mut UiFrame<'_>) -> Entity,
) {
    let view = SceneView {
        world: frame.world,
        registry: &editor.registry,
        selected: selected_id(frame.world, editor.selected),
    };
    editor.history.begin(label, view, []);

    let entity = spawn(frame);
    editor.selected = Some(entity);

    let id = frame.world.get::<SceneId>(entity).copied();
    let view = SceneView {
        world: frame.world,
        registry: &editor.registry,
        selected: id,
    };
    editor.history.commit_including(view, id);
}

/// Drops a white unit sprite at `at`, ready to be moved.
pub(crate) fn sprite(frame: &mut UiFrame<'_>, name: &str, at: Vec2) -> Entity {
    let entity = frame.world.spawn();
    frame.world.insert(entity, SceneId::new());
    // Named on spawn, the way Unity names a new object after what made it. The
    // hierarchy is unreadable when every row says `Entity 7`.
    frame.world.insert(entity, Name::new(name));
    frame.world.insert(
        entity,
        Transform::from_translation(at).with_scale(Vec2::splat(DEFAULT_SCALE)),
    );
    frame.world.insert(entity, Sprite::default());
    entity
}

/// Drops a sprite at `at` already showing `path`, named after the file.
///
/// Named after the asset because that is what Unity, Unreal and Godot all do
/// when one is dragged into a scene: the object the user is thinking about is
/// "the checker", not "Sprite (4)".
///
/// Sized to the texture's aspect ratio, longest side [`DEFAULT_SCALE`]. The
/// quad is square, so a 16:9 texture dropped onto a square sprite arrives
/// stretched, and the first thing anyone would do is fix it by hand.
pub(crate) fn textured_sprite(frame: &mut UiFrame<'_>, at: Vec2, path: &AssetPath) -> Entity {
    let entity = sprite(frame, &name_of(path), at);

    let Some(component) = frame.world.get_mut::<Sprite>(entity) else {
        return entity;
    };
    component.set_texture(
        Some(path.clone()),
        frame.textures,
        frame.device,
        frame.queue,
    );
    let Some(handle) = component.texture_handle else {
        return entity;
    };

    let texture = frame.textures.get(handle);
    let (width, height) = (texture.width() as f32, texture.height() as f32);
    // A zero-sided texture cannot happen through `Texture::from_rgba8`, but a
    // division that produces `inf` would put the sprite's scale beyond repair,
    // and the guard costs one comparison at spawn.
    let longest = width.max(height);
    if longest <= 0.0 {
        return entity;
    }
    if let Some(transform) = frame.world.get_mut::<Transform>(entity) {
        transform.scale = Vec2::new(width / longest, height / longest) * DEFAULT_SCALE;
    }
    entity
}

/// What to call an entity showing `path`: the file name without its extension.
fn name_of(path: &AssetPath) -> String {
    let file = path.as_str().rsplit('/').next().unwrap_or(path.as_str());
    match file.rsplit_once('.') {
        // `rsplit_once` on a dotfile would leave an empty stem, but a dotfile
        // is never listed as an asset, so the only way here is a real name.
        Some((stem, _)) if !stem.is_empty() => stem.to_owned(),
        _ => file.to_owned(),
    }
}

/// Drops a sprite with a body and a collider that match its outline.
///
/// The collider is [`Sprite::HALF_EXTENT`] rather than a literal, so the green
/// outline lands exactly on the sprite's edge. A collider that disagrees with
/// the picture by a hair is a bug that reads as a physics bug.
pub(crate) fn body(
    frame: &mut UiFrame<'_>,
    name: &str,
    at: Vec2,
    scale: Vec2,
    rotation: f32,
    body: RigidBody,
    color: [f32; 4],
) -> Entity {
    let entity = frame.world.spawn();
    frame.world.insert(entity, SceneId::new());
    frame.world.insert(entity, Name::new(name));
    frame.world.insert(
        entity,
        Transform::from_translation(at)
            .with_scale(scale)
            .with_rotation(rotation),
    );
    frame.world.insert(entity, Sprite::new(color));
    frame.world.insert(entity, body);
    frame.world.insert(
        entity,
        Collider::Box {
            half_extents: Vec2::splat(Sprite::HALF_EXTENT),
        },
    );
    entity
}
