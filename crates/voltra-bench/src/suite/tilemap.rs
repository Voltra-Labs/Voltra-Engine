//! Tilemap meshing: a layer's cells into quads, from nothing and through
//! the chunk cache a frame keeps.

use voltra_ecs::World;
use voltra_render::glam::{IVec2, Mat3, Vec2};
use voltra_scene::sprite::sheets::Sheets;
use voltra_scene::tilemap::mesh;
use voltra_scene::{SpriteBatch, Tile, TileMeshCache, Tilemap, Transform2D};

use super::Bench;
use crate::harness::{iter, measure};

/// Cells along each side of the layer: 65 536 tiles, a large level wholly
/// in view (zoomed out in the editor, say).
const SIDE: i32 = 256;

pub(super) fn benches() -> Vec<Bench> {
    vec![
        Bench::new("tilemap/mesh/256x256", 15.0, |config| {
            let map = layer();
            measure(
                config,
                1,
                iter(|| {
                    let mut batch = SpriteBatch::default();
                    mesh::push(&mut batch, Mat3::IDENTITY, &map, Sheets::default());
                    batch
                }),
            )
            .into()
        }),
        Bench::new("tilemap/cached/256x256", 30.0, |config| {
            let mut world = World::new();
            let entity = world.spawn();
            world.insert(entity, Transform2D::default());
            world.insert(entity, layer());
            let mut batch = SpriteBatch::default();
            let mut tiles = TileMeshCache::default();
            // The first build fills the cache; what is timed is every frame
            // after it, where nothing changed.
            batch.rebuild(&world, Sheets::default(), &mut tiles);
            measure(
                config,
                1,
                iter(|| batch.rebuild(&world, Sheets::default(), &mut tiles)),
            )
            .into()
        }),
    ]
}

/// A full `SIDE` × `SIDE` layer, its frames varied as a real one's are.
fn layer() -> Tilemap {
    let mut map = Tilemap::new(Vec2::ONE);
    for x in 0..SIDE {
        for y in 0..SIDE {
            let frame = ((x * 7 + y * 3) % 16) as u32;
            map.set(
                IVec2::new(x - SIDE / 2, y - SIDE / 2),
                Some(Tile::new(frame)),
            );
        }
    }
    map
}
