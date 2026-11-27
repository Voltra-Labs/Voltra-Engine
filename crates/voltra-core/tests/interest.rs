//! Interest management through `tick.net`: a headless server and headless
//! clients, each sent only what is near the entities it owns or the points
//! the game watches for it. Entering spawns, leaving despawns, the edge does
//! not flap, a hierarchy goes and comes whole, and what comes back carries
//! what changed while it was away.

use std::collections::BTreeMap;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use voltra_core::net::{Conditioned, LinkConditions, MemoryNetwork, NetConfig, PeerId};
use voltra_core::replication::{Interest, NetOwner, ReplicationConfig};
use voltra_core::{App, Game, Headless};
use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;
use voltra_scene::{hierarchy, ComponentRegistry, Replicated, SceneId, Sprite, Transform};

const FRAME: f32 = 1.0 / 60.0;

/// A replicated component besides `Transform`, to change while away.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
struct Health(u32);

struct Rules;

impl Game for Rules {
    fn register(&self, registry: &mut ComponentRegistry) {
        registry.register::<Health>("Health").replicated();
    }
}

fn lossy() -> LinkConditions {
    LinkConditions {
        latency: Duration::from_millis(30),
        jitter: Duration::from_millis(20),
        loss: 0.2,
        duplicate: 0.02,
        bandwidth: 0,
    }
}

fn interest(radius: f32) -> ReplicationConfig {
    ReplicationConfig {
        interest: Some(Interest::new(radius)),
        ..ReplicationConfig::default()
    }
}

struct Rig {
    network: MemoryNetwork,
    address: voltra_core::net::Address,
    link: LinkConditions,
    config: ReplicationConfig,
    server: Headless,
    clients: Vec<Headless>,
}

impl Rig {
    fn new(link: LinkConditions, config: ReplicationConfig) -> Self {
        let network = MemoryNetwork::new();
        let transport = Conditioned::new(network.endpoint(), link, 1);
        let address = transport.inner().address();
        let mut server = Headless::new(
            App::default()
                .with_game(Rules)
                .with_replication(config.clone()),
        );
        server.net_mut().host_on(transport, NetConfig::default());
        Self {
            network,
            address,
            link,
            config,
            server,
            clients: Vec::new(),
        }
    }

    /// A client joined, and its peer once it is in. `level` runs on its
    /// world first: the level it loaded.
    fn join(&mut self, level: impl FnOnce(&mut World)) -> PeerId {
        let seed = 10 + self.clients.len() as u64;
        let transport = Conditioned::new(self.network.endpoint(), self.link, seed);
        let mut client = Headless::new(
            App::default()
                .with_game(Rules)
                .with_replication(self.config.clone()),
        );
        level(client.world_mut());
        client
            .net_mut()
            .join_on(transport, self.address, NetConfig::default());
        self.clients.push(client);
        let index = self.clients.len() - 1;
        for _ in 0..120 {
            self.step(1);
            if let Some(peer) = self.clients[index].net().local_id() {
                return peer;
            }
        }
        panic!("the client never joined");
    }

    fn step(&mut self, frames: u32) {
        for _ in 0..frames {
            self.server.step(1, FRAME);
            for client in &mut self.clients {
                client.step(1, FRAME);
            }
        }
    }

    fn world(&mut self) -> &mut World {
        self.server.world_mut()
    }

    fn relevant(&self, peer: PeerId) -> usize {
        self.server
            .net()
            .replication()
            .and_then(|r| r.server())
            .and_then(|s| s.stats(peer))
            .expect("replicating to the client")
            .relevant
    }
}

fn spawn(world: &mut World, at: Vec2) -> (Entity, SceneId) {
    let entity = world.spawn();
    let id = SceneId::new();
    world.insert(entity, id);
    world.insert(entity, Transform::from_translation(at));
    world.insert(entity, Replicated::default());
    (entity, id)
}

fn owned(world: &mut World, at: Vec2, peer: PeerId) -> Entity {
    let (entity, _) = spawn(world, at);
    world.insert(entity, NetOwner(peer));
    entity
}

