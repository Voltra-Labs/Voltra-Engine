//! The tile mesh cache changes what a batch costs, never what it holds: every
//! frame built through it equals the same frame built from nothing.

use voltra_assets::{AssetPath, Atlases, Handle, TextureSizes};
use voltra_ecs::{Entity, World};
use voltra_render::glam::{IVec2, UVec2, Vec2};
use voltra_render::Texture;
use voltra_testkit::scratch_root;

use super::TileAnimation;
use crate::batch::SpriteBatch;
use crate::sprite::sheets::Sheets;
use crate::{Parallax, Tile, TileMeshCache, Tilemap, Transform};

struct Sizes;

impl TextureSizes for Sizes {
    fn size(&self, _texture: Handle<Texture>) -> Option<UVec2> {
        Some(UVec2::new(64, 16))
    }

    fn placeholder(&self) -> Handle<Texture> {
        Handle::forge(0, 0)
    }
}

const ATLAS: &str = "(version: 1, grid: Some((cell: (16, 16), columns: 4, rows: 1)))";

fn atlases() -> (Atlases, AssetPath, std::path::PathBuf) {
    let root = scratch_root();
    std::fs::write(root.join("ground.atlas.ron"), ATLAS).expect("the fixture writes");
    (
        Atlases::new(&root),
        AssetPath::new("ground.atlas.ron").expect("valid"),
        root,
    )
}

/// Two chunks across and two up of mixed frames, one of them animated.
fn level(atlases: &mut Atlases, path: &AssetPath) -> (World, Entity) {
    let mut map = Tilemap::default();
    map.set_atlas(Some(path.clone()), atlases);
    map.texture_handle = Some(Handle::forge(7, 0));
    for x in -16..16 {
        for y in -16..16 {
            map.set(IVec2::new(x, y), Some(Tile::new(((x + y) & 3) as u32)));
        }
    }
    map.animations.insert(
        3,
        TileAnimation {
            frames: vec![3, 1],
            frame_time: 0.5,
        },
    );
    let mut world = World::new();
    let layer = world.spawn();
    world.insert(layer, Transform::default());
    world.insert(layer, map);
    (world, layer)
}

/// Builds `sheets`' frame both ways and asserts they agree.
fn same(world: &World, sheets: Sheets<'_>, cache: &mut TileMeshCache) -> SpriteBatch {
    let cached = SpriteBatch::from_world_cached(world, sheets, cache);
    let fresh = SpriteBatch::from_world(world, sheets);
    assert_eq!(cached.quads, fresh.quads);
    assert_eq!(cached.vertices, fresh.vertices);
    assert_eq!(cached.indices, fresh.indices);
    assert_eq!(cached.ranges, fresh.ranges);
    cached
}

#[test]
fn a_cached_frame_equals_a_fresh_one_through_every_change() {
    let (mut atlases, path, root) = atlases();
    let (mut world, layer) = level(&mut atlases, &path);
    let mut cache = TileMeshCache::default();
    let all = Sheets::new(&atlases, &Sizes);

    // Built, then copied.
    assert_eq!(same(&world, all, &mut cache).sprite_count(), 32 * 32);
    same(&world, all, &mut cache);
    assert_eq!(cache.chunk_count(), 4);

    // A view cutting through chunks, so some quads are culled one by one.
    let part = all.visible_in(Vec2::new(-3.5, -20.0), Vec2::new(20.0, 2.5));
    let drawn = same(&world, part, &mut cache).sprite_count();
    assert!(drawn < 32 * 32, "{drawn}");

    // The clock moves an animated tile on.
    same(&world, all.at_time(0.75), &mut cache);

    // An edit, a move, a recolour, a resize.
    {
        let map = world.get_mut::<Tilemap>(layer).expect("spawned with one");
        map.set(IVec2::new(2, 2), None);
        map.set(IVec2::new(40, 0), Some(Tile::new(1)));
    }
    same(&world, all, &mut cache);
    world.insert(layer, Transform::from_translation(Vec2::new(3.0, -1.0)));
    same(&world, all, &mut cache);
    world.get_mut::<Tilemap>(layer).expect("spawned").color = [1.0, 0.5, 0.5, 0.5];
    same(&world, all, &mut cache);
    world.get_mut::<Tilemap>(layer).expect("spawned").tile_size = Vec2::splat(2.0);
    same(&world, all, &mut cache);

    // A new animation on a frame no chunk animated before.
    world
        .get_mut::<Tilemap>(layer)
        .expect("spawned")
        .animations
        .insert(
            0,
            TileAnimation {
                frames: vec![2],
                frame_time: 1.0,
            },
        );
    same(&world, all, &mut cache);

    // The atlas re-sliced on disk under the same handle.
    std::fs::write(
        root.join("ground.atlas.ron"),
        "(version: 1, grid: Some((cell: (8, 16), columns: 8, rows: 1)))",
    )
    .expect("the fixture rewrites");
    assert!(atlases.reload(&path));
    same(&world, Sheets::new(&atlases, &Sizes), &mut cache);
}

#[test]
fn what_a_build_did_not_draw_is_dropped() {
    let (mut atlases, path, _root) = atlases();
    let (mut world, layer) = level(&mut atlases, &path);
    let mut cache = TileMeshCache::default();
    let all = Sheets::new(&atlases, &Sizes);
    same(&world, all, &mut cache);
    assert_eq!(cache.chunk_count(), 4);

    // One chunk in view: the other three go.
    let corner = all.visible_in(Vec2::new(1.0, 1.0), Vec2::new(2.0, 2.0));
    same(&world, corner, &mut cache);
    assert_eq!(cache.chunk_count(), 1);

    // A despawned layer takes its chunks with it.
    world.despawn(layer);
    same(&world, all, &mut cache);
    assert_eq!(cache.chunk_count(), 0);
}

