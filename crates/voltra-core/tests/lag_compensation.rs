//! Lag compensation through `tick.net`: over 40 ms of latency each way, a
//! client aims at a target where it draws it — a round trip and a jitter
//! buffer behind where the server has it — and the server's rewound ray
//! hits it there while a ray at the live world misses. Slower links and
//! sparser snapshots still hit; a client claiming more than its measured
//! lag is held to it; bounded by `max_rewind`, off with the history, and
//! blind to what the client was not sent.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use voltra_core::net::{Conditioned, LinkConditions, MemoryNetwork, NetConfig, NetEvent, PeerId};
use voltra_core::replication::{
    Interest, LagCompensation, NetOwner, Prediction, ReplicationConfig, ViewTime,
};
use voltra_core::{App, Headless, QueryFilter, Rewound};
use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;
use voltra_scene::{Collider, LagCompensated, Replicated, SceneId, Transform};

const FRAME: f32 = 1.0 / 60.0;
/// The target's half-width: a shot further off than this misses.
const HALF: f32 = 0.25;
/// Where the far target sits, out of a small interest radius.
const FAR: f32 = 100.0;

/// What the client sends each step: where it sees the target, if it does.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
struct Aim {
    x: Option<f32>,
}

/// One shot as the server tested it.
#[derive(Debug, Clone, Copy)]
struct Shot {
    rewound: bool,
    live: bool,
    /// Ticks back from the newest held.
    back: f64,
    /// Whether the far target, out of the client's interest, was found.
    far: bool,
    /// Whether the far crate — replicated, with a collider, not
    /// lag-compensated — was found.
    far_crate: bool,
}

/// A ray straight up through `x`.
fn up(rewound: &Rewound<'_>, x: f32) -> Option<Entity> {
    rewound
        .ray(Vec2::new(x, -10.0), Vec2::Y, 20.0, QueryFilter::new())
        .map(|hit| hit.entity)
}

fn target(world: &mut World, x: f32) -> Entity {
    let entity = world.spawn();
    world.insert(entity, SceneId::new());
    world.insert(entity, Transform::from_translation(Vec2::new(x, 0.0)));
    world.insert(entity, Replicated::default());
    world.insert(
        entity,
        Collider::Box {
            half_extents: Vec2::splat(HALF),
        },
    );
    world.insert(entity, LagCompensated);
    entity
}

fn link(latency: u64) -> LinkConditions {
    LinkConditions {
        latency: Duration::from_millis(latency),
        jitter: Duration::ZERO,
        loss: 0.0,
        duplicate: 0.0,
        bandwidth: 0,
    }
}

