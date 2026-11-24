//! A character walked up a slope made of tiles: a tilemap layer whose two
//! slope frames rise half a tile each, between a floor of full tiles and a
//! platform of them, its colliders generated as a game's level has them.

use voltra_ecs::{Entity, World};
use voltra_physics::move_and_slide;
use voltra_render::glam::{IVec2, Vec2};
use voltra_scene::tilemap::{sync_colliders, TileData, TileShape};
use voltra_scene::{CharacterBody2D, Collider, Tile, Tilemap, Transform};

const DT: f32 = 1.0 / 60.0;
const GRAVITY: f32 = -9.81;
const TILE: f32 = 0.5;

/// Full tiles along the bottom row; on the row above, a gentle slope over
/// cells 10 and 11 — up half a tile each — then a platform of full tiles.
/// The floor's top is at y = 0.5, the platform's at y = 1.0 from x = 6.
fn level(world: &mut World, mirrored: bool) {
    let mut map = Tilemap::new(Vec2::splat(TILE));
    map.collision = true;
    let low = TileShape::Slope {
        left: 0.0,
        right: 0.5,
    };
    let high = TileShape::Slope {
        left: 0.5,
        right: 1.0,
    };
    for (frame, shape) in [(1, low), (2, high)] {
        map.tile_data.insert(
            frame,
            TileData {
                shape,
                ..TileData::default()
            },
        );
    }
    let at = |x: i32| if mirrored { -1 - x } else { x };
    let turn = |tile: Tile| if mirrored { tile.mirrored_x() } else { tile };
    for x in 0..30 {
        map.set(IVec2::new(at(x), 0), Some(Tile::new(0)));
    }
    map.set(IVec2::new(at(10), 1), Some(turn(Tile::new(1))));
    map.set(IVec2::new(at(11), 1), Some(turn(Tile::new(2))));
    for x in 12..30 {
        map.set(IVec2::new(at(x), 1), Some(Tile::new(0)));
    }
    let layer = world.spawn();
    world.insert(layer, Transform::default());
    world.insert(layer, map);
    sync_colliders(world);
}

fn character(world: &mut World, at: Vec2) -> Entity {
    let entity = world.spawn();
    world.insert(entity, Transform::from_translation(at));
    world.insert(
        entity,
        Collider::Box {
            half_extents: Vec2::splat(0.2),
        },
    );
    world.insert(entity, CharacterBody2D::default());
    entity
}

/// Walks the character at `walk` units a second for `ticks`, falling, and
/// answers where it was each tick.
fn walk(world: &mut World, entity: Entity, walk: f32, ticks: u32) -> Vec<Vec2> {
    let mut velocity = Vec2::ZERO;
    let mut path = Vec::new();
    for _ in 0..ticks {
        velocity.y += GRAVITY * DT;
        velocity.x = walk;
        velocity = move_and_slide(world, entity, velocity, DT)
            .expect("a character")
            .velocity;
        path.push(world.get::<Transform>(entity).expect("placed").translation);
    }
    path
}

fn walks_up(mirrored: bool) {
    let mut world = World::new();
    level(&mut world, mirrored);
    let side = if mirrored { -1.0 } else { 1.0 };
    let me = character(&mut world, Vec2::new(2.0 * side, 0.75));

    let path = walk(&mut world, me, 2.0 * side, 300);

    let end = *path.last().expect("ticks");
    assert!(end.x * side > 7.0, "it got past the slope: {end}");
    assert!(
        (end.y - 1.2).abs() < 0.05,
        "standing on the platform, a half tile up: {end}"
    );
    let body = world.get::<CharacterBody2D>(me).expect("a character");
    assert!(body.contact.on_floor);
    // On the way, it stood on the slope itself: half way across the two
    // slope cells (x 5..6), where the surface is at 0.75, its feet between
    // the floor and the platform — on the slope, held up by its uphill
    // corner, not hovering at either level.
    let on_slope = path
        .iter()
        .find(|p| (p.x * side - 5.5).abs() < 0.05)
        .expect("it crossed the slope");
    let feet = on_slope.y - 0.2;
    assert!(
        feet > 0.7 && feet < 0.95,
        "feet on the slope's surface half way up: {on_slope}"
    );
}

#[test]
fn a_character_walks_up_a_slope_of_tiles() {
    walks_up(false);
}

#[test]
fn a_mirrored_slope_is_walked_up_the_other_way() {
    walks_up(true);
}
