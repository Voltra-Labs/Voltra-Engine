//! Times building the sprite batch for a level far larger than the screen.
//!
//! `cargo run --release -p voltra-scene --example batch_stress`
//!
//! A 512 × 64 tilemap layer (32 768 tiles) and 5 000 sprites spread over
//! 2 000 units, seen through a camera about 21 × 12 units across — a real
//! level, where almost everything is off screen. Reports the mean time to
//! build the batch and how many quads it holds — and then the same for the
//! whole layer in view, from nothing and through a [`TileMeshCache`]. Not a test: the counts are
//! what the tests pin.

use std::time::Instant;

use voltra_ecs::World;
use voltra_render::glam::{IVec2, Vec2};
use voltra_scene::sprite::sheets::Sheets;
use voltra_scene::{Sprite, SpriteBatch, Tile, TileMeshCache, Tilemap, Transform};

fn level() -> World {
    let mut world = World::new();
    let mut map = Tilemap::new(Vec2::ONE);
    for x in 0..512 {
        for y in 0..64 {
            map.set(IVec2::new(x - 256, y - 64), Some(Tile::new(0)));
        }
    }
    let layer = world.spawn();
    world.insert(layer, Transform::default());
    world.insert(layer, map);
    for i in 0..5000 {
        let sprite = world.spawn();
        world.insert(
            sprite,
            Transform::from_translation(Vec2::new(i as f32 * 0.4 - 1000.0, (i % 7) as f32)),
        );
        world.insert(sprite, Sprite::default());
    }
    world
}

fn main() {
    let world = level();
    // What the draw path hands the batch: a camera at the origin and the view
    // it sees. Before culling, every quad in the world was built.
    let sheets = voltra_scene_view(Sheets::default());
    let runs = 100;
    let mut quads = 0;
    let start = Instant::now();
    for _ in 0..runs {
        let batch = SpriteBatch::from_world(&world, sheets);
        quads = batch.sprite_count();
        std::hint::black_box(batch);
    }
    let ms = start.elapsed().as_secs_f64() * 1000.0 / runs as f64;
    println!("batch {ms:7.3} ms   quads {quads}");

    // The parts of it that do not depend on what is in view.
    let start = Instant::now();
    for _ in 0..runs {
        std::hint::black_box(voltra_scene::WorldTransforms::from_world(&world));
    }
    let ms = start.elapsed().as_secs_f64() * 1000.0 / runs as f64;
    println!("  of which composing transforms {ms:7.3} ms");

    // Zoomed out over the whole tilemap layer: every tile in view, which is
    // where rebuilding the layer's quads each frame is the cost.
    let map_only = tilemap_only();
    let everything =
        Sheets::default().visible_in(Vec2::new(-300.0, -100.0), Vec2::new(300.0, 100.0));
    let start = Instant::now();
    for _ in 0..runs {
        let batch = SpriteBatch::from_world(&map_only, everything);
        quads = batch.sprite_count();
        std::hint::black_box(batch);
    }
    let ms = start.elapsed().as_secs_f64() * 1000.0 / runs as f64;
    println!("whole layer in view {ms:7.3} ms   quads {quads}");

    // The same, as the draw path builds it: chunks kept between frames,
    // and the batch's buffers too.
    let mut cache = TileMeshCache::default();
    let mut batch = SpriteBatch::default();
    let start = Instant::now();
    for _ in 0..runs {
        batch.rebuild(&map_only, everything, &mut cache);
        std::hint::black_box(&batch);
    }
    let ms = start.elapsed().as_secs_f64() * 1000.0 / runs as f64;
    println!("  cached            {ms:7.3} ms");

    let mut cache = TileMeshCache::default();
    let start = Instant::now();
    for _ in 0..runs {
        batch.rebuild(&world, sheets, &mut cache);
        std::hint::black_box(&batch);
    }
    let ms = start.elapsed().as_secs_f64() * 1000.0 / runs as f64;
    println!("level, cached       {ms:7.3} ms");
}

/// The level's tilemap layer and nothing else.
fn tilemap_only() -> World {
    let mut world = level();
    let sprites: Vec<_> = world.query::<Sprite>().map(|(e, _)| e).collect();
    for e in sprites {
        world.despawn(e);
    }
    world
}

/// The view a 16:9 camera about 21 units wide sees from the origin.
fn voltra_scene_view(sheets: Sheets<'_>) -> Sheets<'_> {
    let half = Vec2::new(10.67, 6.0);
    sheets.visible_in(-half, half)
}
