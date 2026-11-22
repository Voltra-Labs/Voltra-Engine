//! Which entities a box drawn over the world takes in — the editor's
//! marquee.
//!
//! An entity with a shape — a sprite's quad, a text's box, a tilemap
//! layer's tiles — is in when all of it is inside the box, Figma's and
//! Inkscape's rule rather than Unity's "touches it": a box dragged across a
//! level must not take the ground layer every time it crosses a tile. An
//! entity with nothing drawn — a light, a camera, an emitter, an empty
//! parent — is in when its origin is. Screen-space HUD elements are not in
//! the world and never are.

use voltra_ecs::{Entity, World};
use voltra_render::glam::{IVec2, Mat3, Vec2};

use crate::hierarchy::WorldTransforms;
use crate::scene_id::SceneId;
use crate::sprite::quad::quad;
use crate::sprite::sheets::Sheets;
use crate::sprite::{draw_key, Sprite};
use crate::text::{shape, Text};
use crate::tilemap::Tilemap;

/// Every scene entity the box from `a` to `b` (world space, either corner
/// first) takes in, bottom of the draw order first — so the last is the
/// topmost, which a selection makes its primary.
pub fn entities_within(world: &World, a: Vec2, b: Vec2, sheets: Sheets<'_>) -> Vec<Entity> {
    let (min, max) = (a.min(b), a.max(b));
    let inside = |p: Vec2| p.cmpge(min).all() && p.cmple(max).all();
    let transforms = WorldTransforms::seen_from(world, sheets.camera);
    let mut found: Vec<(Entity, (i32, i32, u32))> = world
        .query::<SceneId>()
        .filter(|(entity, _)| !crate::ui::is_ui(world, *entity))
        .filter_map(|(entity, _)| {
            let matrix = transforms.matrix(entity);
            let taken = match shape_of(world, entity, matrix, sheets) {
                Some(corners) => corners.iter().all(|&c| inside(c)),
                None => inside(matrix.transform_point2(Vec2::ZERO)),
            };
            taken.then(|| (entity, key_of(world, entity, sheets)))
        })
        .collect();
    found.sort_by_key(|(_, key)| *key);
    found.into_iter().map(|(entity, _)| entity).collect()
}

/// The corners of what `entity` draws, in the world — a sprite's quad, a
/// text's laid-out box, a tilemap layer's tiles' bounds — or `None` for an
/// entity with nothing drawn (or a text not laid out yet). What the editor
/// outlines a selected entity by.
pub fn outline(world: &World, entity: Entity, sheets: Sheets<'_>) -> Option<[Vec2; 4]> {
    let transforms = WorldTransforms::seen_from(world, sheets.camera);
    shape_of(world, entity, transforms.matrix(entity), sheets)
}

/// [`outline`] through an already composed `matrix`.
fn shape_of(world: &World, entity: Entity, matrix: Mat3, sheets: Sheets<'_>) -> Option<[Vec2; 4]> {
    let local = if let Some(sprite) = world.get::<Sprite>(entity) {
        quad(sprite, sheets).corners()
    } else if let Some(text) = world.get::<Text>(entity) {
        let bounds = shape::bounds(text, sheets.fonts?)?;
        box_corners(bounds.min, bounds.max)
    } else {
        let map = world.get::<Tilemap>(entity)?;
        let (low, high) = map.bounds()?;
        box_corners(map.cell_origin(low), map.cell_origin(high + IVec2::ONE))
    };
    Some(local.map(|corner| matrix.transform_point2(corner)))
}

/// The key `entity` draws at, for ordering what a box took.
fn key_of(world: &World, entity: Entity, sheets: Sheets<'_>) -> (i32, i32, u32) {
    if let Some(sprite) = world.get::<Sprite>(entity) {
        draw_key(
            entity,
            sheets.rank(&sprite.sorting_layer),
            sprite.sort_order,
        )
    } else if let Some(text) = world.get::<Text>(entity) {
        draw_key(entity, sheets.rank(&text.sorting_layer), text.sort_order)
    } else if let Some(map) = world.get::<Tilemap>(entity) {
        draw_key(entity, sheets.rank(&map.sorting_layer), map.sort_order)
    } else {
        draw_key(entity, 0, 0)
    }
}

