//! The geometry a tilemap layer puts in the sprite batch.

use voltra_assets::{AssetPath, Atlases, Handle, TextureSizes};
use voltra_ecs::World;
use voltra_render::glam::{IVec2, Mat3, UVec2, Vec2};
use voltra_render::Texture;
use voltra_testkit::scratch_root;

use crate::batch::SpriteBatch;
use crate::sprite::sheets::Sheets;
use crate::{Sprite, Tile, Tilemap, Transform};

struct Sizes;

fn sheet() -> Handle<Texture> {
    Handle::forge(7, 0)
}

fn placeholder() -> Handle<Texture> {
    Handle::forge(0, 0)
}

impl TextureSizes for Sizes {
    fn size(&self, _texture: Handle<Texture>) -> Option<UVec2> {
        Some(UVec2::new(64, 16))
    }

    fn placeholder(&self) -> Handle<Texture> {
        placeholder()
    }
}

/// Four 16×16 tiles in a row.
fn atlases() -> (Atlases, AssetPath) {
    let root = scratch_root().keep();
    std::fs::write(
        root.join("ground.atlas.ron"),
        "(version: 1, grid: Some((cell: (16, 16), columns: 4, rows: 1)))",
    )
    .expect("the fixture writes");
    (
        Atlases::new(&root),
        AssetPath::new("ground.atlas.ron").expect("valid"),
    )
}

fn layer(atlases: &mut Atlases, path: &AssetPath) -> Tilemap {
    let mut map = Tilemap::default();
    map.set_atlas(Some(path.clone()), atlases);
    map.texture_handle = Some(sheet());
    map
}

fn mesh(map: &Tilemap, atlases: &Atlases) -> SpriteBatch {
    let mut batch = SpriteBatch::default();
    super::mesh::push(
        &mut batch,
        Mat3::IDENTITY,
        map,
        Sheets::new(atlases, &Sizes),
    );
    batch
}

fn positions(batch: &SpriteBatch) -> Vec<[f32; 2]> {
    batch.quad_vertices().iter().map(|v| v.position).collect()
}

#[test]
fn an_empty_layer_draws_nothing() {
    let (mut atlases, path) = atlases();

    assert!(mesh(&layer(&mut atlases, &path), &atlases).is_empty());
}

#[test]
fn a_tile_covers_its_cell_from_the_lower_left_corner() {
    let (mut atlases, path) = atlases();
    let mut map = layer(&mut atlases, &path);
    map.tile_size = Vec2::new(0.5, 2.0);
    map.set(IVec2::new(2, -1), Some(Tile::new(0)));

    let batch = mesh(&map, &atlases);

    assert_eq!(
        positions(&batch),
        [[1.0, 0.0], [1.0, -2.0], [1.5, -2.0], [1.5, 0.0]]
    );
}

#[test]
fn a_tile_samples_its_frame_of_the_sheet() {
    let (mut atlases, path) = atlases();
    let mut map = layer(&mut atlases, &path);
    map.set(IVec2::ZERO, Some(Tile::new(2)));

    let batch = mesh(&map, &atlases);

    let us: Vec<f32> = batch.quad_vertices().iter().map(|v| v.uv[0]).collect();
    assert_eq!(us, [0.5, 0.5, 0.75, 0.75]);
    assert_eq!(batch.ranges[0].texture, Some(sheet()));
}

#[test]
fn a_flipped_tile_swaps_its_uvs() {
    let (mut atlases, path) = atlases();
    let mut map = layer(&mut atlases, &path);
    map.set(
        IVec2::ZERO,
        Some(Tile {
            frame: 0,
            flip_x: true,
            flip_y: true,
            transpose: false,
        }),
    );

    let batch = mesh(&map, &atlases);

    assert_eq!(
        batch.quad_vertices()[0].uv,
        [0.25, 1.0],
        "top-left samples bottom-right"
    );
}

#[test]
fn a_frame_the_atlas_lacks_draws_the_placeholder_not_nothing() {
    let (mut atlases, path) = atlases();
    let mut map = layer(&mut atlases, &path);
    map.set(IVec2::ZERO, Some(Tile::new(0)));
    map.set(IVec2::new(1, 0), Some(Tile::new(99)));

    let batch = mesh(&map, &atlases);

    assert_eq!(batch.sprite_count(), 2);
    let textures: Vec<_> = batch.ranges.iter().map(|r| r.texture).collect();
    assert_eq!(textures, [Some(sheet()), Some(placeholder())]);
}

#[test]
fn a_whole_layer_is_one_draw_range() {
    let (mut atlases, path) = atlases();
    let mut map = layer(&mut atlases, &path);
    for x in -20..20 {
        map.set(IVec2::new(x, 0), Some(Tile::new((x.rem_euclid(4)) as u32)));
    }

    let batch = mesh(&map, &atlases);

    assert_eq!(batch.sprite_count(), 40);
    assert_eq!(batch.ranges.len(), 1);
}

