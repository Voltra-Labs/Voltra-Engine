//! A remote entity drawn between snapshots: under a jittery link, the
//! client's copy moves along the server's path without a jump, a jitter
//! buffer's depth behind it.

use std::collections::BTreeMap;
use std::time::Duration;

use voltra_core::net::{Conditioned, LinkConditions, MemoryNetwork, NetConfig};
use voltra_core::replication::ReplicationConfig;
use voltra_core::{App, Headless};
use voltra_render::glam::Vec2;
use voltra_scene::{Replicated, SceneId, Transform};

const FRAME: f32 = 1.0 / 60.0;
const RADIUS: f32 = 5.0;
/// Radians per server tick: a lap every two seconds.
const SPEED: f32 = std::f32::consts::TAU / 120.0;

fn jittery(seed: u64, network: &MemoryNetwork) -> Conditioned<voltra_core::net::MemoryTransport> {
    Conditioned::new(
        network.endpoint(),
        LinkConditions {
            latency: Duration::from_millis(60),
            // Up to 50 ms more per datagram, so snapshots arrive bunched,
            // gapped and out of order.
            jitter: Duration::from_millis(50),
            loss: 0.05,
            duplicate: 0.0,
        },
        seed,
    )
}

fn on_circle(tick: u32) -> Vec2 {
    let angle = tick as f32 * SPEED;
    Vec2::new(angle.cos(), angle.sin()) * RADIUS
}

#[test]
fn a_remote_entity_follows_the_servers_path_without_jumps_under_jitter() {
    // Three different runs of the dice.
    for seed in [3, 30, 300] {
        follow(seed);
    }
}

fn follow(seed: u64) {
    let network = MemoryNetwork::new();
    let server_transport = jittery(seed, &network);
    let address = server_transport.inner().address();
    // The server moves it one server step at a time, from its fixed tick:
    // its path is a function of the tick.
    let mut server = Headless::new(
        App::default()
            .with_replication(ReplicationConfig::default())
            .with_fixed_update(|tick| {
                let Some(now) = tick
                    .net
                    .replication()
                    .and_then(|r| r.server())
                    .map(|s| s.tick())
                else {
                    return;
                };
                let movers: Vec<_> = tick.world.query::<Replicated>().map(|(e, _)| e).collect();
                for entity in movers {
                    if let Some(transform) = tick.world.get_mut::<Transform>(entity) {
                        transform.translation = on_circle(now);
                    }
                }
            }),
    );
    server
        .net_mut()
        .host_on(server_transport, NetConfig::default());
    let mut client = Headless::new(App::default().with_replication(ReplicationConfig::default()));
    client
        .net_mut()
        .join_on(jittery(seed + 1, &network), address, NetConfig::default());

    let mover = server.world_mut().spawn();
    let id = SceneId::new();
    server.world_mut().insert(mover, id);
    server.world_mut().insert(mover, Replicated::default());
    server
        .world_mut()
        .insert(mover, Transform::from_translation(on_circle(0)));

    // Where the server had it, by server tick.
    let mut path: BTreeMap<u32, Vec2> = BTreeMap::new();
    let mut drawn: Vec<Vec2> = Vec::new();
    for frame in 0..600 {
        server.step(1, FRAME);
        let at = server
            .world()
            .get::<Transform>(mover)
            .expect("a transform")
            .translation;
        let tick = server
            .net()
            .replication()
            .and_then(|r| r.server())
            .map_or(0, |s| s.tick());
        path.insert(tick, at);
        client.step(1, FRAME);

        if frame >= 180 {
            let copy = client
                .world()
                .query::<SceneId>()
                .find(|(_, seen)| **seen == id)
                .map(|(entity, _)| entity)
                .expect("the client has the mover");
            drawn.push(
                client
                    .world()
                    .get::<Transform>(copy)
                    .expect("with its transform")
                    .translation,
            );
        }
    }

    let stats = client
        .net()
        .replication()
        .and_then(|r| r.client())
        .expect("replicating")
        .stats();
    // The buffer was sized from what it measured: jitter in tens of
    // milliseconds, the delay above it.
    assert!(
        stats.jitter > Duration::from_millis(20),
        "jitter measured: {:?}",
        stats.jitter
    );
    assert!(stats.delay > stats.jitter, "{stats:?}");

    // No jumps: a frame never moves it more than the server's own step,
    // give or take the 10 % the clock may run fast to catch up.
    let step = (on_circle(1) - on_circle(0)).length();
    let mut largest = 0.0f32;
    for pair in drawn.windows(2) {
        largest = largest.max((pair[1] - pair[0]).length());
    }
    assert!(
        largest <= step * 1.12,
        "a frame moved {largest}, the server's step is {step}"
    );
    // Always moving: it never stalls waiting for a late snapshot.
    let stalled = drawn
        .windows(2)
        .filter(|pair| (pair[1] - pair[0]).length() < step * 0.5)
        .count();
    assert_eq!(stalled, 0, "frames held still");

    // And on the server's path: every drawn point is on a chord between
    // two server positions at most five ticks apart — the snapshots lost,
    // or overtaken and dropped, leave gaps the draw spans.
    let chord_sag = RADIUS * (1.0 - (SPEED * 2.5).cos());
    for point in &drawn {
        let off = (point.length() - RADIUS).abs();
        assert!(
            off <= chord_sag + 1e-3,
            "{point} is {off} off the server's path"
        );
    }
    assert!(path.len() > 500);
    assert!(path.iter().all(|(&tick, &at)| at == on_circle(tick)));
}
