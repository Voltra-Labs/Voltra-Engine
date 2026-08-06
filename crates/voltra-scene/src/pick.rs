//! Which entity is under a point.
//!
//! 2D only, per CLAUDE.md: a point tested against a quad, not a ray against a
//! volume. 3D picking is a different subsystem and will not reuse this.

use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;

use crate::sprite::Sprite;
use crate::transform::Transform;

/// Determinant at or below which a transform counts as collapsed.
///
/// Deliberately **not** `f32::EPSILON`. That constant is a *relative* precision
/// figure — the gap between `1.0` and the next representable float — and using
/// it as an absolute cutoff rejects any sprite with a uniform scale below about
/// `3.5e-4`, which is small but perfectly invertible and perfectly legitimate.
///
/// `1e-12` is the determinant of a uniform scale of `1e-6`, a millionth of a
/// world unit. The tightest view the camera allows is two world units divided
/// by `Camera2D::MAX_ZOOM`, so a sprite that size is orders of magnitude below
/// one pixel and cannot be meant to be clicked. Inverting a matrix with that
/// determinant still produces finite numbers, well inside `f32`'s range — so
/// this threshold is about intent, not about the arithmetic breaking down.
const MIN_DETERMINANT: f32 = 1e-12;

/// The topmost sprite whose quad contains `point`, in world space.
///
/// "Topmost" is the same `(sort_order, entity index)` ordering
/// [`SpriteBatch::from_world`] draws in, so the sprite this returns is the one
/// whose pixels are actually visible at that point.
///
/// [`SpriteBatch::from_world`]: crate::batch::SpriteBatch::from_world
pub fn sprite_at(world: &World, point: Vec2) -> Option<Entity> {
    world
        .query2::<Transform, Sprite>()
        .filter(|(_entity, transform, _sprite)| contains(transform, point))
        .max_by_key(|(entity, _transform, sprite)| (sprite.sort_order, entity.index()))
        .map(|(entity, _transform, _sprite)| entity)
}