#[test]
fn a_layer_draws_among_sprites_by_sort_order() {
    // Ground behind (-1), a sprite on 0, foliage in front (1): the batch has
    // to come out in that order whatever order they were spawned in.
    let (mut atlases, path) = atlases();
    let mut world = World::new();

    let mut front = layer(&mut atlases, &path);
    front.sort_order = 1;
    front.set(IVec2::new(10, 0), Some(Tile::new(0)));
    let mut back = layer(&mut atlases, &path);
    back.sort_order = -1;
    back.set(IVec2::new(-10, 0), Some(Tile::new(0)));

    for map in [front, back] {
        let e = world.spawn();
        world.insert(e, Transform::default());
        world.insert(e, map);
    }
    let e = world.spawn();
    world.insert(e, Transform::default());
    world.insert(e, Sprite::default());

    let batch = SpriteBatch::from_world(&world, Sheets::new(&atlases, &Sizes));

    let xs: Vec<f32> = batch
        .quad_vertices()
        .iter()
        .step_by(4)
        .map(|v| v.position[0])
        .collect();
    assert_eq!(xs, [-10.0, -0.5, 10.0]);
}

#[test]
fn a_layer_moves_with_its_transform() {
    let (mut atlases, path) = atlases();
    let mut map = layer(&mut atlases, &path);
    map.set(IVec2::ZERO, Some(Tile::new(0)));
    let mut world = World::new();
    let e = world.spawn();
    world.insert(e, Transform::from_translation(Vec2::new(5.0, 5.0)));
    world.insert(e, map);

    let batch = SpriteBatch::from_world(&world, Sheets::new(&atlases, &Sizes));

    assert_eq!(batch.quad_vertices()[1].position, [5.0, 5.0]);
}

#[test]
fn a_layer_without_a_transform_is_not_drawn() {
    let (mut atlases, path) = atlases();
    let mut map = layer(&mut atlases, &path);
    map.set(IVec2::ZERO, Some(Tile::new(0)));
    let mut world = World::new();
    let e = world.spawn();
    world.insert(e, map);

    assert!(SpriteBatch::from_world(&world, Sheets::new(&atlases, &Sizes)).is_empty());
}

#[test]
fn an_animated_frame_samples_the_frame_its_clock_is_on() {
    use crate::tilemap::TileAnimation;

    let (mut atlases, path) = atlases();
    let mut map = layer(&mut atlases, &path);
    map.set(IVec2::ZERO, Some(Tile::new(0)));
    map.animations.insert(
        0,
        TileAnimation {
            frames: vec![0, 2],
            frame_time: 0.5,
        },
    );
    let u_at = |time: f32| {
        let mut batch = SpriteBatch::default();
        super::mesh::push(
            &mut batch,
            Mat3::IDENTITY,
            &map,
            Sheets::new(&atlases, &Sizes).at_time(time),
        );
        batch.quad_vertices()[0].uv[0]
    };
    assert_eq!(u_at(0.0), 0.0, "frame 0 at the start");
    assert_eq!(u_at(0.75), 0.5, "frame 2 of four is half way across");
    assert_eq!(u_at(1.1), 0.0, "and round again");
}

#[test]
fn a_frame_with_no_animation_is_unchanged_by_the_clock() {
    let (mut atlases, path) = atlases();
    let mut map = layer(&mut atlases, &path);
    map.set(IVec2::ZERO, Some(Tile::new(1)));
    let mut batch = SpriteBatch::default();
    super::mesh::push(
        &mut batch,
        Mat3::IDENTITY,
        &map,
        Sheets::new(&atlases, &Sizes).at_time(123.0),
    );
    assert_eq!(batch.quad_vertices()[0].uv[0], 0.25);
}

#[test]
fn a_turned_tile_samples_its_frame_a_quarter_round() {
    let (mut atlases, path) = atlases();
    let mut map = layer(&mut atlases, &path);
    map.set(IVec2::ZERO, Some(Tile::new(0).rotated_cw()));

    let batch = mesh(&map, &atlases);

    // Frame 0 is u 0..0.25, v 0..1. Turned clockwise, the cell's top-left
    // shows what was the frame's bottom-left, and its top-right the
    // frame's top-left.
    assert_eq!(batch.quad_vertices()[0].uv, [0.0, 1.0], "top-left");
    assert_eq!(batch.quad_vertices()[3].uv, [0.0, 0.0], "top-right");
    assert_eq!(batch.quad_vertices()[2].uv, [0.25, 0.0], "bottom-right");
}

#[test]
fn animations_round_trip_and_an_old_layer_reads_without_them() {
    use crate::tilemap::TileAnimation;

    let mut map = Tilemap::default();
    map.animations.insert(
        3,
        TileAnimation {
            frames: vec![3, 4],
            frame_time: 0.2,
        },
    );
    let text = ron::to_string(&map).expect("writes");
    let back: Tilemap = ron::from_str(&text).expect("reads");
    assert_eq!(back.animations, map.animations);

    let plain = ron::to_string(&Tilemap::default()).expect("writes");
    assert!(!plain.contains("animations"), "nothing written when empty");
    let read: Tilemap = ron::from_str(&plain).expect("an old layer reads");
    assert!(read.animations.is_empty());
}
