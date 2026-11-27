//! A base prefab changed on disk reaching an instance of its variant in a
//! running game, as the watcher's reload would bring it.

use voltra_assets::AssetPath;
use voltra_ecs::World;
use voltra_scene::{prefab, ComponentRegistry, Name, SceneId, Sprite, Transform};
use voltra_testkit::scratch_root;

use crate::app::App;

const FRAME: f32 = 1.0 / 60.0;

fn path(raw: &str) -> AssetPath {
    AssetPath::new(raw).expect("a valid asset path")
}

/// Writes a one-entity coin of `color` to `root/coin.prefab.ron`, under
/// the same record id every time.
fn write_coin(root: &std::path::Path, color: [f32; 4]) {
    let mut world = World::new();
    let coin = world.spawn();
    let id = "019fe500-0000-7000-8000-0000000000c1"
        .parse()
        .expect("a uuid");
    world.insert(coin, SceneId(id));
    world.insert(coin, Name::new("coin"));
    world.insert(coin, Transform::default());
    world.insert(coin, Sprite::new(color));
    let mut prefabs = voltra_scene::Prefabs::new(root);
    let file = prefab::fragment(
        &world,
        &ComponentRegistry::with_defaults(),
        &mut prefabs,
        coin,
    )
    .expect("cut");
    voltra_scene::format::write(&file, &root.join("coin.prefab.ron")).expect("written");
}

#[test]
fn a_base_changed_on_disk_reaches_an_instance_of_its_variant() {
    let root = scratch_root();
    write_coin(&root, [1.0, 0.0, 0.0, 1.0]);
    let mut app = App::default().with_asset_root(&root).with_simulation();
    let registry = ComponentRegistry::with_defaults();
    let base = path("coin.prefab.ron");
    let variant = path("coin Variant.prefab.ron");
    prefab::variant(&registry, app.prefabs(), &base, &variant).expect("written");
    let coin = app
        .spawn(&variant, &Transform::default())
        .expect("spawns")
        .root()
        .expect("a root");
    app.advance(FRAME);

    write_coin(&root, [0.0, 1.0, 0.0, 1.0]);
    assert!(app.reload_prefab(&base));

    let sprite = app.world.get::<Sprite>(coin).expect("a sprite");
    assert_eq!(sprite.color, [0.0, 1.0, 0.0, 1.0]);
}
