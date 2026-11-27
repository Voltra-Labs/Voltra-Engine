//! Replication through `tick.net`: a headless server and two headless
//! clients over a lossy in-memory network. Spawns, moves and despawns on
//! the server reach both clients' worlds; the snapshots stay inside their
//! byte budget, and priorities share it out.

use std::collections::BTreeMap;
use std::time::Duration;

use voltra_core::net::{Conditioned, LinkConditions, MemoryNetwork, NetConfig};
use voltra_core::replication::ReplicationConfig;
use voltra_core::{App, Headless};
use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;
use voltra_scene::{Name, Replicated, SceneId, Transform};

const FRAME: f32 = 1.0 / 60.0;

/// 20 % of datagrams lost, 30–50 ms late (so reordered) — both ways.
fn lossy() -> LinkConditions {
    LinkConditions {
        latency: Duration::from_millis(30),
        jitter: Duration::from_millis(20),
        loss: 0.2,
        duplicate: 0.02,
        bandwidth: 0,
    }
}

struct Rig {
    server: Headless,
    clients: Vec<Headless>,
}

impl Rig {
    fn new(clients: usize, link: LinkConditions, config: ReplicationConfig) -> Self {
        let network = MemoryNetwork::new();
        let server_transport = Conditioned::new(network.endpoint(), link, 1);
        let address = server_transport.inner().address();
        let mut server = Headless::new(App::default().with_replication(config.clone()));
        server
            .net_mut()
            .host_on(server_transport, NetConfig::default());
        let clients = (0..clients)
            .map(|n| {
                let transport = Conditioned::new(network.endpoint(), link, 10 + n as u64);
                let mut client = Headless::new(App::default().with_replication(config.clone()));
                client
                    .net_mut()
                    .join_on(transport, address, NetConfig::default());
                client
            })
            .collect();
        Self { server, clients }
    }

    fn step(&mut self, frames: u32, mut each: impl FnMut(&mut Headless, u32)) {
        for frame in 0..frames {
            each(&mut self.server, frame);
            self.server.step(1, FRAME);
            for client in &mut self.clients {
                client.step(1, FRAME);
            }
        }
    }
}

/// Every replicated entity's id and transform.
fn replicated(world: &World) -> BTreeMap<SceneId, Transform> {
    world
        .query::<Replicated>()
        .map(|(entity, _)| {
            (
                *world
                    .get::<SceneId>(entity)
                    .expect("a replicated entity has an id"),
                *world.get::<Transform>(entity).expect("and a transform"),
            )
        })
        .collect()
}

fn spawn(world: &mut World, at: Vec2, priority: f32) -> Entity {
    let entity = world.spawn();
    world.insert(entity, SceneId::new());
    world.insert(entity, Transform::from_translation(at));
    world.insert(entity, Replicated::with_priority(priority));
    entity
}

#[test]
fn two_clients_see_the_servers_world_after_spawns_moves_and_despawns_under_loss() {
    let mut rig = Rig::new(2, lossy(), ReplicationConfig::default());

    // A level entity every end loaded from one file: adopted, not doubled.
    let level_id = SceneId::new();
    for game in std::iter::once(&mut rig.server).chain(rig.clients.iter_mut()) {
        let world = game.world_mut();
        let entity = world.spawn();
        world.insert(entity, level_id);
        world.insert(entity, Transform::default());
        world.insert(entity, Replicated::default());
        world.insert(entity, Name::new("door"));
    }
    // And one only a client's level has, which the server despawned before
    // it joined: removed.
    let stray = spawn(rig.clients[0].world_mut(), Vec2::ZERO, 1.0);
    rig.step(60, |_, _| {});
    assert!(rig.clients.iter().all(|c| c.net().local_id().is_some()));

    let mut movers: Vec<Entity> = (0..20)
        .map(|n| spawn(rig.server.world_mut(), Vec2::new(n as f32, 0.0), 1.0))
        .collect();
    rig.step(60, |_, _| {});

    // Everything moves for two seconds.
    let moving = movers.clone();
    rig.step(120, |server, frame| {
        for (n, &entity) in moving.iter().enumerate() {
            let angle = frame as f32 * 0.05 + n as f32;
            let at = Vec2::new(angle.cos(), angle.sin()) * (3.0 + n as f32);
            if let Some(transform) = server.world_mut().get_mut::<Transform>(entity) {
                transform.translation = at;
                transform.rotation = angle;
            }
        }
    });

    // Some go, some come.
    for entity in movers.drain(..7) {
        rig.server.world_mut().despawn(entity);
    }
    for n in 0..3 {
        movers.push(spawn(
            rig.server.world_mut(),
            Vec2::new(-1.0, n as f32),
            1.0,
        ));
    }
    // The adopted door moves on the server, and so on the clients.
    let door = rig
        .server
        .world()
        .query::<SceneId>()
        .find(|(_, id)| **id == level_id)
        .map(|(e, _)| e)
        .expect("the door");
    rig.server
        .world_mut()
        .insert(door, Transform::from_translation(Vec2::new(5.0, 5.0)));
    rig.step(180, |_, _| {});

    let truth = replicated(rig.server.world());
    assert_eq!(truth.len(), 17, "16 movers and the door");
    for (n, client) in rig.clients.iter().enumerate() {
        assert_eq!(
            replicated(client.world()),
            truth,
            "client {n} sees the server's world"
        );
    }
    assert!(!rig.clients[0].world().is_alive(stray));
    let doors = rig.clients[1]
        .world()
        .query::<Name>()
        .filter(|(_, name)| name.as_str() == "door")
        .count();
    assert_eq!(doors, 1, "the level's door adopted, not doubled");

    // The loss was real.
    let replication = rig.server.net().replication().expect("replicating");
    let server = replication.server().expect("a server");
    assert!(server.tick() > 300);
    for peer in rig.server.net().peers().collect::<Vec<_>>() {
        let stats = rig.server.net().stats(peer).expect("connected");
        assert!(stats.packets_sent > 300);
    }
}