fn place(world: &mut World, entity: Entity, at: Vec2) {
    world
        .get_mut::<Transform>(entity)
        .expect("a transform")
        .translation = at;
}

/// The client's copy of `id`.
fn copy(world: &World, id: SceneId) -> Option<Entity> {
    world
        .query::<SceneId>()
        .find(|(_, other)| **other == id)
        .map(|(entity, _)| entity)
}

/// Every replicated entity's id and transform.
fn replicated(world: &World) -> BTreeMap<SceneId, Transform> {
    world
        .query::<Replicated>()
        .filter_map(|(entity, _)| {
            Some((
                *world.get::<SceneId>(entity)?,
                *world.get::<Transform>(entity)?,
            ))
        })
        .collect()
}

#[test]
fn an_entity_enters_at_the_radius_and_leaves_past_it_under_loss() {
    let mut rig = Rig::new(lossy(), interest(10.0));
    let peer = rig.join(|_| {});
    owned(rig.world(), Vec2::ZERO, peer);
    let (mover, id) = spawn(rig.world(), Vec2::new(20.0, 0.0));
    rig.step(60);
    assert!(copy(rig.clients[0].world(), id).is_none(), "far: not sent");
    assert_eq!(rig.relevant(peer), 1, "only its own");

    place(rig.world(), mover, Vec2::new(9.5, 0.0));
    rig.step(60);
    let first = copy(rig.clients[0].world(), id).expect("near: spawned");
    assert_eq!(rig.relevant(peer), 2);

    // Back and forth across the radius, inside the leave radius (11): no
    // flapping — the client keeps the one copy throughout.
    for n in 0..240 {
        let x = if (n / 7) % 2 == 0 { 10.2 } else { 10.9 };
        place(rig.world(), mover, Vec2::new(x, 0.0));
        rig.step(1);
        assert_eq!(
            copy(rig.clients[0].world(), id),
            Some(first),
            "frame {n}: the same copy"
        );
    }

    place(rig.world(), mover, Vec2::new(11.5, 0.0));
    rig.step(90);
    assert!(
        copy(rig.clients[0].world(), id).is_none(),
        "past the leave radius: despawned, through 20 % loss"
    );
    assert!(
        rig.server.world().is_alive(mover),
        "on the server it lives on"
    );
    let stats = rig.clients[0]
        .net()
        .replication()
        .and_then(|r| r.client())
        .expect("a client")
        .stats();
    assert_eq!(stats.out_of_interest, 1, "set aside");

    // And it comes back where the server has it.
    place(rig.world(), mover, Vec2::new(-4.0, 3.0));
    rig.step(90);
    let back = copy(rig.clients[0].world(), id).expect("back");
    assert_eq!(
        rig.clients[0]
            .world()
            .get::<Transform>(back)
            .map(|t| t.translation),
        Some(Vec2::new(-4.0, 3.0))
    );

    // Away again and despawned for real meanwhile: the client lets go of
    // what it set aside.
    place(rig.world(), mover, Vec2::new(40.0, 0.0));
    rig.step(90);
    assert!(copy(rig.clients[0].world(), id).is_none());
    rig.world().despawn(mover);
    rig.step(90);
    let stats = rig.clients[0]
        .net()
        .replication()
        .and_then(|r| r.client())
        .expect("a client")
        .stats();
    assert_eq!(stats.out_of_interest, 0, "nothing kept");
}

