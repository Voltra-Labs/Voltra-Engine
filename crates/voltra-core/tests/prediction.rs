//! Prediction and reconciliation: under 150 ms of latency each way, the
//! client's own entity moves the step its player presses, and ends where
//! the server says, with no correction to see on the way.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use voltra_core::net::{Conditioned, LinkConditions, MemoryNetwork, NetConfig, NetEvent, PeerId};
use voltra_core::replication::{NetOwner, Prediction, ReplicationConfig};
use voltra_core::{App, Headless, Net};
use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;
use voltra_scene::{Replicated, SceneId, Transform};

const FRAME: f32 = 1.0 / 60.0;
const SPEED: f32 = 3.0;

/// What the player holds: a direction on each axis.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
struct Stick {
    x: i8,
    y: i8,
}

/// The one movement both ends run: the server for real, the client ahead
/// of it and again on every correction.
fn walk(world: &mut World, entity: Entity, stick: &Stick, step: f32) {
    if let Some(transform) = world.get_mut::<Transform>(entity) {
        transform.translation += Vec2::new(f32::from(stick.x), f32::from(stick.y)) * SPEED * step;
    }
}

fn slow(seed: u64, network: &MemoryNetwork) -> Conditioned<voltra_core::net::MemoryTransport> {
    Conditioned::new(
        network.endpoint(),
        LinkConditions {
            latency: Duration::from_millis(150),
            jitter: Duration::ZERO,
            loss: 0.0,
            duplicate: 0.0,
            bandwidth: 0,
        },
        seed,
    )
}

fn server_game() -> App {
    App::default()
        .with_replication(ReplicationConfig::default())
        .with_update(|tick| {
            // A player per client, as it joins.
            for event in tick.net.events().to_vec() {
                if let NetEvent::Connected(peer) = event {
                    let world = &mut *tick.world;
                    let player = world.spawn();
                    world.insert(player, SceneId::new());
                    world.insert(player, Transform::default());
                    world.insert(player, Replicated::with_priority(4.0));
                    world.insert(player, NetOwner(peer));
                }
            }
        })
        .with_fixed_update(|tick| {
            let players: Vec<(Entity, PeerId)> = tick
                .world
                .query::<NetOwner>()
                .map(|(entity, owner)| (entity, owner.0))
                .collect();
            for (entity, peer) in players {
                // No input this step: the client predicted no step either.
                if let Some(stick) = tick.net.input::<Stick>(peer) {
                    walk(tick.world, entity, &stick, tick.delta);
                }
            }
        })
}

#[test]
fn the_local_player_moves_at_once_and_ends_where_the_server_says() {
    let network = MemoryNetwork::new();
    let server_transport = slow(1, &network);
    let address = server_transport.inner().address();
    let mut server = Headless::new(server_game());
    server
        .net_mut()
        .host_on(server_transport, NetConfig::default());

    let held = Rc::new(Cell::new(Stick::default()));
    let prediction = Rc::new(RefCell::new(Prediction::<Stick>::new()));
    let (stick, predicted) = (held.clone(), prediction.clone());
    let mut client = Headless::new(
        App::default()
            .with_replication(ReplicationConfig::default())
            .with_fixed_update(move |tick| {
                tick.predict(&mut predicted.borrow_mut(), stick.get(), walk);
            }),
    );
    client
        .net_mut()
        .join_on(slow(2, &network), address, NetConfig::default());

    let step = |server: &mut Headless, client: &mut Headless| {
        server.step(1, FRAME);
        client.step(1, FRAME);
    };
    for _ in 0..90 {
        step(&mut server, &mut client);
    }
    let mine = prediction
        .borrow()
        .entity()
        .expect("the server sent our player");
    let local_id = client.net().local_id().expect("joined");
    let theirs = Net::owned_by(server.world(), local_id).expect("the server's copy");
    let at = |game: &Headless, entity: Entity| {
        game.world()
            .get::<Transform>(entity)
            .expect("a transform")
            .translation
    };
    assert_eq!(at(&client, mine), Vec2::ZERO);

    // Pressed: moving the very next step here, a round trip before the
    // server even hears of it.
    held.set(Stick { x: 1, y: 0 });
    step(&mut server, &mut client);
    assert!(at(&client, mine).x > 0.0, "the player moves at once");
    assert_eq!(
        at(&server, theirs),
        Vec2::ZERO,
        "the server has not heard yet"
    );

    // Held for two seconds, turning once: every correction lands exactly
    // where the prediction already was.
    for frame in 0..120 {
        if frame == 60 {
            held.set(Stick { x: 0, y: 1 });
        }
        step(&mut server, &mut client);
    }
    let stats = prediction.borrow().stats();
    assert!(stats.corrections > 60, "{stats:?}");
    assert!(
        stats.replayed / stats.corrections >= 10,
        "a round trip of inputs replayed each time: {stats:?}"
    );
    assert!(
        stats.max_error < 1e-5,
        "no snap in the steady state: {stats:?}"
    );
    let ahead = at(&client, mine).distance(at(&server, theirs));
    // Ahead by the inputs still on their way: 150 ms of them, 0.45 units.
    assert!(
        ahead > SPEED * 0.12,
        "the client is ahead of the server by its latency: {ahead}"
    );

    // Let go: the server catches up, and both are at the same place.
    held.set(Stick::default());
    for _ in 0..60 {
        step(&mut server, &mut client);
    }
    assert_eq!(at(&client, mine), at(&server, theirs));
    assert!(at(&client, mine).x > 0.0 && at(&client, mine).y > 0.0);
    // What waits for the server is a round trip of inputs, no more: the
    // acknowledged ones are let go.
    let pending = prediction.borrow().pending();
    assert!((15..=24).contains(&pending), "{pending} inputs pending");
}

#[test]
fn a_misprediction_is_corrected_to_the_servers_state() {
    // The server moves the player on its own — a push the client cannot
    // predict — and the client ends where the server put it.
    let network = MemoryNetwork::new();
    let server_transport = slow(3, &network);
    let address = server_transport.inner().address();
    let mut server = Headless::new(server_game());
    server
        .net_mut()
        .host_on(server_transport, NetConfig::default());
    let prediction = Rc::new(RefCell::new(Prediction::<Stick>::new()));
    let predicted = prediction.clone();
    let mut client = Headless::new(
        App::default()
            .with_replication(ReplicationConfig::default())
            .with_fixed_update(move |tick| {
                tick.predict(&mut predicted.borrow_mut(), Stick { x: 1, y: 0 }, walk);
            }),
    );
    client
        .net_mut()
        .join_on(slow(4, &network), address, NetConfig::default());
    for _ in 0..90 {
        server.step(1, FRAME);
        client.step(1, FRAME);
    }
    let local_id = client.net().local_id().expect("joined");
    let theirs = Net::owned_by(server.world(), local_id).expect("the server's copy");
    server
        .world_mut()
        .get_mut::<Transform>(theirs)
        .expect("a transform")
        .translation
        .y = 2.0;
    for _ in 0..60 {
        server.step(1, FRAME);
        client.step(1, FRAME);
    }
    let mine = prediction.borrow().entity().expect("predicting");
    let y = client
        .world()
        .get::<Transform>(mine)
        .expect("a transform")
        .translation
        .y;
    assert_eq!(y, 2.0, "the server wins");
    assert!(prediction.borrow().stats().max_error >= 2.0 - 1e-4);
}
