use voltra_ecs::World;
use voltra_render::glam::{IVec2, Vec2};

use super::*;
use crate::format::{from_scene_file, to_scene_file};
use crate::{ComponentRegistry, SceneId};

fn cell(x: i32, y: i32) -> IVec2 {
    IVec2::new(x, y)
}

#[test]
fn an_empty_map_has_nothing_anywhere() {
    let map = Tilemap::default();

    assert_eq!(map.get(cell(0, 0)), None);
    assert!(map.is_empty());
    assert_eq!(map.bounds(), None);
}

#[test]
fn a_tile_set_is_the_tile_got() {
    let mut map = Tilemap::default();
    let tile = Tile {
        frame: 7,
        flip_x: true,
        flip_y: false,
    };

    assert_eq!(map.set(cell(3, 4), Some(tile)), None);

    assert_eq!(map.get(cell(3, 4)), Some(tile));
    assert_eq!(map.len(), 1);
}

#[test]
fn setting_returns_what_was_there() {
    let mut map = Tilemap::default();
    map.set(cell(0, 0), Some(Tile::new(1)));

    assert_eq!(map.set(cell(0, 0), Some(Tile::new(2))), Some(Tile::new(1)));
    assert_eq!(map.len(), 1, "a replace is not a second tile");
}

#[test]
fn negative_cells_live_in_their_own_chunk_not_the_origins() {
    // -1 rounds towards negative infinity: chunk -1, not chunk 0 where it
    // would overwrite cell 15 or alias cell 0.
    let mut map = Tilemap::default();
    map.set(cell(-1, -1), Some(Tile::new(1)));
    map.set(cell(0, 0), Some(Tile::new(2)));
    map.set(cell(CHUNK - 1, CHUNK - 1), Some(Tile::new(3)));

    assert_eq!(map.get(cell(-1, -1)), Some(Tile::new(1)));
    assert_eq!(map.get(cell(0, 0)), Some(Tile::new(2)));
    assert_eq!(map.get(cell(CHUNK - 1, CHUNK - 1)), Some(Tile::new(3)));
    assert_eq!(map.len(), 3);
}

#[test]
fn tiles_come_back_on_the_cells_they_were_put_on() {
    let mut map = Tilemap::default();
    let placed = [cell(-17, 3), cell(0, 0), cell(40, -2), cell(15, 16)];
    for (frame, &at) in placed.iter().enumerate() {
        map.set(at, Some(Tile::new(frame as u32)));
    }

    let mut found: Vec<_> = map.tiles().collect();
    found.sort_by_key(|(at, _)| (at.x, at.y));
    let mut expected: Vec<_> = placed
        .iter()
        .enumerate()
        .map(|(frame, &at)| (at, Tile::new(frame as u32)))
        .collect();
    expected.sort_by_key(|(at, _)| (at.x, at.y));
    assert_eq!(found, expected);
}

#[test]
fn erasing_the_last_tile_of_a_chunk_drops_the_chunk() {
    let mut map = Tilemap::default();
    map.set(cell(100, 100), Some(Tile::new(0)));

    assert_eq!(map.set(cell(100, 100), None), Some(Tile::new(0)));

    assert!(map.is_empty());
}

#[test]
fn erasing_an_empty_cell_does_nothing() {
    let mut map = Tilemap::default();

    assert_eq!(map.set(cell(5, 5), None), None);
    assert!(map.is_empty());
}

#[test]
fn a_frame_too_big_to_store_leaves_the_cell_alone() {
    let mut map = Tilemap::default();
    map.set(cell(0, 0), Some(Tile::new(3)));

    map.set(cell(0, 0), Some(Tile::new(Tile::MAX_FRAME + 1)));
    map.set(cell(1, 0), Some(Tile::new(u32::MAX)));

    assert_eq!(map.get(cell(0, 0)), Some(Tile::new(3)));
    assert_eq!(map.get(cell(1, 0)), None);
}

#[test]
fn the_largest_storable_frame_round_trips_with_both_flips() {
    let mut map = Tilemap::default();
    let tile = Tile {
        frame: Tile::MAX_FRAME,
        flip_x: true,
        flip_y: true,
    };

    map.set(cell(0, 0), Some(tile));

    assert_eq!(map.get(cell(0, 0)), Some(tile));
}