#[test]
fn an_always_relevant_entity_and_one_with_no_place_reach_everyone() {
    let mut rig = Rig::new(LinkConditions::default(), interest(5.0));
    let peer = rig.join(|_| {});
    owned(rig.world(), Vec2::ZERO, peer);
    let (beacon, beacon_id) = spawn(rig.world(), Vec2::new(1000.0, 0.0));
    rig.world()
        .insert(beacon, Replicated::default().always_relevant());
    let round = rig.world().spawn();
    let round_id = SceneId::new();
    rig.world().insert(round, round_id);
    rig.world().insert(round, Replicated::default());
    rig.world().insert(round, Health(5));
    let (_, wide_id) = spawn(rig.world(), Vec2::new(0.0, 40.0));
    let wide = copy(rig.server.world(), wide_id).expect("there");
    rig.world()
        .insert(wide, Replicated::default().with_relevance_radius(50.0));
    let (_, far_id) = spawn(rig.world(), Vec2::new(0.0, 40.0));
    rig.step(30);

    let client = rig.clients[0].world();
    assert!(copy(client, beacon_id).is_some(), "always relevant");
    assert!(copy(client, round_id).is_some(), "no transform: everywhere");
    assert!(copy(client, wide_id).is_some(), "its own radius reaches");
    assert!(copy(client, far_id).is_none(), "the session's does not");
}

#[test]
fn the_owned_entity_and_its_hierarchy_are_relevant_wherever_they_are() {
    let mut rig = Rig::new(LinkConditions::default(), interest(5.0));
    let peer = rig.join(|_| {});
    // Its player rides in a seat a long way from the vehicle's origin: the
    // vehicle is the replicated root, relevant through the seat.
    let (vehicle, vehicle_id) = spawn(rig.world(), Vec2::new(-300.0, 0.0));
    let (seat, seat_id) = spawn(rig.world(), Vec2::new(200.0, 0.0));
    rig.world().insert(seat, NetOwner(peer));
    hierarchy::set_parent(rig.world(), seat, vehicle).expect("parents");
    // A turret on a vehicle nobody is near, its child placed by the player.
    let (tank, tank_id) = spawn(rig.world(), Vec2::new(500.0, 0.0));
    let (turret, turret_id) = spawn(rig.world(), Vec2::new(-600.0, 0.0));
    hierarchy::set_parent(rig.world(), turret, tank).expect("parents");
    rig.step(30);

    let client = rig.clients[0].world();
    assert!(
        copy(client, vehicle_id).is_some(),
        "the owned entity's root"
    );
    assert!(copy(client, seat_id).is_some(), "the owned entity");
    assert!(
        copy(client, tank_id).is_none() && copy(client, turret_id).is_none(),
        "a child follows its root, even sitting next to the player"
    );
    assert_eq!(rig.relevant(peer), 2);

    // The player brings the tank near: both come, root and child.
    place(rig.world(), tank, Vec2::new(-98.0, 0.0));
    rig.step(30);
    let client = rig.clients[0].world();
    assert!(copy(client, tank_id).is_some() && copy(client, turret_id).is_some());
    assert_eq!(rig.relevant(peer), 4);
}

#[test]
fn with_its_owner_gone_a_client_sees_what_the_game_watches_for_it() {
    let mut rig = Rig::new(LinkConditions::default(), interest(8.0));
    let peer = rig.join(|_| {});
    let player = owned(rig.world(), Vec2::ZERO, peer);
    let (_, near_id) = spawn(rig.world(), Vec2::new(3.0, 0.0));
    let (_, over_there_id) = spawn(rig.world(), Vec2::new(100.0, 0.0));
    rig.step(30);
    assert!(copy(rig.clients[0].world(), near_id).is_some());

    // The player dies: nothing to watch from, so nothing near it is sent.
    rig.world().despawn(player);
    rig.step(30);
    assert!(copy(rig.clients[0].world(), near_id).is_none());
    assert_eq!(rig.relevant(peer), 0);

    // A spectator camera, set by the game.
    assert!(rig
        .server
        .net_mut()
        .set_interest_points(peer, vec![Vec2::new(98.0, 0.0)]));
    rig.step(30);
    let client = rig.clients[0].world();
    assert!(copy(client, over_there_id).is_some(), "watched");
    assert!(copy(client, near_id).is_none(), "and only that");
    let points = rig
        .server
        .net()
        .replication()
        .and_then(|r| r.server())
        .expect("a server")
        .interest_points(peer)
        .to_vec();
    assert_eq!(points, [Vec2::new(98.0, 0.0)]);
}

