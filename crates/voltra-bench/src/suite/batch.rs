//! Sprite batching: a world of quads into instances and ranges, and the
//! upload that hands them to the renderer.

use voltra_assets::Handle;
use voltra_ecs::World;
use voltra_render::glam::Vec2;
use voltra_render::MeshBuffers;
use voltra_scene::sprite::sheets::Sheets;
use voltra_scene::{Sprite, SpriteBatch, TileMeshCache, Transform};

use super::{Bench, Outcome};
use crate::harness::{iter, measure};

/// Distinct textures the sprites are spread over: a level's sprites come
/// from a handful of sheets, so the batch splits into that many runs.
const TEXTURES: usize = 8;

pub(super) fn benches() -> Vec<Bench> {
    // Thresholds from the noise measured in stage 107 (docs/ARCHITECTURE.md):
    // 10k is short enough for a preempted slice to show.
    let mut benches: Vec<Bench> = [(10_000, 40.0), (50_000, 25.0), (100_000, 30.0)]
        .into_iter()
        .map(|(count, threshold)| {
            Bench::new(
                format!("batch/sprites/{}k", count / 1000),
                threshold,
                move |config| {
                    let world = sprites(count);
                    let mut batch = SpriteBatch::default();
                    let mut tiles = TileMeshCache::default();
                    let stats = measure(
                        config,
                        1,
                        iter(|| batch.rebuild(&world, Sheets::default(), &mut tiles)),
                    );
                    debug_assert_eq!(batch.sprite_count(), count);
                    stats.into()
                },
            )
        })
        .collect();
    benches.push(Bench::new("batch/upload/100k", 20.0, upload));
    benches
}

/// `count` sprites in a square grid, every one in view (no camera culls
/// them), each run of `count / TEXTURES` sharing a texture — what the
/// draw path rebuilds every frame.
fn sprites(count: usize) -> World {
    let mut world = World::new();
    let side = (count as f32).sqrt().ceil() as usize;
    for i in 0..count {
        let entity = world.spawn();
        let at = Vec2::new((i % side) as f32, (i / side) as f32);
        world.insert(entity, Transform::from_translation(at));
        let texture = (i * TEXTURES / count) as u32;
        world.insert(
            entity,
            Sprite {
                // Never resolved: the batch only compares handles to split
                // its runs, and draws nothing.
                texture_handle: Some(Handle::forge(texture, 0)),
                ..Sprite::default()
            },
        );
    }
    world
}

/// 100k quads written to the device as the draw path writes them: into
/// buffers kept from the frame before (`SpriteBatch::upload_into`), then
/// submitted, as a frame's submission carries the writes. Stage 107 timed
/// fresh buffers made every frame (`SpriteBatch::upload`), which the draw
/// path no longer does (stage 108).
fn upload(config: &crate::harness::Config) -> Outcome {
    let Some((device, queue)) = voltra_testkit::headless_device() else {
        return Outcome::Skipped("no GPU adapter".to_owned());
    };
    let mut batch = SpriteBatch::default();
    batch.rebuild(
        &sprites(100_000),
        Sheets::default(),
        &mut TileMeshCache::default(),
    );
    let mut buffers = MeshBuffers::default();
    measure(
        config,
        1,
        iter(|| {
            let drawn = batch
                .upload_into(&device, &queue, &mut buffers)
                .map(|mesh| mesh.quad_count());
            queue.submit([]);
            // Lets the device free the writes' staging, as a frame's
            // submission would; without it they pile up unfreed.
            let _ = device.poll(voltra_render::wgpu::PollType::Poll);
            drawn
        }),
    )
    .into()
}