fn box_corners(min: Vec2, max: Vec2) -> [Vec2; 4] {
    [min, Vec2::new(max.x, min.y), max, Vec2::new(min.x, max.y)]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Tile, Transform};

    fn spawn(world: &mut World, at: Vec2) -> Entity {
        let e = world.spawn();
        world.insert(e, SceneId::new());
        world.insert(e, Transform::from_translation(at));
        e
    }

    #[test]
    fn a_sprite_is_taken_only_when_all_of_it_is_inside() {
        let mut world = World::new();
        // Unit quads: half a unit either way of their centre.
        let whole = spawn(&mut world, Vec2::new(1.0, 1.0));
        world.insert(whole, Sprite::default());
        let straddling = spawn(&mut world, Vec2::new(3.0, 1.0));
        world.insert(straddling, Sprite::default());
        let taken = entities_within(&world, Vec2::ZERO, Vec2::new(3.0, 2.0), Sheets::default());
        assert_eq!(taken, [whole]);
    }

    #[test]
    fn an_entity_with_nothing_drawn_is_taken_by_its_origin_and_hud_never() {
        let mut world = World::new();
        let lamp = spawn(&mut world, Vec2::new(0.5, 0.5));
        world.insert(lamp, crate::PointLight2D::default());
        let far = spawn(&mut world, Vec2::new(9.0, 9.0));
        let hud = spawn(&mut world, Vec2::new(0.5, 0.5));
        world.insert(hud, crate::UiRect::default());
        // Corners in either order.
        let taken = entities_within(&world, Vec2::new(1.0, 1.0), Vec2::ZERO, Sheets::default());
        assert_eq!(taken, [lamp]);
        assert!(!taken.contains(&far) && !taken.contains(&hud));
    }

    #[test]
    fn a_layer_is_taken_only_when_its_tiles_are_inside() {
        let mut world = World::new();
        let ground = spawn(&mut world, Vec2::ZERO);
        let mut map = Tilemap::new(Vec2::ONE);
        map.set(IVec2::new(0, 0), Some(Tile::new(0)));
        map.set(IVec2::new(4, 0), Some(Tile::new(0)));
        world.insert(ground, map);
        let some = entities_within(
            &world,
            Vec2::splat(-0.5),
            Vec2::new(2.0, 1.5),
            Sheets::default(),
        );
        assert!(
            some.is_empty(),
            "the box crosses the level, it does not take it"
        );
        let all = entities_within(
            &world,
            Vec2::splat(-0.5),
            Vec2::new(5.5, 1.5),
            Sheets::default(),
        );
        assert_eq!(all, [ground]);
    }

    #[test]
    fn the_topmost_is_last() {
        let mut world = World::new();
        let top = spawn(&mut world, Vec2::new(1.0, 1.0));
        world.insert(top, Sprite::default().with_sort_order(5));
        let bottom = spawn(&mut world, Vec2::new(1.0, 1.0));
        world.insert(bottom, Sprite::default().with_sort_order(-5));
        let taken = entities_within(&world, Vec2::ZERO, Vec2::splat(2.0), Sheets::default());
        assert_eq!(taken, [bottom, top]);
    }

    #[test]
    fn an_outline_is_what_is_drawn_and_nothing_for_an_empty_entity() {
        let mut world = World::new();
        let sprite = spawn(&mut world, Vec2::new(2.0, 0.0));
        world.insert(sprite, Sprite::default());
        let corners = outline(&world, sprite, Sheets::default()).expect("a quad");
        let min = corners.iter().copied().fold(Vec2::INFINITY, Vec2::min);
        let max = corners.iter().copied().fold(Vec2::NEG_INFINITY, Vec2::max);
        assert_eq!((min, max), (Vec2::new(1.5, -0.5), Vec2::new(2.5, 0.5)));
        let empty = spawn(&mut world, Vec2::ZERO);
        assert_eq!(outline(&world, empty, Sheets::default()), None);
    }
}