/// Whether `point`, in world space, falls inside this transform's quad.
///
/// Carries the point into the sprite's local space rather than building an
/// oriented bounding box in world space. Every sprite is the same axis-aligned
/// unit quad before its transform, so once the point is local the test is two
/// comparisons — and rotation and non-uniform scale come out exact with no
/// second code path.
fn contains(transform: &Transform, point: Vec2) -> bool {
    let matrix = transform.matrix();

    // A zero scale on either axis makes the matrix singular. `Mat3::inverse`
    // does not panic on one: it returns infinities and NaN, and every
    // comparison against NaN is false. `inverse_or_zero` is worse rather than
    // better here — a zero matrix sends every point to the origin, which is
    // inside the quad, so a collapsed sprite would be pickable everywhere.
    if matrix.determinant().abs() < MIN_DETERMINANT {
        return false;
    }

    // Inclusive on both edges, so a point exactly on a boundary is a hit. The
    // GPU's fill convention for that same edge is its own, so a pixel on the
    // shared border of two sprites can be picked as one and shaded as the
    // other. A sub-pixel disagreement on a measure-zero set, and not worth a
    // second rule to reconcile.
    let local = matrix.inverse().transform_point2(point);
    local.x.abs() <= Sprite::HALF_EXTENT && local.y.abs() <= Sprite::HALF_EXTENT
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::FRAC_PI_4;

    // `World`, `Entity`, `Transform`, `Sprite` and `Vec2` all arrive through
    // `super::*` — do not import them again here, or the glob and the explicit
    // import name the same item twice.

    fn spawn(world: &mut World, transform: Transform, sprite: Sprite) -> Entity {
        let e = world.spawn();
        world.insert(e, transform);
        world.insert(e, sprite);
        e
    }

    #[test]
    fn an_empty_world_picks_nothing() {
        assert_eq!(sprite_at(&World::new(), Vec2::ZERO), None);
    }

    #[test]
    fn a_point_outside_every_sprite_picks_nothing() {
        let mut world = World::new();
        spawn(&mut world, Transform::default(), Sprite::default());
        assert_eq!(sprite_at(&world, Vec2::new(5.0, 5.0)), None);
    }

    #[test]
    fn a_point_inside_picks_that_sprite() {
        let mut world = World::new();
        let a = spawn(
            &mut world,
            Transform::from_translation(Vec2::new(-2.0, 0.0)),
            Sprite::default(),
        );
        let b = spawn(
            &mut world,
            Transform::from_translation(Vec2::new(2.0, 0.0)),
            Sprite::default(),
        );

        assert_eq!(sprite_at(&world, Vec2::new(-2.1, 0.1)), Some(a));
        assert_eq!(sprite_at(&world, Vec2::new(2.1, -0.1)), Some(b));
    }

    #[test]
    fn rotation_is_respected_not_approximated() {
        let mut world = World::new();
        // A unit square turned 45 degrees. Its bounding box reaches about
        // 0.707 on each axis, but the quad itself does not: the corner of the
        // box is outside the diamond.
        spawn(
            &mut world,
            Transform::default().with_rotation(FRAC_PI_4),
            Sprite::default(),
        );

        // Inside the diamond.
        assert!(sprite_at(&world, Vec2::new(0.0, 0.6)).is_some());
        // Inside the bounding box, outside the diamond. An AABB test would
        // wrongly report a hit here, which is the whole point of this test.
        assert_eq!(sprite_at(&world, Vec2::new(0.45, 0.45)), None);
    }

    #[test]
    fn non_uniform_scale_is_respected() {
        let mut world = World::new();
        // Four wide, half tall.
        spawn(
            &mut world,
            Transform::default().with_scale(Vec2::new(4.0, 0.5)),
            Sprite::default(),
        );

        assert!(sprite_at(&world, Vec2::new(1.8, 0.0)).is_some());
        // Inside on x, outside on y.
        assert_eq!(sprite_at(&world, Vec2::new(1.8, 0.4)), None);
    }

    #[test]
    fn the_higher_sort_order_wins_an_overlap() {
        let mut world = World::new();
        let _under = spawn(
            &mut world,
            Transform::default(),
            Sprite::default().with_sort_order(10),
        );
        let over = spawn(
            &mut world,
            Transform::default(),
            Sprite::default().with_sort_order(20),
        );

        assert_eq!(sprite_at(&world, Vec2::ZERO), Some(over));

        // And the answer does not depend on which was spawned first.
        let mut reversed = World::new();
        let over_first = spawn(
            &mut reversed,
            Transform::default(),
            Sprite::default().with_sort_order(20),
        );
        spawn(
            &mut reversed,
            Transform::default(),
            Sprite::default().with_sort_order(10),
        );
        assert_eq!(sprite_at(&reversed, Vec2::ZERO), Some(over_first));
    }

    #[test]
    fn an_overlap_tie_goes_to_the_later_entity() {
        let mut world = World::new();
        let _first = spawn(&mut world, Transform::default(), Sprite::default());
        let second = spawn(&mut world, Transform::default(), Sprite::default());

        assert_eq!(sprite_at(&world, Vec2::ZERO), Some(second));
    }

    #[test]
    fn a_zero_scale_sprite_is_never_picked() {
        let mut world = World::new();
        spawn(
            &mut world,
            Transform::default().with_scale(Vec2::new(0.0, 1.0)),
            Sprite::default(),
        );

        // Honest about its own weakness: this passes with the determinant check
        // removed too. `glam` inverts an exactly-singular matrix to NaN, and
        // every comparison against NaN is false, so the sprite is skipped by
        // accident rather than by decision. Kept because it pins the behaviour
        // we want, but the test that makes the guard load-bearing is the next
        // one.
        assert_eq!(sprite_at(&world, Vec2::ZERO), None);
        assert_eq!(sprite_at(&world, Vec2::new(100.0, 100.0)), None);
    }

    #[test]
    fn a_near_zero_scale_sprite_is_never_picked() {
        let mut world = World::new();
        // Nearly singular rather than singular, which is the case the
        // determinant check actually decides. This matrix still inverts to
        // finite numbers, so NaN does not rescue us: without the check, the
        // inverse scales x by 1e30, the origin maps to the origin, and a sprite
        // far thinner than a pixel is pickable along its entire centre line.
        spawn(
            &mut world,
            Transform::default().with_scale(Vec2::new(1e-30, 1.0)),
            Sprite::default(),
        );

        assert_eq!(sprite_at(&world, Vec2::ZERO), None);
    }

    #[test]
    fn picking_agrees_with_the_draw_order() {
        // `sort_order` and spawn order deliberately disagree: the sprite that
        // must win is spawned FIRST, so it carries the *lower* entity index.
        //
        // That is what makes this test about composition rather than about
        // either key alone. Order by index only and the loser wins. Swap the
        // tuple to `(index, sort_order)` and the loser wins. Only
        // `(sort_order, index)`, in that order, in both places, gives `top` —
        // which is the property the whole design rests on, since a divergence
        // would mean clicking selects something other than the visible pixels.
        let mut world = World::new();
        let top = spawn(
            &mut world,
            Transform::default(),
            Sprite::new([0.0, 1.0, 0.0, 1.0]).with_sort_order(2),
        );
        spawn(
            &mut world,
            Transform::default(),
            Sprite::new([1.0, 0.0, 0.0, 1.0]).with_sort_order(1),
        );

        let batch = crate::batch::SpriteBatch::from_world(&world);
        let last_drawn_green = batch.vertices[batch.vertices.len() - 1].color[1];

        assert_eq!(sprite_at(&world, Vec2::ZERO), Some(top));
        assert!(
            last_drawn_green > 0.5,
            "the sort_order 2 sprite must be drawn last, got green {last_drawn_green}"
        );
    }
}
