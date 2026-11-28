//! A remote 3D entity drawn between snapshots (stage 134): its
//! `Transform3D` moves every frame along the server's path, its rotation
//! slerped in step with its position, and comes to rest where the server's
//! is.

use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use voltra_core::net::{Conditioned, LinkConditions, MemoryNetwork, NetConfig};
use voltra_core::replication::ReplicationConfig;
use voltra_core::{App, Headless};
use voltra_ecs::{Entity, World};
use voltra_render::glam::{Quat, Vec3};
use voltra_scene::{Replicated, SceneId, Transform3D};
use voltra_testkit::rotation_apart;

const FRAME: f32 = 1.0 / 60.0;

fn link(
    seed: u64,
    network: &MemoryNetwork,
    latency: u64,
) -> Conditioned<voltra_core::net::MemoryTransport> {
    Conditioned::new(
        network.endpoint(),
        LinkConditions {
            latency: Duration::from_millis(latency),
            jitter: Duration::ZERO,
            loss: 0.0,
            duplicate: 0.0,
            bandwidth: 0,
        },
        seed,
    )
}

/// The client's copy of `id`.
fn copy(world: &World, id: SceneId) -> Option<Entity> {
    world
        .query::<SceneId>()
        .find(|(_, other)| **other == id)
        .map(|(entity, _)| entity)
}

fn transform(world: &World, entity: Entity) -> Transform3D {
    *world.get::<Transform3D>(entity).expect("a 3D transform")
}

/// Metres per server tick along the path, and radians per tick about Y.
const PACE: f32 = 0.05;
const TURN: f32 = 0.02;

/// Where the mover is at `tick`: along (1, 0.5, -1), turned about Y by
/// the same parameter — so a drawn copy's rotation can be checked against
/// its own position.
fn along(tick: u32) -> Transform3D {
    let s = tick as f32;
    Transform3D::from_translation(Vec3::new(1.0, 0.5, -1.0) * s * PACE)
        .with_rotation(Quat::from_rotation_y(s * TURN))
}

#[test]
fn a_3d_remote_entity_is_drawn_between_snapshots() {
    // A snapshot every third tick: a copy snapped to each would hold still
    // two frames in three.
    let config = ReplicationConfig {
        snapshot_interval: 3,
        ..ReplicationConfig::default()
    };
    let network = MemoryNetwork::new();
    let server_transport = link(1, &network, 40);
    let address = server_transport.inner().address();
    let moving = Rc::new(Cell::new(true));
    let still = moving.clone();
    let mut server = Headless::new(
        App::default()
            .with_replication(config.clone())
            .with_fixed_update(move |tick| {
                let Some(now) = tick
                    .net
                    .replication()
                    .and_then(|r| r.server())
                    .map(|s| s.tick())
                else {
                    return;
                };
                if !still.get() {
                    return;
                }
                let movers: Vec<_> = tick.world.query::<Replicated>().map(|(e, _)| e).collect();
                for entity in movers {
                    if let Some(t) = tick.world.get_mut::<Transform3D>(entity) {
                        *t = along(now);
                    }
                }
            }),
    );
    server
        .net_mut()
        .host_on(server_transport, NetConfig::default());
    let mut client = Headless::new(App::default().with_replication(config));
    client
        .net_mut()
        .join_on(link(2, &network, 40), address, NetConfig::default());
    let mover = server.world_mut().spawn();
    let id = SceneId::new();
    server.world_mut().insert(mover, id);
    server.world_mut().insert(mover, Replicated::default());
    server.world_mut().insert(mover, along(0));

    let mut drawn: Vec<Transform3D> = Vec::new();
    for frame in 0..300 {
        server.step(1, FRAME);
        client.step(1, FRAME);
        if frame >= 120 {
            let seen = copy(client.world(), id).expect("the client has the mover");
            drawn.push(transform(client.world(), seen));
        }
    }
    let step = (along(1).translation - along(0).translation).length();
    for pair in drawn.windows(2) {
        let moved = (pair[1].translation - pair[0].translation).length();
        assert!(
            moved > step * 0.5 && moved < step * 1.12,
            "a frame moved {moved} m; the server's step is {step} m"
        );
    }
    for t in &drawn {
        // On the server's line, and turned as far as it has come along it:
        // translation and rotation are drawn at the same moment.
        let s = t.translation.x / PACE;
        let expected = along(0)
            .with_rotation(Quat::from_rotation_y(s * TURN))
            .rotation;
        assert!(
            (t.translation - Vec3::new(1.0, 0.5, -1.0) * s * PACE).length() < 1e-4,
            "{} is off the path",
            t.translation
        );
        assert!(
            rotation_apart(t.rotation, expected) < 1e-4,
            "turned with its position"
        );
        assert!((t.rotation.length() - 1.0).abs() < 1e-5);
    }

    // Stopped: the copy comes to rest exactly where the server has it.
    moving.set(false);
    for _ in 0..90 {
        server.step(1, FRAME);
        client.step(1, FRAME);
    }
    let seen = copy(client.world(), id).expect("still there");
    assert_eq!(
        transform(client.world(), seen),
        transform(server.world(), mover)
    );
}
