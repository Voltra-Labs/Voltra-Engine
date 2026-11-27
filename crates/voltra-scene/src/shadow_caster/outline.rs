//! A shadow caster's outlines in the world: its polygon carried by its
//! transform, or its collider and a solid layer's generated boxes.

use std::collections::HashMap;

use voltra_ecs::{Entity, World};
use voltra_render::glam::{Mat3, Vec2};
use voltra_render::Occluder;

use super::silhouette::{sprite_outlines, SpriteShapes};
use super::{CasterShape, ShadowCaster2D};
use crate::hierarchy::{world_matrix_2d, WorldTransforms2D};
use crate::tilemap::TileCollider;
use crate::{Collider2D, Tilemap, Transform2D};

/// Corners a circle collider's outline is cut into: the polygon stays
/// within half a percent of the radius of the true circle, so its shadow's
/// edge is off by less than that over the light's distance.
pub const CIRCLE_SIDES: usize = 32;

/// Every caster's outline in the world, for the frame's lights; polygons
/// carried by `transforms`, the world's composed matrices, and sprite
/// silhouettes read through `sprites` (none without it).
pub fn occluders(
    world: &World,
    transforms: &WorldTransforms2D,
    sprites: Option<SpriteShapes<'_>>,
) -> Vec<Occluder> {
    let casters: Vec<(Entity, &ShadowCaster2D)> = world.query::<ShadowCaster2D>().collect();
    if casters.is_empty() {
        return Vec::new();
    }
    // Only looked through when some caster is a solid layer.
    let mut tile_boxes: HashMap<Entity, Vec<Entity>> = HashMap::new();
    if casters
        .iter()
        .any(|(entity, _)| is_solid_layer(world, *entity))
    {
        for (tile_box, mark) in world.query::<TileCollider>() {
            tile_boxes.entry(mark.layer).or_default().push(tile_box);
        }
    }
    let mut out = Vec::new();
    for (entity, caster) in casters {
        let matrix = || transforms.matrix(entity);
        let boxes = tile_boxes.get(&entity).map_or(&[][..], Vec::as_slice);
        outlines(world, entity, caster, matrix, boxes, sprites, &mut out);
    }
    out
}

/// `entity`'s outlines, for drawing the one selected; nothing when it casts
/// no shadow. A sprite silhouette needs `sprites`.
pub fn outlines_of(
    world: &World,
    entity: Entity,
    sprites: Option<SpriteShapes<'_>>,
) -> Vec<Occluder> {
    let Some(caster) = world.get::<ShadowCaster2D>(entity) else {
        return Vec::new();
    };
    let boxes: Vec<Entity> = if is_solid_layer(world, entity) {
        world
            .query::<TileCollider>()
            .filter(|(_, mark)| mark.layer == entity)
            .map(|(tile_box, _)| tile_box)
            .collect()
    } else {
        Vec::new()
    };
    let mut out = Vec::new();
    let matrix = || world_matrix_2d(world, entity);
    outlines(world, entity, caster, matrix, &boxes, sprites, &mut out);
    out
}

fn is_solid_layer(world: &World, entity: Entity) -> bool {
    world
        .get::<Tilemap>(entity)
        .is_some_and(|map| map.collision)
}

/// `caster`'s outlines into `out`: its polygon or its sprite's silhouette
/// carried by `matrix`, or its collider and the layer's generated
/// `tile_boxes`.
fn outlines(
    world: &World,
    entity: Entity,
    caster: &ShadowCaster2D,
    matrix: impl FnOnce() -> Mat3,
    tile_boxes: &[Entity],
    sprites: Option<SpriteShapes<'_>>,
    out: &mut Vec<Occluder>,
) {
    let outline = |points: Vec<Vec2>, closed: bool| Occluder {
        points,
        closed,
        self_shadow: caster.self_shadow,
        layers: caster.layers,
    };
    match &caster.shape {
        CasterShape::Polygon { points, closed } => {
            let matrix = matrix();
            let placed = points.iter().map(|p| matrix.transform_point2(*p)).collect();
            out.push(outline(placed, *closed));
        }
        CasterShape::Sprite { alpha } => {
            if let Some(sprites) = sprites {
                out.extend(
                    sprite_outlines(world, entity, matrix(), *alpha, sprites)
                        .into_iter()
                        .map(|points| outline(points, true)),
                );
            }
        }
        CasterShape::Collider => {
            out.extend(
                std::iter::once(entity)
                    .chain(tile_boxes.iter().copied())
                    .filter_map(|shape| collider_outline(world, shape, CIRCLE_SIDES))
                    .map(|points| outline(points, true)),
            );
        }
    }
}

