//! A predicted 3D entity (stage 134): reconciled with the server as a 2D
//! one is, its correction measured as a distance in space.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use voltra_core::net::{Conditioned, LinkConditions, MemoryNetwork, NetConfig, NetEvent, PeerId};
use voltra_core::replication::{NetOwner, Prediction, ReplicationConfig};
use voltra_core::{App, Headless, Net};
use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec3;
use voltra_scene::{Replicated, SceneId, Transform3D};

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

fn transform(world: &World, entity: Entity) -> Transform3D {
    *world.get::<Transform3D>(entity).expect("a 3D transform")
}

/// What the player holds: a direction on X and Z.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
struct Stick {
    x: i8,
    z: i8,
}

const SPEED: f32 = 3.0;

fn walk(world: &mut World, entity: Entity, stick: &Stick, step: f32) {
    if let Some(t) = world.get_mut::<Transform3D>(entity) {
        t.translation += Vec3::new(f32::from(stick.x), 0.0, f32::from(stick.z)) * SPEED * step;
    }
}

#[test]
fn a_predicted_3d_entity_is_corrected_and_its_error_measured_in_space() {
    let network = MemoryNetwork::new();
    let server_transport = link(7, &network, 50);
    let address = server_transport.inner().address();
    let mut server = Headless::new(
        App::default()
            .with_replication(ReplicationConfig::default())
            .with_update(|tick| {
                for event in tick.net.events().to_vec() {
                    if let NetEvent::Connected(peer) = event {
                        let world = &mut *tick.world;
                        let player = world.spawn();
                        world.insert(player, SceneId::new());
                        world.insert(player, Transform3D::default());
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
                    if let Some(stick) = tick.net.input::<Stick>(peer) {
                        walk(tick.world, entity, &stick, tick.delta);
                    }
                }
            }),
    );
    server
        .net_mut()
        .host_on(server_transport, NetConfig::default());
    let prediction = Rc::new(RefCell::new(Prediction::<Stick>::new()));
    let predicted = prediction.clone();
    let mut client = Headless::new(
        App::default()
            .with_replication(ReplicationConfig::default())
            .with_fixed_update(move |tick| {
                tick.predict(&mut predicted.borrow_mut(), Stick { x: 1, z: -1 }, walk);
            }),
    );
    client
        .net_mut()
        .join_on(link(8, &network, 50), address, NetConfig::default());
    for _ in 0..120 {
        server.step(1, FRAME);
        client.step(1, FRAME);
    }
    let stats = prediction.borrow().stats();
    assert!(stats.corrections > 30, "{stats:?}");
    assert!(
        stats.max_error < 1e-5,
        "the steady state shows nothing: {stats:?}"
    );

    // A push the server makes alone, 2 m up: corrected to, and measured.
    let peer = client.net().local_id().expect("joined");
    let theirs = Net::owned_by(server.world(), peer).expect("the server's copy");
    server
        .world_mut()
        .get_mut::<Transform3D>(theirs)
        .expect("a 3D transform")
        .translation
        .y = 2.0;
    for _ in 0..60 {
        server.step(1, FRAME);
        client.step(1, FRAME);
    }
    let mine = prediction.borrow().entity().expect("predicting");
    assert_eq!(
        transform(client.world(), mine).translation.y,
        2.0,
        "the server wins"
    );
    let max = prediction.borrow().stats().max_error;
    assert!(
        (max - 2.0).abs() < 1e-3,
        "a 2 m correction measured as {max} m"
    );
}