/// A server and a client: the server moves a target back and forth along x
/// (up to 8 units a second) and tests every shot the client sends; the
/// client aims at the target where it draws it. Answers the shots after the
/// first two seconds. `latency` is each way.
fn play(config: ReplicationConfig, latency: u64) -> Vec<Shot> {
    let network = MemoryNetwork::new();
    let server_transport = Conditioned::new(network.endpoint(), link(latency), 1);
    let address = server_transport.inner().address();

    let shots: Rc<RefCell<Vec<Shot>>> = Rc::default();
    let moving: Rc<RefCell<Option<(Entity, Entity, Entity)>>> = Rc::default();
    let (recorded, targets) = (shots.clone(), moving.clone());
    let mut time = 0.0_f32;
    let mut server = Headless::new(
        App::default()
            .with_replication(config.clone())
            .with_update(|tick| {
                for event in tick.net.events().to_vec() {
                    if let NetEvent::Connected(peer) = event {
                        let world = &mut *tick.world;
                        let player = world.spawn();
                        world.insert(player, SceneId::new());
                        world.insert(player, Transform::default());
                        world.insert(player, Replicated::default());
                        world.insert(player, NetOwner(peer));
                    }
                }
            })
            .with_fixed_update(move |tick| {
                let (near, far, far_crate) = *targets.borrow_mut().get_or_insert_with(|| {
                    let far_crate = target(tick.world, -FAR);
                    tick.world.remove::<LagCompensated>(far_crate);
                    (target(tick.world, 0.0), target(tick.world, FAR), far_crate)
                });
                let peers: Vec<_> = tick
                    .world
                    .query::<NetOwner>()
                    .map(|(_, owner)| owner.0)
                    .collect();
                for peer in peers {
                    let Some(Aim { x: Some(x) }) = tick.net.input::<Aim>(peer) else {
                        continue;
                    };
                    let newest = tick
                        .net
                        .pose_history()
                        .and_then(|h| h.newest())
                        .map_or(0.0, f64::from);
                    let rewound = tick.net.rewound(tick.world, peer);
                    let live = Rewound::live(tick.world);
                    recorded.borrow_mut().push(Shot {
                        rewound: up(&rewound, x) == Some(near),
                        live: up(&live, x) == Some(near),
                        back: tick.net.rewind_target(peer).map_or(0.0, |t| newest - t),
                        far: up(&rewound, FAR) == Some(far),
                        far_crate: up(&rewound, -FAR) == Some(far_crate),
                    });
                }
                time += tick.delta;
                if let Some(t) = tick.world.get_mut::<Transform>(near) {
                    t.translation.x = 4.0 * (time * 2.0).sin();
                }
            }),
    );
    server
        .net_mut()
        .host_on(server_transport, NetConfig::default());

    let prediction = Rc::new(RefCell::new(Prediction::<Aim>::new()));
    let predicted = prediction.clone();
    let mut client = Headless::new(App::default().with_replication(config).with_fixed_update(
        move |tick| {
            let mine = predicted.borrow().entity();
            // The target nearest the middle, where the client draws it.
            let x = tick
                .world
                .query::<Replicated>()
                .filter(|(entity, _)| Some(*entity) != mine)
                .filter_map(|(entity, _)| tick.world.get::<Transform>(entity))
                .map(|t| t.translation.x)
                .find(|x| x.abs() < FAR / 2.0);
            tick.predict(&mut predicted.borrow_mut(), Aim { x }, |_, _, _, _| {});
        },
    ));
    client.net_mut().join_on(
        Conditioned::new(network.endpoint(), link(latency), 2),
        address,
        NetConfig::default(),
    );

    for _ in 0..360 {
        server.step(1, FRAME);
        client.step(1, FRAME);
    }
    assert!(prediction.borrow().entity().is_some(), "the client joined");
    let all = shots.borrow().clone();
    all.into_iter().skip(120).collect()
}

#[test]
fn a_shot_hits_where_the_client_saw_the_target_and_not_where_it_is() {
    let shots = play(ReplicationConfig::default(), 40);
    assert!(shots.len() > 150, "{} shots", shots.len());
    // Rewound by the round trip and the jitter buffer: 80 ms and a
    // snapshot or two, inside the 200 ms bound.
    let back: f64 = shots.iter().map(|s| s.back).sum::<f64>() / shots.len() as f64;
    assert!((5.0..12.0).contains(&back), "rewound {back:.2} ticks");
    let rewound = shots.iter().filter(|s| s.rewound).count();
    let live = shots.iter().filter(|s| s.live).count();
    assert_eq!(rewound, shots.len(), "every shot hits where it was aimed");
    assert!(
        live * 2 < shots.len(),
        "most would miss the live world: {live} of {}",
        shots.len()
    );
    // Without interest management the far target was sent, and is there.
    assert!(shots.iter().all(|s| s.far && s.far_crate));
}

/// The share of `shots` the rewound ray hit.
fn hit_rate(shots: &[Shot]) -> f64 {
    shots.iter().filter(|s| s.rewound).count() as f64 / shots.len().max(1) as f64
}

#[test]
fn slower_links_and_sparser_snapshots_still_hit() {
    // Each way, and snapshots every so many ticks: a view lag at the old
    // 200 ms cap, past it, and a long round trip.
    for (latency, interval) in [(40, 3), (60, 3), (100, 1)] {
        let shots = play(
            ReplicationConfig {
                snapshot_interval: interval,
                ..ReplicationConfig::default()
            },
            latency,
        );
        assert!(shots.len() > 150, "{latency} ms, every {interval}");
        let rate = hit_rate(&shots);
        let deepest = shots.iter().map(|s| s.back).fold(0.0, f64::max);
        assert!(
            rate >= 0.99,
            "{latency} ms each way, a snapshot every {interval}: {:.1} % hit, \
             rewound up to {deepest:.1} ticks",
            rate * 100.0
        );
    }
}

