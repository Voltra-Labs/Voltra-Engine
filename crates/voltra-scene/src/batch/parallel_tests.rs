//! Placing sprites on the pool changes how fast a batch is built, never
//! what it holds.

use voltra_ecs::World;
use voltra_jobs::JobPool;
use voltra_render::glam::{IVec2, Vec2};

use crate::sprite::sheets::Sheets;
use crate::{ParticleEmitter, Sprite, SpriteBatch, Tile, Tilemap, Transform};

/// Thousands of sprites over a wide strip, in several sort orders, with a
/// tilemap layer and an emitter between them, some rotated.
fn crowd() -> World {
    let mut world = World::new();
    for i in 0..3000 {
        let e = world.spawn();
        let mut transform =
            Transform::from_translation(Vec2::new(i as f32 * 0.37 - 500.0, (i % 11) as f32 - 5.0));
        transform.rotation = i as f32 * 0.1;
        world.insert(e, transform);
        world.insert(
            e,
            Sprite {
                sort_order: i % 5 - 2,
                color: [1.0, (i % 3) as f32 / 2.0, 0.5, 1.0],
                ..Sprite::default()
            },
        );
    }
    let layer = world.spawn();
    world.insert(layer, Transform::default());
    let mut map = Tilemap::new(Vec2::ONE);
    for x in -40..40 {
        map.set(IVec2::new(x, -3), Some(Tile::new(0)));
    }
    world.insert(layer, map);
    let emitter = world.spawn();
    world.insert(emitter, Transform::default());
    world.insert(emitter, ParticleEmitter::default());
    crate::particles::advance(&mut world, 0.5, false);
    world
}

fn assert_same(a: &SpriteBatch, b: &SpriteBatch) {
    assert_eq!(a.vertices, b.vertices);
    assert_eq!(a.indices, b.indices);
    assert_eq!(a.ranges, b.ranges);
}

#[test]
fn a_batch_built_on_the_pool_equals_one_built_alone() {
    let world = crowd();
    let pool = JobPool::new(3);
    for view in [
        None,
        Some((Vec2::new(-20.0, -6.0), Vec2::new(20.0, 6.0))),
        Some((Vec2::new(1000.0, 1000.0), Vec2::new(1001.0, 1001.0))),
    ] {
        let sheets = Sheets {
            visible: view,
            ..Sheets::default()
        };
        let alone = SpriteBatch::from_world(&world, sheets);
        let pooled = SpriteBatch::from_world(&world, sheets.with_jobs(&pool));
        assert_same(&alone, &pooled);
    }
}

#[test]
fn a_pool_with_no_workers_gives_the_same_batch() {
    let world = crowd();
    let pool = JobPool::new(0);
    let alone = SpriteBatch::from_world(&world, Sheets::default());
    let pooled = SpriteBatch::from_world(&world, Sheets::default().with_jobs(&pool));
    assert_same(&alone, &pooled);
}
