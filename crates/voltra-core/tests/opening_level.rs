//! A networked build's opening level: a client that loads its first scene
//! the way a game's binary does (`App::load_scene`) and joins on its first
//! frame holds the level's replicated entities under the file's ids, as the
//! server does, so its own copy — the sprite the level gave it — is the
//! server's, not swept and sent back bare.

use std::time::Duration;

use voltra_assets::AssetPath;
use voltra_core::net::{MemoryNetwork, NetConfig};
use voltra_core::replication::ReplicationConfig;
use voltra_core::{App, Headless};
use voltra_ecs::World;
use voltra_render::glam::Vec2;
use voltra_scene::{ComponentRegistry, Name, Replicated, SceneId, Sprite, Transform};
use voltra_testkit::scratch_root;

const FRAME: f32 = 1.0 / 60.0;

#[test]
fn a_client_s_opening_level_is_the_server_s_by_id() {
    let root = scratch_root();
    let coin_id = SceneId::new();
    let mut level = World::new();
    let coin = level.spawn();
    level.insert(coin, coin_id);
    level.insert(coin, Name::new("Coin"));
    level.insert(coin, Transform::from_translation(Vec2::new(1.0, 2.0)));
    level.insert(coin, Sprite::default().with_sort_order(7));
    level.insert(coin, Replicated::default());
    let file = voltra_scene::format::to_scene_file(&level, &ComponentRegistry::with_defaults())
        .expect("saves");
    std::fs::create_dir_all(root.join("scenes")).expect("a folder");
    std::fs::write(
        root.join("scenes/level.ron"),
        ron::to_string(&file).expect("serialises"),
    )
    .expect("writes");
    let scene = AssetPath::new("scenes/level.ron").expect("a path");

    let network = MemoryNetwork::new();
    let transport = network.endpoint();
    let address = transport.address();
    // A dedicated server opens its scene as authored.
    let mut server = Headless::open(
        App::default()
            .with_asset_root(&root)
            .with_replication(ReplicationConfig::default()),
        &scene,
    )
    .expect("opens");
    server.net_mut().host_on(transport, NetConfig::default());

    // The client as a game's binary starts: its first scene asked for, and
    // the session joined before that frame is over.
    let mut app = App::default()
        .with_asset_root(&root)
        .with_replication(ReplicationConfig::default());
    app.load_scene(&scene);
    let mut client = Headless::new(app);
    client
        .net_mut()
        .join_on(network.endpoint(), address, NetConfig::default());
    for _ in 0..90 {
        server.step(1, FRAME);
        client.step(1, FRAME);
        std::thread::sleep(Duration::from_micros(200));
    }
    assert!(client.net().local_id().is_some(), "joined");

    let coins: Vec<_> = client
        .world()
        .query::<Replicated>()
        .map(|(entity, _)| entity)
        .collect();
    assert_eq!(coins.len(), 1, "one coin");
    let world = client.world();
    assert_eq!(world.get::<SceneId>(coins[0]), Some(&coin_id));
    assert_eq!(
        world.get::<Sprite>(coins[0]).map(|s| s.sort_order),
        Some(7),
        "the level's own sprite kept"
    );
    let stats = client
        .net()
        .replication()
        .and_then(|r| r.client())
        .map(|c| c.stats())
        .expect("a client");
    assert_eq!(stats.out_of_interest, 0, "nothing swept: {stats:?}");
}
