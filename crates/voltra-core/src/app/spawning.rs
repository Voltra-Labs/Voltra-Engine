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
    prefab, AudioSource, Instance, Prefab, Prefabs, SceneError, Sprite, Text, Tilemap, Transform,
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
        at: &Transform,
    ) -> Result<Instance, SceneError> {
        let Some(prefab) = self.prefabs.try_get(prefab) else {
            log::warn!("spawning a prefab handle this store never issued; nothing spawned");
            return Ok(Instance::default());
        };
        let instance = prefab::spawn(self.world, self.registry, prefab, at)?;
        self.spawned.extend_from_slice(&instance.entities);
        Ok(instance)
    }
}

/// The prefab store, made on first use.
///
/// Lazy rather than built at `resumed` like the stores that need a device: a
/// prefab is a file and nothing else, so a headless run — every test, a
/// server — can spawn one. The root is the same one `resumed` resolves.
pub(super) fn prefab_store<'a>(
    prefabs: &'a mut Option<Prefabs>,
    asset_root: &Option<PathBuf>,
) -> &'a mut Prefabs {
    prefabs.get_or_insert_with(|| {
        Prefabs::new(
            asset_root
                .clone()
                .unwrap_or_else(voltra_assets::default_root),
        )
    })
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
    if let Some(sprite) = world.get_mut::<Sprite>(entity) {
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

    fn resolve_entity(&mut self, entity: Entity) {
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
        prefab_store(&mut self.prefabs, &self.asset_root)
    }

    /// Spawns a copy of the prefab at `path` while the world is being set up.
    ///
    /// The setup-time twin of [`Tick::spawn`], for a level laid out in code
    /// before [`App::run`]. Nothing is resolved here — there is no device yet
    /// — and nothing needs to be: the loop resolves the whole world when it
    /// resumes, which is what it already does for every entity a game
    /// inserted by hand.
    pub fn spawn(&mut self, path: &AssetPath, at: &Transform) -> Result<Instance, SceneError> {
        let prefabs = prefab_store(&mut self.prefabs, &self.asset_root);
        let handle = prefabs.load(path);
        let instance = prefab::spawn(&mut self.world, &self.registry, prefabs.get(handle), at)?;
        // Queued as well, for a call made after the loop is already running —
        // a UI callback, say — where nothing else would resolve them.
        self.spawned.extend_from_slice(&instance.entities);
        Ok(instance)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;
    use voltra_render::glam::Vec2;
    use voltra_scene::{ComponentRegistry, Name, SceneId};
    use voltra_testkit::scratch_root;

    /// An asset root holding `coin.ron`: one named, positioned entity.
    fn root_with_a_coin() -> PathBuf {
        let root = scratch_root();
        let mut world = voltra_ecs::World::new();
        let coin = world.spawn();
        world.insert(coin, SceneId::new());
        world.insert(coin, Name::new("coin"));
        world.insert(coin, Transform::default());
        voltra_scene::format::save(
            &world,
            &ComponentRegistry::with_defaults(),
            &root.join("coin.ron"),
        )
        .expect("the fixture writes");
        root
    }

    fn coin() -> AssetPath {
        AssetPath::new("coin.ron").expect("valid")
    }

    #[test]
    fn a_tick_spawns_a_prefab_where_it_is_told() {
        let spawned = Rc::new(RefCell::new(Vec::new()));
        let seen = spawned.clone();
        let mut app = App::default()
            .with_asset_root(root_with_a_coin())
            .with_simulation()
            .with_update(move |tick| {
                let coin = tick.prefab(&coin());
                let instance = tick
                    .spawn(coin, &Transform::from_translation(Vec2::new(3.0, 4.0)))
                    .expect("the prefab reads");
                seen.borrow_mut().extend(instance.roots);
            });

        app.advance(1.0 / 60.0);

        let roots = spawned.borrow();
        assert_eq!(roots.len(), 1);
        assert_eq!(
            app.world.get::<Transform>(roots[0]).map(|t| t.translation),
            Some(Vec2::new(3.0, 4.0))
        );
        assert!(app.spawned.is_empty(), "the queue was resolved and emptied");
    }

    #[test]
    fn a_tick_that_spawns_every_frame_makes_one_per_frame() {
        let mut app = App::default()
            .with_asset_root(root_with_a_coin())
            .with_simulation()
            .with_update(|tick| {
                let coin = tick.prefab(&coin());
                tick.spawn(coin, &Transform::default()).expect("reads");
            });

        for _ in 0..3 {
            app.advance(1.0 / 60.0);
        }

        assert_eq!(app.world.component_count::<Name>(), 3);
    }

    #[test]
    fn a_forged_handle_spawns_nothing_rather_than_panicking() {
        let mut app = App::default()
            .with_asset_root(scratch_root())
            .with_simulation()
            .with_update(|tick| {
                let instance = tick
                    .spawn(Handle::forge(40, 0), &Transform::default())
                    .expect("not an error");
                assert!(instance.entities.is_empty());
            });

        app.advance(1.0 / 60.0);

        assert_eq!(app.world.entity_count(), 0);
    }

    #[test]
    fn an_entity_despawned_by_the_tick_that_made_it_is_skipped() {
        let mut app = App::default()
            .with_asset_root(root_with_a_coin())
            .with_simulation()
            .with_update(|tick| {
                let coin = tick.prefab(&coin());
                let instance = tick.spawn(coin, &Transform::default()).expect("reads");
                for entity in instance.entities {
                    tick.world.despawn(entity);
                }
            });

        app.advance(1.0 / 60.0);

        assert_eq!(app.world.entity_count(), 0);
    }

    #[test]
    fn a_game_can_load_its_prefabs_before_the_first_tick() {
        let mut app = App::default().with_asset_root(root_with_a_coin());

        let handle = app.prefabs().load(&coin());

        assert_eq!(app.prefabs().get(handle).len(), 1);
    }
}
