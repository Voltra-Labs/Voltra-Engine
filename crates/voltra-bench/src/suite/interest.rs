//! Interest management at a thousand replicated entities and eight
//! clients: the grid's rebuild and queries, the per-pair distance check
//! the grid replaces, and a whole server frame with interest on and off.

use std::collections::HashSet;
use std::time::{Duration, Instant};

use voltra_core::net::{MemoryNetwork, NetConfig, PeerId};
use voltra_core::replication::{Interest, InterestGrid, NetOwner, ReplicationConfig};
use voltra_core::{App, Headless};
use voltra_ecs::Entity;
use voltra_render::glam::Vec2;
use voltra_scene::{Replicated, SceneId, Transform};

use super::Bench;
use crate::harness::{iter, measure, Config};

const ENTITIES: usize = 1000;
const CLIENTS: usize = 8;
/// The map's side: a thousand entities over it are a few dozen per
/// client's neighbourhood.
const MAP: f32 = 400.0;
const RADIUS: f32 = 30.0;
const FRAME: f32 = 1.0 / 60.0;

pub(super) fn benches() -> Vec<Bench> {
    vec![
        Bench::new("interest/grid/1k-entities-8-clients", 15.0, |config| {
            grid(config, CLIENTS)
        }),
        Bench::new("interest/per-pair/1k-entities-8-clients", 15.0, |config| {
            per_pair(config, CLIENTS)
        }),
        // Where the two cross: the grid's rebuild is flat in clients, the
        // pairs grow with them.
        Bench::new("interest/grid/1k-entities-64-clients", 15.0, |config| {
            grid(config, 64)
        }),
        Bench::new("interest/per-pair/1k-entities-64-clients", 15.0, |config| {
            per_pair(config, 64)
        }),
        Bench::new(
            "interest/server-frame/1k-entities-8-clients",
            15.0,
            |config| server_frame(config, Some(Interest::new(RADIUS))),
        ),
        Bench::new(
            "interest/server-frame-without/1k-entities-8-clients",
            15.0,
            |config| server_frame(config, None),
        ),
    ]
}

/// The grid rebuilt and read by `clients` clients, each with one point and
/// the hysteresis set it had.
fn grid(config: &Config, clients: usize) -> super::Outcome {
    let (entities, points) = layout(clients);
    let mut grid = InterestGrid::new(Interest::new(RADIUS));
    let mut previous = vec![HashSet::new(); clients];
    measure(
        config,
        1,
        iter(|| {
            grid.clear();
            for (id, at) in entities.iter().enumerate() {
                grid.insert(id as u32 + 1, *at, None);
            }
            for (point, seen) in points.iter().zip(&mut previous) {
                let mut now = HashSet::with_capacity(seen.len());
                grid.relevant(std::slice::from_ref(point), seen, &mut now);
                *seen = now;
            }
        }),
    )
    .into()
}

/// The same answer by every pair's distance: what the grid replaces.
fn per_pair(config: &Config, clients: usize) -> super::Outcome {
    let (entities, points) = layout(clients);
    let interest = Interest::new(RADIUS);
    let enter = RADIUS * RADIUS;
    let leave = interest.leave_radius(RADIUS).powi(2);
    let mut previous = vec![HashSet::new(); clients];
    measure(
        config,
        1,
        iter(|| {
            for (point, seen) in points.iter().zip(&mut previous) {
                let mut now = HashSet::with_capacity(seen.len());
                for (id, at) in entities.iter().enumerate() {
                    let id = id as u32 + 1;
                    let distance = (*at - *point).length_squared();
                    if distance <= enter || (distance <= leave && seen.contains(&id)) {
                        now.insert(id);
                    }
                }
                *seen = now;
            }
        }),
    )
    .into()
}

/// A thousand entities scattered over the map, and a point per client.
fn layout(clients: usize) -> (Vec<Vec2>, Vec<Vec2>) {
    let mut seed = 0x2545_f491_u32;
    let mut next = move || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        seed as f32 / u32::MAX as f32 * MAP - MAP / 2.0
    };
    let entities = (0..ENTITIES).map(|_| Vec2::new(next(), next())).collect();
    let points = (0..clients).map(|_| Vec2::new(next(), next())).collect();
    (entities, points)
}

/// One frame of a server replicating a thousand moving entities to eight
/// clients, each owning one of them: the world read, relevance, eight
/// snapshots encoded and sent. The clients step outside the clock, so the
/// server works from their acknowledgements as it would in a match.
fn server_frame(config: &Config, interest: Option<Interest>) -> super::Outcome {
    let replication = ReplicationConfig {
        interest,
        ..ReplicationConfig::default()
    };
    let network = MemoryNetwork::new();
    let transport = network.endpoint();
    let address = transport.address();
    let mut server = Headless::new(App::default().with_replication(replication.clone()));
    server.net_mut().host_on(transport, NetConfig::default());
    let mut clients: Vec<Headless> = (0..CLIENTS)
        .map(|_| {
            let mut client = Headless::new(App::default().with_replication(replication.clone()));
            client
                .net_mut()
                .join_on(network.endpoint(), address, NetConfig::default());
            client
        })
        .collect();
    for _ in 0..30 {
        server.step(1, FRAME);
        for client in &mut clients {
            client.step(1, FRAME);
        }
    }
    let peers: Vec<PeerId> = clients.iter().filter_map(|c| c.net().local_id()).collect();
    if peers.len() != CLIENTS {
        return super::Outcome::Skipped("the clients did not all join".to_owned());
    }
    let (spots, _) = layout(CLIENTS);
    let movers: Vec<Entity> = spots
        .iter()
        .enumerate()
        .map(|(n, at)| {
            let world = server.world_mut();
            let entity = world.spawn();
            world.insert(entity, SceneId::new());
            world.insert(entity, Transform::from_translation(*at));
            world.insert(entity, Replicated::default());
            if let Some(&peer) = peers.get(n) {
                world.insert(entity, NetOwner(peer));
            }
            entity
        })
        .collect();

    let mut frame = 0u32;
    let mut run = |iterations: u64| {
        let mut timed = Duration::ZERO;
        for _ in 0..iterations {
            frame += 1;
            let world = server.world_mut();
            for (n, &entity) in movers.iter().enumerate() {
                if let Some(t) = world.get_mut::<Transform>(entity) {
                    t.translation =
                        spots[n] + Vec2::new((frame as f32 * 0.05 + n as f32).sin(), 0.0);
                }
            }
            let start = Instant::now();
            server.step(1, FRAME);
            timed += start.elapsed();
            for client in &mut clients {
                client.step(1, FRAME);
            }
        }
        timed
    };
    measure(config, 1, &mut run).into()
}
