//! The polygon a caster starts from when it is given one of its own: the
//! shape it already had, in the entity's space, so switching does not move
//! the shadow.

use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;

use super::outline::collider_outline;
use crate::hierarchy::world_matrix;
use crate::sprite::quad::quad;
use crate::sprite::sheets::Sheets;
use crate::{Collider2D, Sprite2D};

/// Corners a circle collider is cut into for editing: few enough to grab
/// one at a time, where the drawn outline uses
/// [`CIRCLE_SIDES`](super::CIRCLE_SIDES) for a smooth shadow.
pub const EDIT_CIRCLE_SIDES: usize = 16;

/// `entity`'s collider as it casts now, taken into the entity's own space;
/// else its sprite's quad, resolved against `sheets`; else `None`.
///
/// The collider is placed by the entity's own [`Transform2D`] (as the physics
/// places it) while a polygon is carried by the composed matrix, parents and
/// all; going through the world outline and back through that matrix's
/// inverse keeps the shadow where it was either way — a circle on a
/// non-uniform scale, which the physics keeps round, or a collider under a
/// parent, should physics and parenting stop being exclusive.
/// `None` for a collider with no area, or an entity scaled to nothing.
pub fn starting_polygon(world: &World, entity: Entity, sheets: Sheets<'_>) -> Option<Vec<Vec2>> {
    if world.get::<Collider2D>(entity).is_some() {
        let outline = collider_outline(world, entity, EDIT_CIRCLE_SIDES)?;
        let inverse = world_matrix(world, entity).inverse();
        if !inverse.is_finite() {
            return None;
        }
        return Some(
            outline
                .iter()
                .map(|p| inverse.transform_point2(*p))
                .collect(),
        );
    }
    let sprite = world.get::<Sprite2D>(entity)?;
    Some(quad(sprite, sheets).corners().to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Transform2D;

    #[test]
    fn a_box_collider_starts_as_its_unscaled_corners() {
        let mut world = World::new();
        let entity = world.spawn();
        world.insert(entity, Transform2D::default().with_scale(Vec2::splat(3.0)));
        world.insert(
            entity,
            Collider2D::Box {
                half_extents: Vec2::new(1.0, 2.0),
            },
        );
        let points = starting_polygon(&world, entity, Sheets::default()).expect("a collider");
        assert_eq!(points.len(), 4);
        assert!(points.contains(&Vec2::new(-1.0, 2.0)), "{points:?}");
    }

    #[test]
    fn a_circle_starts_as_an_editable_polygon_around_its_centre() {
        let mut world = World::new();
        let entity = world.spawn();
        world.insert(entity, Transform2D::default());
        world.insert(entity, Collider2D::Circle { radius: 2.0 });
        let points = starting_polygon(&world, entity, Sheets::default()).expect("a collider");
        assert_eq!(points.len(), EDIT_CIRCLE_SIDES);
        assert!(points.iter().all(|p| (p.length() - 2.0).abs() < 1e-5));
    }

    #[test]
    fn without_a_collider_the_sprite_s_quad_is_the_start_and_without_either_nothing() {
        let mut world = World::new();
        let sprite = world.spawn();
        world.insert(sprite, Sprite2D::default());
        let points = starting_polygon(&world, sprite, Sheets::default()).expect("a sprite");
        assert_eq!(points.len(), 4);
        assert!(points.iter().all(|p| p.x.abs() == 0.5 && p.y.abs() == 0.5));
        let bare = world.spawn();
        assert_eq!(starting_polygon(&world, bare, Sheets::default()), None);
    }

    /// Where the polygon lands once the entity's matrix carries it.
    fn placed(world: &World, entity: Entity, points: &[Vec2]) -> Vec<Vec2> {
        let matrix = world_matrix(world, entity);
        points.iter().map(|p| matrix.transform_point2(*p)).collect()
    }

    #[test]
    fn switching_keeps_the_shadow_where_it_was_for_a_squashed_turned_circle() {
        // The physics keeps a circle round under a non-uniform scale; a
        // polygon carried by the same matrix would be squashed unless it
        // starts from the round outline taken back through the inverse.
        let mut world = World::new();
        let ball = world.spawn();
        let mut transform =
            Transform2D::from_translation(Vec2::new(1.0, 2.0)).with_scale(Vec2::new(2.0, 1.0));
        transform.rotation = 0.7;
        world.insert(ball, transform);
        world.insert(ball, Collider2D::Circle { radius: 1.0 });

        let before = collider_outline(&world, ball, EDIT_CIRCLE_SIDES).expect("a collider");
        let points = starting_polygon(&world, ball, Sheets::default()).expect("a collider");
        for (a, b) in placed(&world, ball, &points).iter().zip(&before) {
            assert!(a.distance(*b) < 1e-4, "{a} against {b}");
        }
    }
}