#[test]
fn an_entity_back_in_interest_carries_what_changed_and_keeps_what_was_its_own() {
    let mut rig = Rig::new(lossy(), interest(10.0));
    // A level both ends loaded: a crate the client draws with its sprite.
    let crate_id = SceneId::new();
    let level = move |world: &mut World| {
        let entity = world.spawn();
        world.insert(entity, crate_id);
        world.insert(entity, Transform::from_translation(Vec2::new(2.0, 0.0)));
        world.insert(entity, Replicated::default());
        world.insert(entity, Health(10));
        world.insert(entity, Sprite::default().with_sort_order(7));
        entity
    };
    let on_server = level(rig.world());
    let peer = rig.join(|world| {
        level(world);
    });
    owned(rig.world(), Vec2::ZERO, peer);
    let (shield, shield_id) = spawn(rig.world(), Vec2::new(-2.0, 0.0));
    rig.world().insert(shield, Health(1));
    rig.step(60);
    let adopted = copy(rig.clients[0].world(), crate_id).expect("adopted");
    assert_eq!(
        rig.clients[0].world().get::<Health>(adopted),
        Some(&Health(10))
    );

    // Out of interest; changed while away: moved, hurt, a component gone.
    place(rig.world(), on_server, Vec2::new(50.0, 0.0));
    place(rig.world(), shield, Vec2::new(-50.0, 0.0));
    rig.step(60);
    assert!(copy(rig.clients[0].world(), crate_id).is_none());
    rig.world().insert(on_server, Health(3));
    rig.world()
        .get_mut::<Transform>(on_server)
        .expect("a transform")
        .rotation = 1.5;
    rig.world().remove::<Health>(shield);
    rig.step(30);

    place(rig.world(), on_server, Vec2::new(4.0, 0.0));
    place(rig.world(), shield, Vec2::new(-4.0, 0.0));
    rig.step(90);
    let client = rig.clients[0].world();
    let back = copy(client, crate_id).expect("back");
    assert_eq!(client.get::<Health>(back), Some(&Health(3)), "what changed");
    assert_eq!(
        client
            .get::<Transform>(back)
            .map(|t| (t.translation, t.rotation)),
        Some((Vec2::new(4.0, 0.0), 1.5))
    );
    assert_eq!(
        client.get::<Sprite>(back).map(|s| s.sort_order),
        Some(7),
        "the level's sprite, the client's own, kept"
    );
    let shield_back = copy(client, shield_id).expect("back too");
    assert_eq!(
        client.get::<Health>(shield_back),
        None,
        "removed while away stays removed"
    );
    let copies = client
        .query::<SceneId>()
        .filter(|(_, id)| **id == crate_id)
        .count();
    assert_eq!(copies, 1, "one copy");
}

#[test]
fn relevant_entities_share_the_budget_and_far_ones_cost_nothing() {
    let budget = 600;
    let config = ReplicationConfig {
        bytes_per_snapshot: budget,
        ..interest(10.0)
    };
    let mut rig = Rig::new(LinkConditions::default(), config);
    let peer = rig.join(|_| {});
    owned(rig.world(), Vec2::ZERO, peer);
    // A crowd far away moving every tick, and twenty near.
    let far: Vec<Entity> = (0..300)
        .map(|n| spawn(rig.world(), Vec2::new(100.0 + n as f32, 0.0)).0)
        .collect();
    let near: Vec<Entity> = (0..20)
        .map(|n| spawn(rig.world(), Vec2::new(-5.0 + n as f32 * 0.5, 1.0)).0)
        .collect();
    rig.step(30);

    let mut largest = 0;
    let mut cut = 0;
    for frame in 0..120 {
        for (n, &entity) in far.iter().chain(&near).enumerate() {
            if let Some(t) = rig.world().get_mut::<Transform>(entity) {
                t.translation.y = (frame as f32 * 0.1 + n as f32).sin();
            }
        }
        rig.step(1);
        let stats = rig
            .server
            .net()
            .replication()
            .and_then(|r| r.server())
            .and_then(|s| s.stats(peer))
            .expect("replicating");
        largest = largest.max(stats.snapshot_bytes);
        if !stats.complete {
            cut += 1;
        }
        assert_eq!(stats.relevant, 21);
    }
    assert!(largest <= budget, "a snapshot of {largest} bytes");
    assert_eq!(cut, 0, "the twenty near ones fit every snapshot");
    rig.step(30);
    let seen = replicated(rig.clients[0].world());
    assert_eq!(seen.len(), 21, "its own and the near ones, never the crowd");
    let truth = replicated(rig.server.world());
    for (id, transform) in &seen {
        assert_eq!(truth.get(id), Some(transform), "exactly the server's");
    }
}

