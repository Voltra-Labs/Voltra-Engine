//! Handles resolved for a whole world: what a world populated before the
//! stores existed, or loaded from a file, has never met.

use voltra_assets::{Atlases, Clips, Textures};
use voltra_ecs::World;
use voltra_render::wgpu;
use voltra_scene::{AudioSource, Sprite2D, Tilemap};

/// Re-resolves every [`Sprite2D`]'s texture handle from its path.
///
/// Visible to `app` as well as to [`UiFrame`](super::UiFrame) because the
/// world can arrive already populated: [`App::world`](crate::app::App::world)
/// is public and documented to be filled before `run`, and those sprites
/// have never met a [`Textures`].
pub(in crate::app) fn resolve_world_textures(
    world: &mut World,
    textures: &mut Textures,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) {
    // Decoded across the cores first, then resolved below from the cache:
    // a scene opening with dozens of sheets decodes them at once rather than
    // one after another. A pool for the occasion — Open, undo, start — so no
    // thread waits idle between scenes.
    let paths: Vec<_> = world
        .query::<Sprite2D>()
        .flat_map(|(_, sprite)| [sprite.texture.clone(), sprite.normal_map.clone()])
        .flatten()
        .chain(
            world
                .query::<Tilemap>()
                .flat_map(|(_, map)| [map.texture.clone(), map.normal_map.clone()])
                .flatten(),
        )
        .chain(
            world
                .query::<voltra_scene::ParticleEmitter>()
                .filter_map(|(_, emitter)| emitter.texture.clone()),
        )
        .chain(
            world
                .query::<voltra_scene::PointLight2D>()
                .filter_map(|(_, light)| light.cookie.clone()),
        )
        .filter(|path| textures.by_path_handle(path).is_none())
        .collect();
    if paths.len() > 1 {
        let jobs = voltra_jobs::JobPool::with_available_parallelism();
        textures.preload(device, queue, paths, &jobs);
    }
    for (_, sprite) in world.query_mut::<Sprite2D>() {
        let path = sprite.texture.clone();
        sprite.set_texture(path, textures, device, queue);
        let normal = sprite.normal_map.clone();
        sprite.set_normal_map(normal, textures, device, queue);
    }
    for (_, map) in world.query_mut::<Tilemap>() {
        let path = map.texture.clone();
        map.set_texture(path, textures, device, queue);
        let normal = map.normal_map.clone();
        map.set_normal_map(normal, textures, device, queue);
    }
    for (_, emitter) in world.query_mut::<voltra_scene::ParticleEmitter>() {
        let path = emitter.texture.clone();
        emitter.set_texture(path, textures, device, queue);
    }
    for (_, light) in world.query_mut::<voltra_scene::PointLight2D>() {
        let path = light.cookie.clone();
        light.set_cookie(path, textures, device, queue);
    }
    for (_, line) in world.query_mut::<voltra_scene::Line2D>() {
        let path = line.texture.clone();
        line.set_texture(path, textures, device, queue);
    }
    for (_, polygon) in world.query_mut::<voltra_scene::Polygon2D>() {
        let path = polygon.texture.clone();
        polygon.set_texture(path, textures, device, queue);
    }
    for (_, trail) in world.query_mut::<voltra_scene::Trail2D>() {
        let path = trail.texture.clone();
        trail.set_texture(path, textures, device, queue);
    }
    for (_, skinned) in world.query_mut::<voltra_scene::SkinnedSprite>() {
        let path = skinned.texture.clone();
        skinned.set_texture(path, textures, device, queue);
    }
}

/// Re-resolves every [`AudioSource`]'s clip handle from its path.
///
/// The audible member of the same family as [`resolve_world_textures`] and
/// [`resolve_world_atlases`], and needed for the same two reasons: a world can
/// arrive already populated, and Open replaces every entity in it.
pub(in crate::app) fn resolve_world_clips(world: &mut World, clips: &mut Clips) {
    for (_, source) in world.query_mut::<AudioSource>() {
        let path = source.clip.clone();
        source.set_clip(path, clips);
    }
}

/// Re-resolves every [`Sprite2D`]'s, [`Tilemap`]'s and particle flipbook's
/// atlas handle from its path.
///
/// The slicing half of [`resolve_world_textures`], and separate from it
/// because it needs no device: a world can be re-sliced headless, and only the
/// pixels need a GPU.
pub(in crate::app) fn resolve_world_atlases(world: &mut World, atlases: &mut Atlases) {
    for (_, sprite) in world.query_mut::<Sprite2D>() {
        let path = sprite.atlas.clone();
        sprite.set_atlas(path, atlases);
    }
    for (_, map) in world.query_mut::<Tilemap>() {
        let path = map.atlas.clone();
        map.set_atlas(path, atlases);
    }
    for (_, emitter) in world.query_mut::<voltra_scene::ParticleEmitter>() {
        emitter.resolve_atlas(atlases);
    }
}
