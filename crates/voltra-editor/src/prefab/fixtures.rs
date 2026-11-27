//! A base prefab and a variant of it on disk, for the editor's tests of
//! what acts on instances of both.

use std::path::Path;

use voltra_assets::AssetPath;
use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;
use voltra_scene::format::ComponentRegistry;
use voltra_scene::{
    hierarchy, prefab, Name, PointLight2D, PrefabPart, Prefabs, SceneId, Sprite, Transform,
};

/// The base, `coin.prefab.ron`.
pub(crate) const BASE: &str = "coin.prefab.ron";
/// Its variant, `coin Variant.prefab.ron`.
pub(crate) const VARIANT: &str = "coin Variant.prefab.ron";

pub(crate) fn path(raw: &str) -> AssetPath {
    AssetPath::new(raw).expect("a path")
}

/// The base coin, a sprite of `color` with a light of `intensity` and a
/// glow under it, written through the store.
pub(crate) fn write_coin(
    registry: &ComponentRegistry,
    prefabs: &mut Prefabs,
    color: [f32; 4],
    intensity: f32,
) {
    let mut world = World::new();
    let coin = world.spawn();
    world.insert(
        coin,
        SceneId(
            "019fe500-0000-7000-8000-0000000000c1"
                .parse()
                .expect("a uuid"),
        ),
    );
    world.insert(coin, Name::new("coin"));
    world.insert(coin, Transform::default());
    world.insert(coin, Sprite::new(color));
    world.insert(
        coin,
        PointLight2D {
            intensity,
            ..PointLight2D::default()
        },
    );
    let glow = world.spawn();
    world.insert(
        glow,
        SceneId(
            "019fe500-0000-7000-8000-0000000000c2"
                .parse()
                .expect("a uuid"),
        ),
    );
    world.insert(glow, Name::new("glow"));
    world.insert(glow, Transform::from_translation(Vec2::new(0.0, 0.2)));
    world.insert(glow, Sprite::default().with_sort_order(1));
    hierarchy::set_parent(&mut world, glow, coin).expect("parents");
    prefab::write(&world, registry, prefabs, coin, &path(BASE)).expect("written");
}

/// Opens the variant as prefab mode does, gives its root's light
/// `intensity`, records the override and saves it.
pub(crate) fn override_variant_light(
    root: &Path,
    registry: &ComponentRegistry,
    prefabs: &mut Prefabs,
    file: &AssetPath,
    intensity: f32,
) {
    let full = root.join(file.as_str());
    let mut world = World::new();
    voltra_scene::format::load(&full, registry, &mut world).expect("reads");
    prefab::sync_all(&mut world, registry, prefabs);
    let top = world
        .query::<SceneId>()
        .map(|(e, _)| e)
        .find(|&e| hierarchy::parent_of(&world, e).is_none())
        .expect("a root");
    world
        .get_mut::<PointLight2D>(top)
        .expect("a light")
        .intensity = intensity;
    prefab::refresh_overrides(&mut world, registry, prefabs, top).expect("compared");
    voltra_scene::format::save(&world, registry, &full).expect("saved");
    prefabs.reload(file);
}

pub(crate) fn spawned(
    world: &mut World,
    registry: &ComponentRegistry,
    prefabs: &mut Prefabs,
    file: &AssetPath,
) -> Entity {
    let handle = prefabs.load(file);
    prefab::spawn(world, registry, prefabs.get(handle), &Transform::default())
        .expect("spawns")
        .root()
        .expect("a root")
}

pub(crate) fn overridden(world: &World, entity: Entity) -> Vec<String> {
    world
        .get::<PrefabPart>(entity)
        .expect("a part")
        .overridden
        .iter()
        .cloned()
        .collect()
}

pub(crate) fn intensity(world: &World, entity: Entity) -> f32 {
    world
        .get::<PointLight2D>(entity)
        .expect("a light")
        .intensity
}

pub(crate) fn color(world: &World, entity: Entity) -> [f32; 4] {
    world.get::<Sprite>(entity).expect("a sprite").color
}

/// A store at `root` holding the base (red, its light at 1) and the
/// variant, whose root's light is its own at 3.
pub(crate) fn base_and_variant(root: &Path, registry: &ComponentRegistry) -> Prefabs {
    let mut prefabs = Prefabs::new(root);
    write_coin(registry, &mut prefabs, [1.0, 0.0, 0.0, 1.0], 1.0);
    prefab::variant(registry, &mut prefabs, &path(BASE), &path(VARIANT)).expect("a variant");
    override_variant_light(root, registry, &mut prefabs, &path(VARIANT), 3.0);
    prefabs
}
