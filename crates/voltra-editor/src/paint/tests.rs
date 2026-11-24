use voltra_render::glam::{UVec2, Vec2};
use voltra_scene::tilemap::Terrain;
use voltra_scene::{Tile, Transform};

use super::*;

fn world_with_layer() -> (World, Entity, Entity) {
    let mut world = World::new();
    let layer = world.spawn();
    world.insert(layer, Transform::default());
    world.insert(layer, Tilemap::new(Vec2::ONE));
    let sprite = world.spawn();
    world.insert(sprite, Transform::default());
    (world, layer, sprite)
}

fn map(world: &World, layer: Entity) -> &Tilemap {
    world.get::<Tilemap>(layer).expect("a layer")
}

fn frame_at(world: &World, layer: Entity, x: i32, y: i32) -> Option<u32> {
    map(world, layer)
        .get(IVec2::new(x, y))
        .map(|tile| tile.frame)
}

#[test]
fn an_unarmed_brush_targets_nothing() {
    let (world, layer, _) = world_with_layer();
    assert_eq!(Paint::default().target(&world, Some(layer)), None);
}

#[test]
fn an_armed_brush_targets_the_selected_layer_only() {
    let (mut world, layer, sprite) = world_with_layer();
    let mut paint = Paint::default();
    paint.set_tool(Some(PaintTool::Brush));

    assert_eq!(paint.target(&world, Some(layer)), Some(layer));
    assert_eq!(paint.target(&world, Some(sprite)), None);
    assert_eq!(paint.target(&world, None), None);

    world.despawn(layer);
    assert_eq!(paint.target(&world, Some(layer)), None, "a despawned layer");
}

#[test]
fn a_block_stamp_paints_where_it_is_clicked() {
    let (mut world, layer, _) = world_with_layer();
    let mut paint = Paint {
        stamp: Stamp::new(
            UVec2::new(3, 2),
            (10..16).map(|f| Some(Tile::new(f))).collect(),
        )
        .expect("a block"),
        ..Paint::default()
    };

    let label = paint.stroke_through(
        &mut world,
        layer,
        PaintTool::Brush,
        false,
        &[IVec2::new(2, 3)],
    );

    assert_eq!(label, Some("Paint tiles"));
    assert_eq!(map(&world, layer).len(), 6);
    assert_eq!(frame_at(&world, layer, 2, 3), Some(10));
    assert_eq!(frame_at(&world, layer, 4, 4), Some(15));
}

#[test]
fn a_rectangle_repeats_the_stamp_over_it() {
    let (mut world, layer, _) = world_with_layer();
    let mut paint = Paint {
        stamp: Stamp::new(
            UVec2::new(2, 1),
            vec![Some(Tile::new(1)), Some(Tile::new(2))],
        )
        .expect("a pair"),
        ..Paint::default()
    };

    paint.stroke_through(
        &mut world,
        layer,
        PaintTool::Rectangle,
        false,
        &[IVec2::ZERO, IVec2::new(1, 1), IVec2::new(3, 1)],
    );

    assert_eq!(map(&world, layer).len(), 8);
    let row: Vec<Option<u32>> = (0..4).map(|x| frame_at(&world, layer, x, 1)).collect();
    assert_eq!(row, [Some(1), Some(2), Some(1), Some(2)]);
}

#[test]
fn the_bucket_fills_a_closed_area_only() {
    let (mut world, layer, _) = world_with_layer();
    let walls = world.get_mut::<Tilemap>(layer).expect("a layer");
    for i in 0..5 {
        for cell in [
            IVec2::new(i, 0),
            IVec2::new(i, 4),
            IVec2::new(0, i),
            IVec2::new(4, i),
        ] {
            walls.set(cell, Some(Tile::new(1)));
        }
    }
    let mut paint = Paint {
        stamp: Stamp::single(Tile::new(7)),
        ..Paint::default()
    };

    let label = paint.stroke_through(
        &mut world,
        layer,
        PaintTool::Fill,
        false,
        &[IVec2::new(2, 2)],
    );

    assert_eq!(label, Some("Fill area"));
    let filled = map(&world, layer)
        .tiles()
        .filter(|(_, t)| t.frame == 7)
        .count();
    assert_eq!(filled, 9, "the 3×3 inside");
    assert_eq!(frame_at(&world, layer, 0, 2), Some(1), "the walls stay");
}

#[test]
fn the_bucket_with_a_terrain_joins_it_up() {
    let (mut world, layer, _) = world_with_layer();
    let layer_map = world.get_mut::<Tilemap>(layer).expect("a layer");
    layer_map.terrains.push(Terrain {
        name: "grass".to_owned(),
        frames: std::array::from_fn(|mask| 100 + mask as u32),
        blob: Vec::new(),
    });
    layer_map.set(IVec2::new(0, 0), Some(Tile::new(5)));
    layer_map.set(IVec2::new(2, 0), Some(Tile::new(5)));
    layer_map.set(IVec2::new(1, 0), Some(Tile::new(5)));
    let mut paint = Paint {
        terrain: Some(0),
        ..Paint::default()
    };

    paint.stroke_through(
        &mut world,
        layer,
        PaintTool::Fill,
        false,
        &[IVec2::new(1, 0)],
    );

    let row: Vec<Option<u32>> = (0..3).map(|x| frame_at(&world, layer, x, 0)).collect();
    // East only, east and west, west only: auto-tiled after the fill.
    assert_eq!(row, [Some(102), Some(110), Some(108)]);
}

#[test]
fn the_eyedropper_picks_a_tile_and_paints_nothing() {
    let (mut world, layer, _) = world_with_layer();
    let tile = Tile::new(4).rotated_cw();
    world
        .get_mut::<Tilemap>(layer)
        .expect("a layer")
        .set(IVec2::new(5, 5), Some(tile));
    let before = map(&world, layer).clone();
    let mut paint = Paint::default();

    let label = paint.stroke_through(
        &mut world,
        layer,
        PaintTool::Brush,
        true,
        &[IVec2::new(5, 5)],
    );

    assert_eq!(label, None, "no undo entry: no cell changed");
    assert_eq!(map(&world, layer), &before);
    assert_eq!(
        paint.stamp.single_tile(),
        Some(tile),
        "picked, turned as it was"
    );
}

#[test]
fn the_eyedropper_dragged_picks_a_block() {
    let (mut world, layer, _) = world_with_layer();
    let layer_map = world.get_mut::<Tilemap>(layer).expect("a layer");
    layer_map.set(IVec2::new(0, 0), Some(Tile::new(1)));
    layer_map.set(IVec2::new(1, 1), Some(Tile::new(2)));
    let mut paint = Paint::default();

    paint.stroke_through(
        &mut world,
        layer,
        PaintTool::Eraser,
        true,
        &[IVec2::new(1, 1), IVec2::ZERO],
    );

    assert_eq!(paint.stamp.size(), UVec2::new(2, 2));
    assert_eq!(paint.stamp.get(UVec2::new(1, 1)), Some(Tile::new(2)));
    assert_eq!(
        paint.stamp.get(UVec2::new(1, 0)),
        None,
        "an empty cell is a hole"
    );
}
