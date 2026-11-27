//! A streamed world under replication: the server keeps regions in around
//! its players and owns the replicated entities they hold; a client streams
//! the same regions for their scenery but leaves the replicated entities to
//! the server — one copy each, gone from the client when the server's
//! region unloads, though the client's own region stays.

use std::path::Path;
use std::time::Duration;

use voltra_assets::AssetPath;
use voltra_core::net::{MemoryNetwork, NetConfig};
use voltra_core::replication::ReplicationConfig;
use voltra_core::{App, Headless};
use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;
use voltra_scene::{
    ComponentRegistry, Name, Persistent, Replicated, SceneId, StreamingAnchor, Transform,
    WorldRegions,
};
use voltra_testkit::scratch_root;

const FRAME: f32 = 1.0 / 60.0;
const CELL: f32 = 16.0;

fn path(raw: &str) -> AssetPath {
    AssetPath::new(raw).expect("a valid asset path")
}

/// Three regions in a row; the middle one holds a wall and a crate the
/// server replicates.
fn write_world(root: &Path) -> AssetPath {
    let world = WorldRegions::grid(Vec2::ZERO, Vec2::splat(CELL), 3, 1, |c, _| {
        path(&format!("world/r{c}.ron"))
    });
    for (column, region) in world.regions.iter().enumerate() {
        let mut scene = World::new();
        let wall = scene.spawn();
        scene.insert(wall, SceneId::new());
        scene.insert(wall, Name::new(format!("wall {column}")));
        scene.insert(wall, Transform::from_translation(region.center()));
        if column == 1 {
            let crate_ = scene.spawn();
            scene.insert(crate_, SceneId::new());
            scene.insert(crate_, Name::new("crate"));
            scene.insert(crate_, Transform::from_translation(region.center()));
            scene.insert(crate_, Replicated::default());
        }
        let file = voltra_scene::format::to_scene_file(&scene, &ComponentRegistry::with_defaults())
            .expect("saves");
        let full = root.join(region.scene.as_str());
        std::fs::create_dir_all(full.parent().expect("a parent")).expect("dir");
        std::fs::write(full, ron::to_string(&file).expect("serialises")).expect("writes");
    }
    let mut world = world;
    world.load_radius = 2.0;
    world.unload_margin = 1.0;
    std::fs::write(
        root.join("rows.world.ron"),
        world.to_ron().expect("serialises"),
    )
    .expect("writes");
    path("rows.world.ron")
}

fn anchor(world: &mut World, at: Vec2) -> Entity {
    let entity = world.spawn();
    world.insert(entity, Transform::from_translation(at));
    world.insert(entity, StreamingAnchor::default());
    world.insert(entity, Persistent);
    entity
}

fn count_replicated(world: &World) -> usize {
    world.query::<Replicated>().count()
}

fn has_name(world: &World, wanted: &str) -> bool {
    world
        .query::<Name>()
        .any(|(_, name)| name.as_str() == wanted)
}

