//! A networked session's levels: both ends hold each level entity under the
//! file's id, so a client's copy — the sprite the level gave it — is the
//! server's, not swept and sent back bare. For the opening level a game's
//! binary asks for (`App::load_scene`), for a scene change over frames on
//! both ends in either order (`change_scene`), and for a level reloaded
//! over a persistent entity it also holds.

use std::cell::Cell;
use std::path::Path;
use std::rc::Rc;
use std::time::Duration;

use voltra_assets::AssetPath;
use voltra_core::net::{MemoryNetwork, NetConfig};
use voltra_core::replication::ReplicationConfig;
use voltra_core::{App, Headless};
use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;
use voltra_scene::{ComponentRegistry, Name, Persistent, Replicated, SceneId, Sprite, Transform};
use voltra_testkit::scratch_root;

const FRAME: f32 = 1.0 / 60.0;

/// Writes `scenes/<name>.ron`: a wall, and the coin `coin` names — a
/// replicated entity that is no prefab instance, with a sprite of its own
/// and a glow under it that only draws.
fn write_level(root: &Path, name: &str, coin: Option<SceneId>) {
    let mut level = World::new();
    let wall = level.spawn();
    level.insert(wall, SceneId::new());
    level.insert(wall, Name::new("Wall"));
    level.insert(wall, Transform::default());
    if let Some(id) = coin {
        let coin = level.spawn();
        level.insert(coin, id);
        level.insert(coin, Name::new("Coin"));
        level.insert(coin, Transform::from_translation(Vec2::new(1.0, 2.0)));
        level.insert(coin, Sprite::default().with_sort_order(7));
        level.insert(coin, Replicated::default());
        let glow = level.spawn();
        level.insert(glow, SceneId::new());
        level.insert(glow, Name::new("Glow"));
        level.insert(glow, Transform::default());
        voltra_scene::hierarchy::set_parent(&mut level, glow, coin).expect("parents");
    }
    save(root, name, &level);
}

fn save(root: &Path, name: &str, level: &World) {
    let file = voltra_scene::format::to_scene_file(level, &ComponentRegistry::with_defaults())
        .expect("saves");
    std::fs::create_dir_all(root.join("scenes")).expect("a folder");
    std::fs::write(
        root.join(format!("scenes/{name}.ron")),
        ron::to_string(&file).expect("serialises"),
    )
    .expect("writes");
}

fn scene(name: &str) -> AssetPath {
    AssetPath::new(&format!("scenes/{name}.ron")).expect("a path")
}

fn step(server: &mut Headless, client: &mut Headless, frames: u32) {
    for _ in 0..frames {
        server.step(1, FRAME);
        client.step(1, FRAME);
        // The rest of a frame, in which the load pool reads.
        std::thread::sleep(Duration::from_micros(500));
    }
}

/// The client's replicated entities.
fn replicated(client: &Headless) -> Vec<Entity> {
    client
        .world()
        .query::<Replicated>()
        .map(|(entity, _)| entity)
        .collect()
}

fn named(world: &World, name: &str) -> usize {
    world
        .query::<Name>()
        .filter(|(_, n)| n.as_str() == name)
        .count()
}

/// The client's one coin is the level's: under the file's id, its sprite
/// and glow its own, and moved where the server moves it.
fn assert_dressed(server: &mut Headless, client: &mut Headless, coin_id: SceneId) {
    let coins = replicated(client);
    assert_eq!(coins.len(), 1, "one coin");
    let world = client.world();
    assert_eq!(world.get::<SceneId>(coins[0]), Some(&coin_id));
    assert_eq!(
        world.get::<Sprite>(coins[0]).map(|s| s.sort_order),
        Some(7),
        "the level's own sprite kept"
    );
    assert_eq!(named(world, "Glow"), 1, "its glow, once");
    let ours: Vec<Entity> = server
        .world()
        .query::<Replicated>()
        .map(|(e, _)| e)
        .collect();
    for entity in ours {
        if let Some(t) = server.world_mut().get_mut::<Transform>(entity) {
            t.translation = Vec2::new(5.0, 6.0);
        }
    }
    step(server, client, 60);
    let coins = replicated(client);
    assert_eq!(
        client
            .world()
            .get::<Transform>(coins[0])
            .map(|t| t.translation),
        Some(Vec2::new(5.0, 6.0)),
        "the server's copy"
    );
}