#[test]
fn a_rewind_stops_at_max_rewind() {
    let shots = play(
        ReplicationConfig {
            lag_compensation: Some(LagCompensation {
                max_rewind: Duration::from_millis(50),
                ..LagCompensation::default()
            }),
            ..ReplicationConfig::default()
        },
        40,
    );
    // Three ticks back at most, where the client looked further.
    assert!(shots.iter().all(|s| s.back <= 3.0 + 1e-5), "{shots:?}");
    assert!(shots.iter().any(|s| s.back > 2.9));
    let rewound = shots.iter().filter(|s| s.rewound).count();
    assert!(
        rewound * 2 < shots.len(),
        "the bound costs the laggy shooter: {rewound} of {} hit",
        shots.len()
    );
}

#[test]
fn without_a_history_every_query_is_live() {
    let shots = play(
        ReplicationConfig {
            lag_compensation: None,
            ..ReplicationConfig::default()
        },
        40,
    );
    assert!(shots.iter().all(|s| s.rewound == s.live && s.back == 0.0));
}

#[test]
fn what_the_client_was_not_sent_it_cannot_hit() {
    let shots = play(
        ReplicationConfig {
            interest: Some(Interest::new(10.0)),
            ..ReplicationConfig::default()
        },
        40,
    );
    assert!(shots.iter().all(|s| !s.far), "the far target is hidden");
    assert!(
        shots.iter().all(|s| !s.far_crate),
        "and so is a far collider that is not lag-compensated"
    );
    assert!(shots.iter().all(|s| s.rewound), "the near one is not");
}

/// What a client's replication message is on the wire, written by hand:
/// postcard is positional, so this lays out the same bytes.
#[derive(Serialize)]
struct Forged {
    newest: u32,
    mask: u64,
    inputs: Vec<(u32, Option<ViewTime>, Vec<u8>)>,
    resync: Vec<u32>,
}

#[test]
fn a_client_claiming_the_cap_on_a_quick_link_is_held_to_its_lag() {
    let config = ReplicationConfig::default();
    let network = MemoryNetwork::new();
    let server_transport = network.endpoint();
    let address = server_transport.address();
    let backs: Rc<RefCell<Vec<(f64, Duration)>>> = Rc::default();
    let recorded = backs.clone();
    let mut server = Headless::new(
        App::default()
            .with_replication(config.clone())
            .with_fixed_update(move |tick| {
                let newest = tick
                    .net
                    .pose_history()
                    .and_then(|h| h.newest())
                    .map_or(0.0, f64::from);
                for peer in tick.net.peers().collect::<Vec<_>>() {
                    if tick.net.input::<Aim>(peer).is_none() {
                        continue;
                    }
                    if let (Some(target), Some(allowed)) =
                        (tick.net.rewind_target(peer), tick.net.rewind_allowed(peer))
                    {
                        recorded.borrow_mut().push((newest - target, allowed));
                    }
                }
            }),
    );
    server
        .net_mut()
        .host_on(server_transport, NetConfig::default());
    // No replication of its own: it speaks the wire by hand.
    let mut cheat = Headless::new(App::default());
    cheat.net_mut().join_on(
        network.endpoint(),
        address,
        config.net_config(NetConfig::default()),
    );
    let input = voltra_core::net::encode(&Aim { x: Some(0.0) }).expect("encodes");
    for tick in 1..=240u32 {
        server.step(1, FRAME);
        let now = server
            .net()
            .replication()
            .and_then(|r| r.server())
            .map_or(0, |s| s.tick());
        // Every input claims to have been made the whole cap ago: 30
        // ticks, half a second.
        let claim = ViewTime {
            tick: now.saturating_sub(30),
            fraction: 0,
        };
        let forged = Forged {
            newest: 0,
            mask: 0,
            inputs: vec![(tick, Some(claim), input.clone())],
            resync: Vec::new(),
        };
        let bytes = voltra_core::net::encode(&forged).expect("encodes");
        if cheat.net().local_id().is_some() {
            let _ = cheat
                .net_mut()
                .send(PeerId::SERVER, config.channels.input, &bytes);
        }
        cheat.step(1, FRAME);
    }
    let backs = backs.borrow();
    assert!(backs.len() > 100, "{} inputs simulated", backs.len());
    for &(back, allowed) in backs.iter().skip(30) {
        // A round trip of a frame or two, two intervals and the slack:
        // a little over 100 ms, six or seven ticks, never the cap's 30.
        assert!(allowed < Duration::from_millis(150), "{allowed:?}");
        assert!(
            back <= allowed.as_secs_f64() * 60.0 + 1e-6,
            "{back} for {allowed:?}"
        );
        assert!(back < 10.0, "held to its lag: {back} ticks back");
    }
}