#[test]
fn snapshots_stay_inside_the_budget_and_priorities_share_it() {
    let budget = 600;
    let config = ReplicationConfig {
        bytes_per_snapshot: budget,
        ..ReplicationConfig::default()
    };
    let mut rig = Rig::new(1, LinkConditions::default(), config);
    rig.step(30, |_, _| {});

    // Far more than fits: 300 entities moving every tick, one of them
    // important.
    let crowd: Vec<Entity> = (0..300)
        .map(|n| spawn(rig.server.world_mut(), Vec2::new(n as f32, 0.0), 1.0))
        .collect();
    let star = spawn(rig.server.world_mut(), Vec2::ZERO, 40.0);
    let peer = rig.server.net().peers().next().expect("a client");

    let mut largest_snapshot = 0;
    let mut largest_frame = 0;
    let mut star_lag = 0.0f32;
    let mut crowd_lag = 0.0f32;
    let mut sent_before = rig.server.net().stats(peer).expect("connected").bytes_sent;
    for frame in 0..240u32 {
        let world = rig.server.world_mut();
        for (n, &entity) in crowd.iter().chain([&star]).enumerate() {
            if let Some(t) = world.get_mut::<Transform>(entity) {
                t.translation.y = (frame as f32 * 0.1 + n as f32).sin();
            }
        }
        rig.server.step(1, FRAME);
        rig.clients[0].step(1, FRAME);

        let stats = rig
            .server
            .net()
            .replication()
            .and_then(|r| r.server())
            .and_then(|s| s.stats(peer))
            .expect("replicating to the client");
        largest_snapshot = largest_snapshot.max(stats.snapshot_bytes);
        let sent = rig.server.net().stats(peer).expect("connected").bytes_sent;
        if frame > 10 {
            largest_frame = largest_frame.max(sent - sent_before);
        }
        sent_before = sent;

        if frame > 120 {
            let truth = replicated(rig.server.world());
            let seen = replicated(rig.clients[0].world());
            let lag = |entity: Entity| {
                let id = rig.server.world().get::<SceneId>(entity).expect("an id");
                match (truth.get(id), seen.get(id)) {
                    (Some(t), Some(s)) => (t.translation - s.translation).length(),
                    _ => 2.0,
                }
            };
            star_lag = star_lag.max(lag(star));
            crowd_lag += crowd.iter().map(|&e| lag(e)).sum::<f32>() / crowd.len() as f32;
        }
    }
    assert!(
        largest_snapshot <= budget,
        "a snapshot of {largest_snapshot} bytes, over {budget}"
    );
    // The packet around it: headers, acks — a datagram's worth at most.
    assert!(
        largest_frame <= (budget + 100) as u64,
        "a frame sent {largest_frame} bytes"
    );
    let crowd_lag = crowd_lag / 119.0;
    assert!(
        star_lag < crowd_lag,
        "the important entity is kept closer: {star_lag} vs {crowd_lag} on average"
    );

    // Once they rest, every one of them gets through.
    rig.step(240, |_, _| {});
    assert_eq!(
        replicated(rig.clients[0].world()),
        replicated(rig.server.world())
    );
}