/// `entity`'s collider as a world-space outline, counter-clockwise, placed
/// by its own [`Transform2D`] as the physics places it, a full turn of any
/// round part cut into `sides`; `None` without one, or for a shape with no area.
pub(super) fn collider_outline(world: &World, entity: Entity, sides: usize) -> Option<Vec<Vec2>> {
    let collider = world.get::<Collider2D>(entity)?;
    let transform = world.get::<Transform2D>(entity)?;
    if collider.is_degenerate(transform) {
        return None;
    }
    Some(collider.outline(transform, sides))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hierarchy;

    fn spawn(world: &mut World, transform: Transform2D, caster: ShadowCaster2D) -> Entity {
        let entity = world.spawn();
        world.insert(entity, transform);
        world.insert(entity, caster);
        entity
    }

    #[test]
    fn a_box_collider_casts_its_four_corners() {
        let mut world = World::new();
        let wall = spawn(
            &mut world,
            Transform2D::from_translation(Vec2::new(2.0, 0.0)),
            ShadowCaster2D::default(),
        );
        world.insert(
            wall,
            Collider2D::Box {
                half_extents: Vec2::new(0.5, 1.0),
            },
        );
        let found = occluders(&world, &WorldTransforms2D::from_world(&world), None);
        assert_eq!(found.len(), 1);
        assert!(found[0].closed && !found[0].self_shadow);
        assert!(found[0].points.contains(&Vec2::new(2.5, 1.0)));
        assert!(found[0].points.contains(&Vec2::new(1.5, -1.0)));
    }

    #[test]
    fn a_circle_collider_casts_a_polygon_around_it() {
        let mut world = World::new();
        let ball = spawn(
            &mut world,
            Transform2D::from_translation(Vec2::new(0.0, 3.0)),
            ShadowCaster2D {
                self_shadow: true,
                ..ShadowCaster2D::default()
            },
        );
        world.insert(ball, Collider2D::Circle { radius: 2.0 });
        let found = occluders(&world, &WorldTransforms2D::from_world(&world), None);
        assert_eq!(found[0].points.len(), CIRCLE_SIDES);
        assert!(found[0].self_shadow);
        assert!(found[0]
            .points
            .iter()
            .all(|p| (p.distance(Vec2::new(0.0, 3.0)) - 2.0).abs() < 1e-4));
    }

    #[test]
    fn a_collider_shape_without_a_collider_casts_nothing() {
        let mut world = World::new();
        spawn(
            &mut world,
            Transform2D::default(),
            ShadowCaster2D::default(),
        );
        let flat = spawn(
            &mut world,
            Transform2D::default(),
            ShadowCaster2D::default(),
        );
        world.insert(
            flat,
            Collider2D::Box {
                half_extents: Vec2::new(0.0, 1.0),
            },
        );
        assert!(occluders(&world, &WorldTransforms2D::from_world(&world), None).is_empty());
    }

    #[test]
    fn a_polygon_follows_its_parent() {
        let mut world = World::new();
        let parent = world.spawn();
        world.insert(
            parent,
            Transform2D::from_translation(Vec2::new(10.0, 0.0)).with_scale(Vec2::splat(2.0)),
        );
        world.insert(parent, crate::SceneId::new());
        let line = spawn(
            &mut world,
            Transform2D::default(),
            ShadowCaster2D {
                shape: CasterShape::Polygon {
                    points: vec![Vec2::ZERO, Vec2::new(1.0, 0.0)],
                    closed: false,
                },
                self_shadow: false,
                ..ShadowCaster2D::default()
            },
        );
        hierarchy::set_parent(&mut world, line, parent).expect("no cycle");
        let found = occluders(&world, &WorldTransforms2D::from_world(&world), None);
        assert!(!found[0].closed);
        assert_eq!(
            found[0].points,
            [Vec2::new(10.0, 0.0), Vec2::new(12.0, 0.0)]
        );
    }

    #[test]
    fn a_solid_layer_casts_its_generated_boxes() {
        let mut world = World::new();
        let layer = spawn(
            &mut world,
            Transform2D::default(),
            ShadowCaster2D::default(),
        );
        let mut map = Tilemap::default();
        map.collision = true;
        map.set(
            voltra_render::glam::IVec2::new(0, 0),
            Some(crate::Tile::default()),
        );
        map.set(
            voltra_render::glam::IVec2::new(3, 0),
            Some(crate::Tile::default()),
        );
        world.insert(layer, map);
        crate::tilemap::sync_colliders(&mut world);
        // Another layer's boxes are not this caster's.
        let other = world.spawn();
        world.insert(other, Transform2D::default());
        let mut other_map = Tilemap::default();
        other_map.collision = true;
        other_map.set(
            voltra_render::glam::IVec2::new(9, 9),
            Some(crate::Tile::default()),
        );
        world.insert(other, other_map);
        crate::tilemap::sync_colliders(&mut world);

        let found = occluders(&world, &WorldTransforms2D::from_world(&world), None);
        assert_eq!(found.len(), 2, "two separate tiles, two boxes: {found:?}");
        assert!(found.iter().all(|o| o.closed && o.points.len() == 4));
    }

    #[test]
    fn one_caster_s_outlines_are_its_own() {
        let mut world = World::new();
        let mine = spawn(
            &mut world,
            Transform2D::default(),
            ShadowCaster2D::default(),
        );
        world.insert(mine, Collider2D::Circle { radius: 1.0 });
        let theirs = spawn(
            &mut world,
            Transform2D::default(),
            ShadowCaster2D::default(),
        );
        world.insert(theirs, Collider2D::Circle { radius: 1.0 });
        let plain = world.spawn();
        world.insert(plain, Transform2D::default());
        world.insert(plain, Collider2D::Circle { radius: 1.0 });
        assert_eq!(outlines_of(&world, mine, None).len(), 1);
        assert!(
            outlines_of(&world, plain, None).is_empty(),
            "no caster, no shadow"
        );
        assert_eq!(
            occluders(&world, &WorldTransforms2D::from_world(&world), None).len(),
            2
        );
    }
}
