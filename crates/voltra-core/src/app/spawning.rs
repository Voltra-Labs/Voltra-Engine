//! Prefabs from a game's tick, and the assets the spawned entities name.
//!
//! The tick spawns; the loop resolves. A spawned sprite names its texture by
//! path, and turning that into a handle needs the GPU — which a tick is not
//! handed, for the reason [`Tick`] gives for not handing it `&mut App`. So a
//! spawn queues what it made, and the loop resolves the queue right after the
//! tick returns, before anything draws or plays it. The entity is complete by
//! the time anything but the tick that made it can see it.

use std::path::PathBuf;

use voltra_assets::{AssetPath, Atlases, Clips, Fonts, Handle, Textures};
use voltra_ecs::{Entity, World};
use voltra_render::wgpu;
use voltra_scene::{
    prefab, AudioSource, ComponentRegistry, Instance, Prefab, Prefabs, SceneError, Sprite2D, Text,
    Tilemap, Transform2D,
};

use crate::app::tick::Tick;
use crate::app::App;

impl Tick<'_> {
    /// The handle for the prefab at `path`, reading it the first time.
    ///
    /// Reads the disk once per path, cached after; a game that wants the read
    /// out of its first frame of play calls this from its setup instead.
    pub fn prefab(&mut self, path: &AssetPath) -> Handle<Prefab> {
        self.prefabs.load(path)
    }

    /// Spawns a copy of `prefab` with its roots placed by `at`.
    ///
    /// See [`voltra_scene::prefab::spawn`] for what a copy means. The
    /// entities are in the world when this returns and can be changed
    /// straight away; their textures, slicings and clips are resolved by the
    /// loop as soon as the tick returns.
    ///
    /// A handle this store never issued spawns nothing, the way a handle that
    /// resolves to nothing plays silence: a game holding one must not crash.
    pub fn spawn(
        &mut self,
        prefab: Handle<Prefab>,
        at: &Transform2D,
    ) -> Result<Instance, SceneError> {
        let Some(prefab) = self.prefabs.try_get(prefab) else {
            log::warn!("spawning a prefab handle this store never issued; nothing spawned");
            return Ok(Instance::default());
        };
        let instance = prefab::spawn(self.world, self.registry, prefab, at)?;
        self.spawned.extend_from_slice(&instance.entities);
        Ok(instance)
    }

    /// A copy of `entity` and everything under it, as the editor's
    /// duplicate makes one: fresh ids, links inside it remapped, the copy a
    /// root (parent it where it belongs) and linked to no prefab — every
    /// component the registry knows, through the same cut a prefab file is
    /// made with. What a screen built from a template row does once per
    /// row.
    pub fn duplicate(&mut self, entity: Entity) -> Result<Instance, SceneError> {
        let file = prefab::fragment(self.world, self.registry, entity)?;
        let copy = Prefab::new(file)?;
        let instance = prefab::spawn(self.world, self.registry, &copy, &Transform2D::default())?;
        for &made in &instance.entities {
            self.world.remove::<voltra_scene::PrefabInstance>(made);
            self.world.remove::<voltra_scene::PrefabPart>(made);
        }
        self.spawned.extend_from_slice(&instance.entities);
        Ok(instance)
    }
}

/// The prefab store, made on first use.
///
/// Lazy rather than built at `resumed` like the stores that need a device: a
/// prefab is a file and nothing else, so a headless run — every test, a
/// server — can spawn one. The root is the same one `resumed` resolves.
///
/// It reads the instances a prefab holds (a variant's base, a nested
/// prefab) through the app's `registry`, so a game's own components follow
/// too; a registry swapped since (the editor loading a game) is taken up
/// here, and the prefabs holding instances read again through it.
pub(super) fn prefab_store<'a>(
    prefabs: &'a mut Option<Prefabs>,
    asset_root: &Option<PathBuf>,
    registry: &ComponentRegistry,
) -> &'a mut Prefabs {
    let store = prefabs.get_or_insert_with(|| {
        Prefabs::with_registry(
            asset_root
                .clone()
                .unwrap_or_else(voltra_assets::default_root),
            registry.clone(),
        )
    });
    store.use_registry(registry);
    store
}