#[test]
fn bounds_span_every_tile() {
    let mut map = Tilemap::default();
    map.set(cell(-3, 2), Some(Tile::new(0)));
    map.set(cell(5, -7), Some(Tile::new(0)));

    assert_eq!(map.bounds(), Some((cell(-3, -7), cell(5, 2))));
}

#[test]
fn a_cell_starts_at_its_index_times_the_tile_size() {
    let map = Tilemap::new(Vec2::new(0.5, 2.0));

    assert_eq!(map.cell_origin(cell(-2, 3)), Vec2::new(-1.0, 6.0));
}

fn round_trip(map: &Tilemap) -> Tilemap {
    let registry = ComponentRegistry::with_defaults();
    let mut world = World::new();
    let entity = world.spawn();
    world.insert(entity, SceneId::new());
    world.insert(entity, map.clone());
    let file = to_scene_file(&world, &registry).expect("saves");

    let mut loaded = World::new();
    from_scene_file(&file, &registry, &mut loaded).expect("loads");
    let (_, map) = loaded.query::<Tilemap>().next().expect("one map");
    map.clone()
}

#[test]
fn a_map_survives_a_save_and_a_load() {
    let mut map = Tilemap::new(Vec2::new(0.5, 0.5));
    map.sort_order = -2;
    map.set(
        cell(-20, 1),
        Some(Tile {
            frame: 4,
            flip_x: false,
            flip_y: true,
        }),
    );
    map.set(cell(3, 3), Some(Tile::new(0)));

    assert_eq!(round_trip(&map), map);
}

#[test]
fn an_erased_map_saves_no_chunks() {
    let mut map = Tilemap::default();
    map.set(cell(0, 0), Some(Tile::new(1)));
    map.set(cell(0, 0), None);

    let text = ron::to_string(&map).expect("serializes");

    assert!(text.contains("chunks:[]"), "{text}");
}

#[test]
fn a_chunk_of_the_wrong_size_is_refused() {
    let text = "(chunks: [(at: (0, 0), cells: [1, 2, 3])])";

    let error = ron::from_str::<Tilemap>(text).expect_err("three cells");

    assert!(error.to_string().contains("256"), "{error}");
}

#[test]
fn a_chunk_listed_twice_is_refused() {
    let cells = vec!["1"; 256].join(",");
    let text =
        format!("(chunks: [(at: (0, 0), cells: [{cells}]), (at: (0, 0), cells: [{cells}])])");

    let error = ron::from_str::<Tilemap>(&text).expect_err("two copies");

    assert!(error.to_string().contains("twice"), "{error}");
}

#[test]
fn a_file_with_no_fields_is_an_empty_one_unit_map() {
    let map: Tilemap = ron::from_str("()").expect("every field defaults");

    assert_eq!(map, Tilemap::default());
}

#[test]
fn a_point_falls_in_the_cell_whose_lower_left_corner_is_below_it() {
    let map = Tilemap::new(Vec2::new(0.5, 2.0));

    assert_eq!(map.cell_at(Vec2::new(0.25, 1.0)), Some(cell(0, 0)));
    assert_eq!(map.cell_at(Vec2::new(0.5, 2.0)), Some(cell(1, 1)));
    assert_eq!(map.cell_at(Vec2::new(-0.01, -0.01)), Some(cell(-1, -1)));
}

#[test]
fn a_sizeless_tile_has_no_cell_under_any_point() {
    let map = Tilemap::new(Vec2::new(0.0, 1.0));
    assert_eq!(map.cell_at(Vec2::ZERO), None);
}

#[test]
fn editing_a_cell_moves_the_revision_and_a_no_op_does_not() {
    let mut map = Tilemap::default();
    let start = map.revision().get();

    map.set(cell(0, 0), Some(Tile::new(1)));
    let painted = map.revision().get();
    assert_ne!(painted, start);

    map.set(cell(0, 0), Some(Tile::new(1)));
    map.set(cell(9, 9), None);
    assert_eq!(map.revision().get(), painted, "nothing changed");

    map.clear();
    assert_ne!(map.revision().get(), painted);
}