/// A server and a client of one session, each with a tick that changes to
/// `next` (behind no screen) once its flag is set.
fn session(
    root: &Path,
    first: &str,
    next: &str,
) -> (Headless, Headless, Rc<Cell<bool>>, Rc<Cell<bool>>) {
    let network = MemoryNetwork::new();
    let transport = network.endpoint();
    let address = transport.address();
    let app = |go: Rc<Cell<bool>>| {
        let next = scene(next);
        let mut app = App::default()
            .with_asset_root(root)
            .with_replication(ReplicationConfig::default())
            .with_update(move |tick| {
                if go.replace(false) {
                    tick.change_scene(&next, None);
                }
            });
        app.load_scene(&scene(first));
        app
    };
    let (go_server, go_client) = (Rc::new(Cell::new(false)), Rc::new(Cell::new(false)));
    let mut server = Headless::new(app(go_server.clone()));
    server.net_mut().host_on(transport, NetConfig::default());
    let mut client = Headless::new(app(go_client.clone()));
    client
        .net_mut()
        .join_on(network.endpoint(), address, NetConfig::default());
    step(&mut server, &mut client, 90);
    assert!(client.net().local_id().is_some(), "joined");
    (server, client, go_server, go_client)
}

#[test]
fn a_client_s_opening_level_is_the_server_s_by_id() {
    let root = scratch_root();
    let coin_id = SceneId::new();
    write_level(&root, "level", Some(coin_id));

    let network = MemoryNetwork::new();
    let transport = network.endpoint();
    let address = transport.address();
    // A dedicated server opens its scene as authored.
    let mut server = Headless::open(
        App::default()
            .with_asset_root(&root)
            .with_replication(ReplicationConfig::default()),
        &scene("level"),
    )
    .expect("opens");
    server.net_mut().host_on(transport, NetConfig::default());

    // The client as a game's binary starts: its first scene asked for, and
    // the session joined before that frame is over.
    let mut app = App::default()
        .with_asset_root(&root)
        .with_replication(ReplicationConfig::default());
    app.load_scene(&scene("level"));
    let mut client = Headless::new(app);
    client
        .net_mut()
        .join_on(network.endpoint(), address, NetConfig::default());
    step(&mut server, &mut client, 90);
    assert!(client.net().local_id().is_some(), "joined");

    assert_dressed(&mut server, &mut client, coin_id);
    let stats = client
        .net()
        .replication()
        .and_then(|r| r.client())
        .map(|c| c.stats())
        .expect("a client");
    assert_eq!(stats.out_of_interest, 0, "nothing swept: {stats:?}");
}

#[test]
fn a_level_both_ends_change_to_is_the_server_s_copy_with_the_level_s_sprite() {
    // Together, the server first, the client first: the server's spawn of
    // the coin may come before the client has read the level, while it is
    // spawning it, or after.
    for (server_lead, client_lead) in [(0, 0), (0, 30), (30, 0)] {
        let root = scratch_root();
        let coin_id = SceneId::new();
        write_level(&root, "a", None);
        write_level(&root, "b", Some(coin_id));
        let (mut server, mut client, go_server, go_client) = session(&root, "a", "b");
        assert!(replicated(&client).is_empty(), "nothing replicated in a");

        go_server.set(true);
        step(&mut server, &mut client, server_lead);
        go_client.set(true);
        step(&mut server, &mut client, client_lead);
        step(&mut server, &mut client, 120);
        assert_dressed(&mut server, &mut client, coin_id);
    }
}

#[test]
fn a_level_reloaded_over_a_persistent_entity_it_holds_leaves_one_of_it() {
    let root = scratch_root();
    // A level whose HUD is persistent: it outlives a change, and the level
    // reloaded holds it again.
    let hud_id = SceneId::new();
    let mut level = World::new();
    let hud = level.spawn();
    level.insert(hud, hud_id);
    level.insert(hud, Name::new("Hud"));
    level.insert(hud, Persistent);
    let label = level.spawn();
    level.insert(label, SceneId::new());
    level.insert(label, Name::new("Label"));
    voltra_scene::hierarchy::set_parent(&mut level, label, hud).expect("parents");
    save(&root, "level", &level);

    let reload = Rc::new(Cell::new(0u8));
    let asked = reload.clone();
    let mut app = App::default()
        .with_asset_root(&root)
        .with_replication(ReplicationConfig::default())
        .with_update(move |tick| match asked.replace(0) {
            1 => tick.load_scene(&scene("level")),
            2 => tick.change_scene(&scene("level"), None),
            _ => {}
        });
    app.load_scene(&scene("level"));
    let mut game = Headless::new(app);
    game.step(2, FRAME);
    for (how, label) in [(1, "load_scene"), (2, "change_scene")] {
        reload.set(how);
        for _ in 0..30 {
            game.step(1, FRAME);
            std::thread::sleep(Duration::from_micros(500));
        }
        let world = game.world();
        assert_eq!(named(world, "Hud"), 1, "{label}: the persistent one alone");
        assert_eq!(named(world, "Label"), 1, "{label}: and what hangs under it");
        let holding = world
            .query::<SceneId>()
            .filter(|(_, id)| **id == hud_id)
            .count();
        assert_eq!(holding, 1, "{label}: one entity under its id");
    }
}