fn step(server: &mut Headless, client: &mut Headless, frames: u32) {
    for _ in 0..frames {
        server.step(1, FRAME);
        client.step(1, FRAME);
        // The rest of a frame, in which the load pool reads.
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn the_server_owns_a_regions_replicated_entities_and_a_client_takes_its_copy() {
    let root = scratch_root();
    let file = write_world(&root);
    let middle = Vec2::new(CELL * 1.5, CELL * 0.5);
    let network = MemoryNetwork::new();
    let server_transport = network.endpoint();
    let address = server_transport.address();
    let config = ReplicationConfig::default();

    let mut server_app = App::default()
        .with_asset_root(&root)
        .with_replication(config.clone());
    let player = anchor(&mut server_app.world, middle);
    server_app.stream_world(&file);
    let mut server = Headless::new(server_app);
    server
        .net_mut()
        .host_on(server_transport, NetConfig::default());

    // The client streams once the server's world has reached it — its
    // crate among it — so what the client's own load brings is past the
    // one-off sweep of strays a first snapshot makes, and a second crate
    // from the file would stay.
    let world_file = file.clone();
    let mut streaming = false;
    let mut client_app = App::default()
        .with_asset_root(&root)
        .with_replication(config)
        .with_update(move |tick| {
            if !streaming && tick.world.query::<Replicated>().count() > 0 {
                streaming = true;
                tick.stream_world(&world_file);
            }
        });
    anchor(&mut client_app.world, middle);
    let mut client = Headless::new(client_app);
    client
        .net_mut()
        .join_on(network.endpoint(), address, NetConfig::default());

    step(&mut server, &mut client, 150);
    assert!(client.net().local_id().is_some(), "joined");

    assert_eq!(count_replicated(server.world()), 1, "the server's crate");
    assert!(has_name(server.world(), "crate"));
    assert_eq!(
        count_replicated(client.world()),
        1,
        "one crate on the client: the server's, not a second from the file"
    );
    assert!(
        has_name(client.world(), "wall 1"),
        "the client's own scenery"
    );

    // The player walks off: the server lets the region go, and its crate
    // with it — on the client too, whose own region stays in.
    server
        .world_mut()
        .insert(player, Transform::from_translation(Vec2::new(-100.0, 0.0)));
    step(&mut server, &mut client, 120);

    assert_eq!(count_replicated(server.world()), 0);
    assert_eq!(
        count_replicated(client.world()),
        0,
        "despawned by the server"
    );
    assert!(has_name(client.world(), "wall 1"), "the scenery stays");
}

/// One region of its own, far from everything, holding a replicated crate.
fn write_lone_region(root: &Path, at: Vec2) -> AssetPath {
    let mut world = WorldRegions::grid(at, Vec2::splat(CELL), 1, 1, |_, _| path("lone/r.ron"));
    let mut scene = World::new();
    let crate_ = scene.spawn();
    scene.insert(crate_, SceneId::new());
    scene.insert(crate_, Name::new("crate"));
    scene.insert(
        crate_,
        Transform::from_translation(world.regions[0].center()),
    );
    scene.insert(crate_, Replicated::default());
    let file = voltra_scene::format::to_scene_file(&scene, &ComponentRegistry::with_defaults())
        .expect("saves");
    let full = root.join("lone/r.ron");
    std::fs::create_dir_all(full.parent().expect("a parent")).expect("dir");
    std::fs::write(full, ron::to_string(&file).expect("serialises")).expect("writes");
    world.load_radius = 2.0;
    world.unload_margin = 1.0;
    std::fs::write(
        root.join("lone.world.ron"),
        world.to_ron().expect("serialises"),
    )
    .expect("writes");
    path("lone.world.ron")
}

#[test]
fn a_region_unloaded_lets_go_of_what_was_swept_of_it_and_no_more() {
    use voltra_core::replication::{Interest, NetOwner};
    use voltra_scene::Sprite;

    let root = scratch_root();
    let far = Vec2::new(500.0, 500.0);
    let file = write_lone_region(&root, far);
    let config = ReplicationConfig {
        interest: Some(Interest::new(10.0)),
        ..ReplicationConfig::default()
    };
    // The level both ends load: a door far from the client's player, with
    // a sprite of the level's own that the wire does not bring.
    let door_id = SceneId::new();
    let level = move |world: &mut World| {
        let door = world.spawn();
        world.insert(door, door_id);
        world.insert(door, Transform::from_translation(Vec2::new(60.0, 0.0)));
        world.insert(door, Replicated::default());
        world.insert(door, Sprite::default().with_sort_order(9));
        door
    };
    let network = MemoryNetwork::new();
    let server_transport = network.endpoint();
    let address = server_transport.address();
    let mut server_app = App::default()
        .with_asset_root(&root)
        .with_replication(config.clone());
    let door = level(&mut server_app.world);
    let mut server = Headless::new(server_app);
    server
        .net_mut()
        .host_on(server_transport, NetConfig::default());

    // The client streams its lone region in before it joins: its crate is
    // the client's own, with an id the server never sends.
    let mut client_app = App::default()
        .with_asset_root(&root)
        .with_replication(config);
    level(&mut client_app.world);
    let anchor = anchor(&mut client_app.world, far + Vec2::splat(CELL / 2.0));
    client_app.stream_world(&file);
    let mut client = Headless::new(client_app);
    for _ in 0..120 {
        client.step(1, FRAME);
        std::thread::sleep(Duration::from_millis(1));
        if has_name(client.world(), "crate") {
            break;
        }
    }
    assert!(has_name(client.world(), "crate"), "the region is in");
    client
        .net_mut()
        .join_on(network.endpoint(), address, NetConfig::default());
    step(&mut server, &mut client, 60);
    let peer = client.net().local_id().expect("joined");
    let player = server.world_mut().spawn();
    server.world_mut().insert(player, SceneId::new());
    server
        .world_mut()
        .insert(player, Transform::from_translation(Vec2::ZERO));
    server.world_mut().insert(player, Replicated::default());
    server.world_mut().insert(player, NetOwner(peer));
    step(&mut server, &mut client, 60);
    let set_aside = |client: &Headless| {
        client
            .net()
            .replication()
            .and_then(|r| r.client())
            .expect("a client")
            .stats()
            .out_of_interest
    };
    assert_eq!(set_aside(&client), 2, "the far door and the crate, swept");

    // The client walks off: its region unloads, and what was swept of it
    // goes. The level's door is still the level's.
    client
        .world_mut()
        .insert(anchor, Transform::from_translation(Vec2::new(-1000.0, 0.0)));
    step(&mut server, &mut client, 60);
    assert_eq!(set_aside(&client), 1, "the door, kept");

    // The door swings to the client: the level's own copy comes back.
    server
        .world_mut()
        .insert(door, Transform::from_translation(Vec2::new(2.0, 0.0)));
    step(&mut server, &mut client, 60);
    let back = client
        .world()
        .query::<SceneId>()
        .find(|(_, id)| **id == door_id)
        .map(|(entity, _)| entity)
        .expect("relevant now");
    assert_eq!(
        client.world().get::<Sprite>(back).map(|s| s.sort_order),
        Some(9),
        "with the sprite the level gave it"
    );
}