#[test]
fn an_entity_spawned_from_a_prefab_is_spawned_from_it_on_the_client() {
    let root = voltra_testkit::scratch_root();
    std::fs::create_dir_all(root.join("prefabs")).expect("a folder");
    std::fs::write(
        root.join("prefabs/ship.prefab.ron"),
        r#"(
    version: 1,
    entities: [
        (
            id: "01a2b100-0000-7000-8000-0000000000a1",
            components: {
                "Name": "Ship",
                "Transform": (translation:(0.0,0.0),rotation:0.0,scale:(1.0,1.0)),
                "Replicated": (priority: 2.0),
            },
        ),
        (
            id: "01a2b100-0000-7000-8000-0000000000a2",
            components: {
                "Name": "Flame",
                "Parent": "01a2b100-0000-7000-8000-0000000000a1",
                "Transform": (translation:(0.0,-1.0),rotation:0.0,scale:(1.0,1.0)),
            },
        ),
    ],
)"#,
    )
    .expect("written");
    let network = MemoryNetwork::new();
    let server_transport = network.endpoint();
    let address = server_transport.address();
    let app = || {
        App::default()
            .with_asset_root(root.clone())
            .with_replication(ReplicationConfig::default())
    };
    let mut server = Headless::new(app());
    server
        .net_mut()
        .host_on(server_transport, NetConfig::default());
    let mut client = Headless::new(app());
    client
        .net_mut()
        .join_on(network.endpoint(), address, NetConfig::default());
    for _ in 0..10 {
        server.step(1, FRAME);
        client.step(1, FRAME);
    }

    let path = voltra_assets::AssetPath::new("prefabs/ship.prefab.ron").expect("a path");
    let ship = server
        .spawn(&path, &Transform::from_translation(Vec2::new(4.0, 2.0)))
        .expect("spawns")
        .root()
        .expect("a root");
    for _ in 0..20 {
        server.step(1, FRAME);
        client.step(1, FRAME);
    }
    let names: Vec<String> = client
        .world()
        .query::<Name>()
        .map(|(_, name)| name.as_str().to_owned())
        .collect();
    assert!(names.contains(&"Ship".to_owned()) && names.contains(&"Flame".to_owned()));
    assert_eq!(names.len(), 2, "one copy of each: {names:?}");
    assert_eq!(replicated(client.world()), replicated(server.world()));

    // Despawned: the whole copy goes, its child included.
    voltra_scene::hierarchy::despawn_recursive(server.world_mut(), ship);
    for _ in 0..20 {
        server.step(1, FRAME);
        client.step(1, FRAME);
    }
    assert_eq!(client.world().query::<Name>().count(), 0);
}

#[test]
fn a_peer_on_another_replication_wire_is_refused_at_the_door() {
    use std::cell::RefCell;
    use std::rc::Rc;
    use voltra_core::net::{DisconnectReason, NetEvent};
    use voltra_core::replication::REPLICATION_WIRE_VERSION;

    let network = MemoryNetwork::new();
    let transport = network.endpoint();
    let address = transport.address();
    let config = ReplicationConfig::default();
    let mut server = Headless::new(App::default().with_replication(config.clone()));
    server.net_mut().host_on(transport, NetConfig::default());

    // A build from before the wire last changed: replication's channels,
    // the old version. Replication is off here only so the config is
    // joined with as it is.
    let mut old = config.net_config(NetConfig::default());
    assert_eq!(old.layer_version, REPLICATION_WIRE_VERSION);
    old.layer_version = REPLICATION_WIRE_VERSION - 1;
    let events = Rc::new(RefCell::new(Vec::new()));
    let seen = events.clone();
    let mut client = Headless::new(
        App::default()
            .with_update(move |tick| seen.borrow_mut().extend(tick.net.events().to_vec())),
    );
    client.net_mut().join_on(network.endpoint(), address, old);
    for _ in 0..60 {
        server.step(1, FRAME);
        client.step(1, FRAME);
    }
    assert_eq!(client.net().local_id(), None, "never let in");
    assert!(server.net().peers().next().is_none());
    assert!(
        events.borrow().contains(&NetEvent::Disconnected {
            peer: voltra_core::net::PeerId::SERVER,
            reason: DisconnectReason::ProtocolMismatch,
        }),
        "refused as another protocol: {:?}",
        events.borrow()
    );
}
