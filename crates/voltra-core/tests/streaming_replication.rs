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