#[test]
fn interest_covering_everything_sends_byte_for_byte_what_none_does() {
    let run = |config: ReplicationConfig| {
        let mut rig = Rig::new(LinkConditions::default(), config);
        let peer = rig.join(|_| {});
        owned(rig.world(), Vec2::ZERO, peer);
        let movers: Vec<Entity> = (0..40)
            .map(|n| spawn(rig.world(), Vec2::new(n as f32 * 3.0, 0.0)).0)
            .collect();
        let mut bytes = Vec::new();
        for frame in 0..90 {
            for (n, &entity) in movers.iter().enumerate() {
                if let Some(t) = rig.world().get_mut::<Transform>(entity) {
                    t.translation = Vec2::new(n as f32 * 3.0, (frame as f32 * 0.2).sin());
                }
            }
            if frame == 45 {
                rig.world().despawn(movers[3]);
            }
            rig.step(1);
            let stats = rig
                .server
                .net()
                .replication()
                .and_then(|r| r.server())
                .and_then(|s| s.stats(peer))
                .expect("replicating");
            bytes.push((stats.snapshot_bytes, stats.relevant));
        }
        (bytes, replicated(rig.clients[0].world()).len())
    };
    let (none, seen_none) = run(ReplicationConfig::default());
    let (all, seen_all) = run(interest(1.0e6));
    assert_eq!(seen_none, 40, "its own and 39 movers");
    assert_eq!(seen_all, seen_none);
    assert_eq!(all, none, "the same snapshots, frame by frame");
}

#[test]
fn a_client_joining_late_is_sent_what_is_near_it_and_finds_its_level_again() {
    let mut rig = Rig::new(lossy(), interest(10.0));
    let early = rig.join(|_| {});
    owned(rig.world(), Vec2::ZERO, early);
    let door_id = SceneId::new();
    let level = move |world: &mut World| {
        let door = world.spawn();
        world.insert(door, door_id);
        world.insert(door, Transform::from_translation(Vec2::new(60.0, 0.0)));
        world.insert(door, Replicated::default());
        world.insert(door, Sprite::default().with_sort_order(9));
        door
    };
    let door = level(rig.world());
    let (_, crowd_id) = spawn(rig.world(), Vec2::new(30.0, 30.0));
    rig.step(120);

    let late = rig.join(|world| {
        level(world);
    });
    owned(rig.world(), Vec2::new(30.0, 28.0), late);
    rig.step(120);
    let client = rig.clients[1].world();
    assert!(copy(client, crowd_id).is_some(), "near it");
    assert!(
        copy(client, door_id).is_none(),
        "the level's far door, never sent, swept out of the world"
    );
    assert!(
        copy(rig.clients[0].world(), crowd_id).is_none(),
        "far from the first"
    );

    // The door swings to the late client: the level's own copy comes back.
    place(rig.world(), door, Vec2::new(31.0, 25.0));
    rig.step(120);
    let client = rig.clients[1].world();
    let back = copy(client, door_id).expect("relevant now");
    assert_eq!(client.get::<Sprite>(back).map(|s| s.sort_order), Some(9));
}