/// Resolves the asset handles one entity's components name, against
/// whichever stores exist.
///
/// The one copy of "what a freshly made entity needs looked up", shared by
/// the loop's queue and by the editor's drop of a prefab: a second would
/// drift the day a component gains a path. The slicing goes before the
/// texture, because a sprite's geometry reads both.
pub(super) fn resolve_entity(
    world: &mut World,
    entity: Entity,
    gpu: Option<(&mut Textures, &wgpu::Device, &wgpu::Queue)>,
    atlases: Option<&mut Atlases>,
    fonts: Option<&mut Fonts>,
    clips: Option<&mut Clips>,
) {
    let mut atlases = atlases;
    let mut gpu = gpu;
    if let Some(sprite) = world.get_mut::<Sprite2D>(entity) {
        if let Some(atlases) = atlases.as_deref_mut() {
            let path = sprite.atlas.clone();
            sprite.set_atlas(path, atlases);
        }
        if let Some((textures, device, queue)) = gpu.as_mut() {
            let path = sprite.texture.clone();
            sprite.set_texture(path, textures, device, queue);
            let normal = sprite.normal_map.clone();
            sprite.set_normal_map(normal, textures, device, queue);
        }
    }
    if let Some(map) = world.get_mut::<Tilemap>(entity) {
        if let Some(atlases) = atlases.as_deref_mut() {
            let path = map.atlas.clone();
            map.set_atlas(path, atlases);
        }
        if let Some((textures, device, queue)) = gpu.as_mut() {
            let path = map.texture.clone();
            map.set_texture(path, textures, device, queue);
            let normal = map.normal_map.clone();
            map.set_normal_map(normal, textures, device, queue);
        }
    }
    if let (Some(emitter), Some(atlases)) = (
        world.get_mut::<voltra_scene::ParticleEmitter>(entity),
        atlases,
    ) {
        emitter.resolve_atlas(atlases);
    }
    if let (Some(emitter), Some((textures, device, queue))) = (
        world.get_mut::<voltra_scene::ParticleEmitter>(entity),
        gpu.as_mut(),
    ) {
        let path = emitter.texture.clone();
        emitter.set_texture(path, textures, device, queue);
    }
    if let (Some(light), Some((textures, device, queue))) = (
        world.get_mut::<voltra_scene::PointLight2D>(entity),
        gpu.as_mut(),
    ) {
        let path = light.cookie.clone();
        light.set_cookie(path, textures, device, queue);
    }
    if let (Some(line), Some((textures, device, queue))) =
        (world.get_mut::<voltra_scene::Line2D>(entity), gpu.as_mut())
    {
        let path = line.texture.clone();
        line.set_texture(path, textures, device, queue);
    }
    if let (Some(polygon), Some((textures, device, queue))) = (
        world.get_mut::<voltra_scene::Polygon2D>(entity),
        gpu.as_mut(),
    ) {
        let path = polygon.texture.clone();
        polygon.set_texture(path, textures, device, queue);
    }
    if let (Some(trail), Some((textures, device, queue))) =
        (world.get_mut::<voltra_scene::Trail2D>(entity), gpu.as_mut())
    {
        let path = trail.texture.clone();
        trail.set_texture(path, textures, device, queue);
    }
    if let (Some(skinned), Some((textures, device, queue))) = (
        world.get_mut::<voltra_scene::SkinnedSprite>(entity),
        gpu.as_mut(),
    ) {
        let path = skinned.texture.clone();
        skinned.set_texture(path, textures, device, queue);
    }
    if let (Some(text), Some(fonts)) = (world.get_mut::<Text>(entity), fonts) {
        let path = text.font.clone();
        text.set_font(path, fonts);
    }
    if let (Some(source), Some(clips)) = (world.get_mut::<AudioSource>(entity), clips) {
        let path = source.clip.clone();
        source.set_clip(path, clips);
    }
}

impl App {
    /// Resolves the asset handles of everything a tick spawned since the last
    /// call.
    ///
    /// Per entity, not the whole world: a game spawning bullets every frame
    /// must not pay for re-resolving the level each time. An entity despawned
    /// by the same tick that made it is skipped. Before `resumed` there are no
    /// stores to resolve against, and the queue is simply dropped — `resumed`
    /// resolves the whole world, these entities included.
    pub(super) fn resolve_spawned(&mut self) {
        if self.spawned.is_empty() {
            return;
        }
        let spawned = std::mem::take(&mut self.spawned);

        for entity in spawned {
            if !self.world.is_alive(entity) {
                continue;
            }
            self.resolve_entity(entity);
        }
    }

    pub(super) fn resolve_entity(&mut self, entity: Entity) {
        let context = self.renderer.as_ref().map(|renderer| renderer.context());
        let gpu = match (self.textures.as_mut(), context) {
            (Some(textures), Some(context)) => Some((textures, context.device(), context.queue())),
            _ => None,
        };
        resolve_entity(
            &mut self.world,
            entity,
            gpu,
            self.atlases.as_mut(),
            self.fonts.as_mut(),
            self.clips.as_mut(),
        );
    }

    /// The prefab store, for a game loading its prefabs before the first tick.
    pub fn prefabs(&mut self) -> &mut Prefabs {
        prefab_store(&mut self.prefabs, &self.asset_root, &self.registry)
    }

    /// Spawns a copy of the prefab at `path` while the world is being set up.
    ///
    /// The setup-time twin of [`Tick::spawn`], for a level laid out in code
    /// before [`App::run`]. Nothing is resolved here — there is no device yet
    /// — and nothing needs to be: the loop resolves the whole world when it
    /// resumes, which is what it already does for every entity a game
    /// inserted by hand.
    pub fn spawn(&mut self, path: &AssetPath, at: &Transform2D) -> Result<Instance, SceneError> {
        let prefabs = prefab_store(&mut self.prefabs, &self.asset_root, &self.registry);
        let handle = prefabs.load(path);
        let instance = prefab::spawn(&mut self.world, &self.registry, prefabs.get(handle), at)?;
        // Queued as well, for a call made after the loop is already running —
        // a UI callback, say — where nothing else would resolve them.
        self.spawned.extend_from_slice(&instance.entities);
        Ok(instance)
    }
}

#[cfg(test)]
mod tests;
