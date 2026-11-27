//! Lag compensation at a hundred lag-compensated colliders: a rewound ray
//! (the past poses put together, then the ray), the same ray at the live
//! world, and a tick's recording.

use voltra_ecs::World;
use voltra_physics::{query, PoseHistory, QueryFilter};
use voltra_render::glam::Vec2;
use voltra_scene::{Collider, LagCompensated, Transform};

use super::Bench;
use crate::harness::{iter, measure};

const ENTITIES: usize = 100;
/// A second of ticks at 60 a second: the default history.
const TICKS: u32 = 60;
/// A shot rewound eight and a half ticks: a round trip and a buffer.
const BACK: f64 = 8.5;

pub(super) fn benches() -> Vec<Bench> {
    vec![
        Bench::new("lag/rewound-ray/100-entities", 15.0, |config| {
            let (world, history) = recorded();
            let at = f64::from(TICKS) - BACK;
            measure(
                config,
                1,
                iter(|| {
                    history.rewound(&world, at).ray(
                        Vec2::new(-60.0, 0.5),
                        Vec2::X,
                        120.0,
                        QueryFilter::new(),
                    )
                }),
            )
            .into()
        }),
        // Five microseconds: three runs spread 40 % on a 4-core container
        // (stage 130), so the limit is the comparer's suggestion.
        Bench::new("lag/live-ray/100-entities", 85.0, |config| {
            let (world, _) = recorded();
            measure(
                config,
                1,
                iter(|| {
                    query::ray(
                        &world,
                        Vec2::new(-60.0, 0.5),
                        Vec2::X,
                        120.0,
                        QueryFilter::new(),
                    )
                }),
            )
            .into()
        }),
        Bench::new("lag/record/100-entities", 15.0, |config| {
            let (world, mut history) = recorded();
            let mut tick = TICKS;
            measure(
                config,
                1,
                iter(|| {
                    tick += 1;
                    history.record(&world, tick);
                }),
            )
            .into()
        }),
    ]
}

/// A hundred lag-compensated boxes in a row, moved and recorded for a
/// second's ticks, and a few walls that are not.
fn recorded() -> (World, PoseHistory) {
    let mut world = World::new();
    let movers: Vec<_> = (0..ENTITIES)
        .map(|n| {
            let entity = world.spawn();
            world.insert(
                entity,
                Transform::from_translation(Vec2::new(n as f32 - 50.0, 0.0)),
            );
            world.insert(
                entity,
                Collider::Box {
                    half_extents: Vec2::splat(0.3),
                },
            );
            world.insert(entity, LagCompensated);
            entity
        })
        .collect();
    for n in 0..10 {
        let wall = world.spawn();
        world.insert(
            wall,
            Transform::from_translation(Vec2::new(n as f32 * 10.0 - 50.0, 20.0)),
        );
        world.insert(wall, Collider::default());
    }
    let mut history = PoseHistory::new(TICKS as usize + 1);
    for tick in 1..=TICKS {
        for (n, &entity) in movers.iter().enumerate() {
            if let Some(t) = world.get_mut::<Transform>(entity) {
                t.translation.y = (tick as f32 * 0.1 + n as f32).sin();
            }
        }
        history.record(&world, tick);
    }
    (world, history)
}