#[test]
fn each_copy_of_a_repeating_layer_keeps_its_own_chunks() {
    let (mut atlases, path, _root) = atlases();
    let (mut world, layer) = level(&mut atlases, &path);
    world.insert(
        layer,
        Parallax {
            repeat: Some(Vec2::new(32.0, 0.0)),
            ..Parallax::default()
        },
    );
    let mut cache = TileMeshCache::default();
    let wide =
        Sheets::new(&atlases, &Sizes).visible_in(Vec2::new(-40.0, -4.0), Vec2::new(40.0, 4.0));
    let first = same(&world, wide, &mut cache);
    let second = same(&world, wide, &mut cache);
    assert_eq!(first.vertices, second.vertices);
    assert!(cache.chunk_count() > 4, "{}", cache.chunk_count());
}

#[test]
fn a_rebuilt_batch_holds_only_this_frame() {
    let (mut atlases, path, _root) = atlases();
    let (world, _) = level(&mut atlases, &path);
    let mut cache = TileMeshCache::default();
    let all = Sheets::new(&atlases, &Sizes);
    let mut batch = SpriteBatch::default();
    batch.rebuild(&world, all, &mut cache);
    let part = all.visible_in(Vec2::new(-3.5, -20.0), Vec2::new(20.0, 2.5));
    batch.rebuild(&world, part, &mut cache);
    let fresh = SpriteBatch::from_world(&world, part);
    assert_eq!(batch.quads, fresh.quads);
    assert_eq!(batch.vertices, fresh.vertices);
    assert_eq!(batch.indices, fresh.indices);
    assert_eq!(batch.ranges, fresh.ranges);

    batch.rebuild(&World::new(), all, &mut cache);
    assert!(batch.is_empty() && batch.indices.is_empty() && batch.ranges.is_empty());
}

#[test]
fn a_layer_s_light_layers_reach_every_range_on_every_path() {
    let (mut atlases, path, _root) = atlases();
    let (mut world, layer) = level(&mut atlases, &path);
    if let Some(map) = world.get_mut::<Tilemap>(layer) {
        map.light_layers = 0b100;
    }
    let all = Sheets::new(&atlases, &Sizes);
    // Part of the level: chunks wholly inside, and chunks across the edge,
    // which go quad by quad.
    let part = all.visible_in(Vec2::new(-3.5, -20.0), Vec2::new(20.0, 2.5));
    let mut cache = TileMeshCache::default();
    for sheets in [all, part] {
        let cached = SpriteBatch::from_world_cached(&world, sheets, &mut cache);
        let fresh = SpriteBatch::from_world(&world, sheets);
        for batch in [&cached, &fresh] {
            assert!(!batch.ranges.is_empty());
            assert!(
                batch.ranges.iter().all(|r| r.light_layers == 0b100),
                "{:?}",
                batch.ranges
            );
        }
    }
}

#[test]
fn a_layer_s_normal_map_reaches_every_range_on_every_path() {
    let (mut atlases, path, _root) = atlases();
    let (mut world, layer) = level(&mut atlases, &path);
    let normal = Handle::forge(9, 0);
    let all = Sheets::new(&atlases, &Sizes);
    let part = all.visible_in(Vec2::new(-3.5, -20.0), Vec2::new(20.0, 2.5));
    let mut cache = TileMeshCache::default();
    // Built once flat, so the cached chunks exist before the map is given:
    // adding a normal map must reach them without a rebuild.
    same(&world, all, &mut cache);
    if let Some(map) = world.get_mut::<Tilemap>(layer) {
        map.normal_handle = Some(normal);
    }
    for sheets in [all, part] {
        let cached = SpriteBatch::from_world_cached(&world, sheets, &mut cache);
        let fresh = SpriteBatch::from_world(&world, sheets);
        for batch in [&cached, &fresh] {
            assert!(!batch.ranges.is_empty());
            assert!(
                batch.ranges.iter().all(|r| r.normal == Some(normal)),
                "{:?}",
                batch.ranges
            );
        }
        assert_eq!(cached.ranges, fresh.ranges);
    }
}

#[test]
fn a_tile_drawing_the_placeholder_is_flat() {
    let (mut atlases, path, _root) = atlases();
    let (mut world, layer) = level(&mut atlases, &path);
    let normal = Handle::forge(9, 0);
    if let Some(map) = world.get_mut::<Tilemap>(layer) {
        map.normal_handle = Some(normal);
        // A frame the four-frame atlas does not have.
        map.set(IVec2::new(0, 0), Some(Tile::new(40)));
    }
    let mut cache = TileMeshCache::default();
    let batch = same(&world, Sheets::new(&atlases, &Sizes), &mut cache);
    let placeholder = Some(Sizes.placeholder());
    let flat: Vec<_> = batch
        .ranges
        .iter()
        .filter(|r| r.texture == placeholder)
        .collect();
    assert!(!flat.is_empty(), "the missing frame draws the placeholder");
    assert!(flat.iter().all(|r| r.normal.is_none()), "{flat:?}");
    assert!(batch
        .ranges
        .iter()
        .filter(|r| r.texture != placeholder)
        .all(|r| r.normal == Some(normal)));
}
