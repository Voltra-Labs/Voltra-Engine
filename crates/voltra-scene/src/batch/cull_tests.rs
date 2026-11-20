//! What the camera cannot see is left out of the batch.

use voltra_ecs::World;
use voltra_render::glam::{IVec2, Vec2};

use crate::sprite::sheets::Sheets;
use crate::{Parallax, SceneId, Sprite, SpriteBatch, Tile, Tilemap, Transform};

/// A view of the box `-5..5` square.
fn view() -> Sheets<'static> {
    Sheets::default().visible_in(Vec2::splat(-5.0), Vec2::splat(5.0))
}

fn sprite_at(world: &mut World, at: Vec2) {
    let entity = world.spawn();
    world.insert(entity, Transform::from_translation(at));
    world.insert(entity, Sprite::default());
}

#[test]
fn a_sprite_off_screen_is_left_out_and_one_on_screen_kept() {
    let mut world = World::new();
    sprite_at(&mut world, Vec2::ZERO);
    sprite_at(&mut world, Vec2::new(50.0, 0.0));
    assert_eq!(SpriteBatch::from_world(&world, view()).sprite_count(), 1);
}

#[test]
fn a_sprite_half_on_screen_is_kept() {
    let mut world = World::new();
    sprite_at(&mut world, Vec2::new(5.3, 0.0));
    assert_eq!(SpriteBatch::from_world(&world, view()).sprite_count(), 1);
}

#[test]
fn a_rotated_quad_is_tested_where_its_corners_are() {
    // A long thin sprite whose centre is off screen but which, turned 90°,
    // reaches into it.
    let mut world = World::new();
    let entity = world.spawn();
    world.insert(
        entity,
        Transform::from_translation(Vec2::new(0.0, 8.0))
            .with_scale(Vec2::new(10.0, 1.0))
            .with_rotation(std::f32::consts::FRAC_PI_2),
    );
    world.insert(entity, Sprite::default());
    assert_eq!(SpriteBatch::from_world(&world, view()).sprite_count(), 1);
}

#[test]
fn no_view_keeps_everything() {
    let mut world = World::new();
    sprite_at(&mut world, Vec2::new(1e6, 0.0));
    assert_eq!(
        SpriteBatch::from_world(&world, Sheets::default()).sprite_count(),
        1
    );
}

#[test]
fn a_tilemap_draws_only_the_tiles_in_view() {
    let mut world = World::new();
    let mut map = Tilemap::new(Vec2::ONE);
    for x in -100..100 {
        map.set(IVec2::new(x, 0), Some(Tile::new(0)));
    }
    let layer = world.spawn();
    world.insert(layer, Transform::default());
    world.insert(layer, map);

    let drawn = SpriteBatch::from_world(&world, view()).sprite_count();
    // Cells -5..=4 cover -5..5; the tile whose edge touches 5 counts too.
    assert!((10..=12).contains(&drawn), "{drawn} tiles");
}

#[test]
fn a_parallax_layer_is_culled_where_it_is_drawn() {
    let mut world = World::new();
    let sky = world.spawn();
    world.insert(sky, SceneId::new());
    world.insert(sky, Transform::default());
    world.insert(sky, Sprite::default());
    world.insert(sky, Parallax { factor: Vec2::ZERO });
    // The camera is far right; the sky is pinned to it and so in view, though
    // its transform is far out of it.
    let far = Vec2::new(100.0, 0.0);
    let sheets = Sheets::default()
        .seen_from(far)
        .visible_in(far - Vec2::splat(5.0), far + Vec2::splat(5.0));
    assert_eq!(SpriteBatch::from_world(&world, sheets).sprite_count(), 1);
}

#[test]
fn a_box_with_a_non_finite_edge_is_kept_rather_than_guessed_away() {
    assert!(view().sees(Vec2::new(f32::NAN, 0.0), Vec2::ONE));
}
