//! Prefab sync: every instance in a level brought in step with its file,
//! as a scene load, a scene change and a hot reload do.

use voltra_assets::AssetPath;
use voltra_ecs::World;
use voltra_render::glam::Vec2;
use voltra_scene::hierarchy::set_parent;
use voltra_scene::{prefab, ComponentRegistry, Name, Prefabs, SceneId, Sprite, Transform};
use voltra_testkit::Scratch;

use super::{Bench, Outcome};
use crate::harness::{iter, measure};

/// Instances of a two-entity coin, and plain entities beside them: a busy
/// level (docs/ARCHITECTURE.md, stage 126).
const INSTANCES: usize = 3_000;
const FILLER: usize = 20_000;

pub(super) fn benches() -> Vec<Bench> {
    vec![Bench::new(
        format!("prefab/sync-all/{INSTANCES}-instances-{FILLER}-entities"),
        20.0,
        |config| {
            let Some((_root, mut world, mut prefabs)) = level() else {
                return Outcome::Skipped("the coin prefab would not write".to_owned());
            };
            let registry = ComponentRegistry::with_defaults();
            // In step after the first: what is timed is the comparing a sync
            // of an unchanged level does, the common case.
            prefab::sync_all(&mut world, &registry, &mut prefabs);
            measure(
                config,
                1,
                iter(|| prefab::sync_all(&mut world, &registry, &mut prefabs).changed),
            )
            .into()
        },
    )]
}

/// The level, and the store its coin is read through — written to a
/// directory of its own under the system's temporary one, removed when the
/// answer's first element drops.
fn level() -> Option<(Scratch, World, Prefabs)> {
    let root = Scratch::new("voltra-bench-prefab");
    let registry = ComponentRegistry::with_defaults();
    let mut authoring = World::new();
    let coin = named(&mut authoring, "coin");
    authoring.insert(coin, Sprite::new([1.0, 0.8, 0.1, 1.0]));
    let glow = named(&mut authoring, "glow");
    authoring.insert(glow, Sprite::default().with_sort_order(1));
    set_parent(&mut authoring, glow, coin).ok()?;
    let path = AssetPath::new("coin.prefab.ron").ok()?;
    let mut prefabs = Prefabs::new(&root);
    prefab::write(&authoring, &registry, &mut prefabs, coin, &path).ok()?;

    let mut world = World::new();
    let handle = prefabs.load(&path);
    for i in 0..INSTANCES {
        let at = Transform::from_translation(Vec2::new(i as f32, 0.0));
        prefab::spawn(&mut world, &registry, prefabs.get(handle), &at).ok()?;
    }
    for _ in 0..FILLER {
        named(&mut world, "filler");
    }
    Some((root, world, prefabs))
}

fn named(world: &mut World, name: &str) -> voltra_ecs::Entity {
    let entity = world.spawn();
    world.insert(entity, SceneId::new());
    world.insert(entity, Name::new(name));
    world.insert(entity, Transform::default());
    entity
}
