//! Which entity is under a point.
//!
//! 2D only, per CLAUDE.md: a point tested against a quad, not a ray against a
//! volume. 3D picking is a different subsystem and will not reuse this.

use voltra_ecs::{Entity, World};
use voltra_render::glam::{Mat3, Vec2};

use crate::hierarchy::WorldTransforms;
use crate::sprite::quad::quad;
use crate::sprite::sheets::Sheets;
use crate::sprite::{draw_key, Sprite};
use crate::transform::Transform;

/// Determinant at or below which a transform counts as collapsed.
///
/// Deliberately **not** `f32::EPSILON`. That constant is a *relative* precision
/// figure — the gap between `1.0` and the next representable float — and using
/// it as an absolute cutoff rejects any sprite with a uniform scale below about
/// `3.5e-4`, which is small but perfectly invertible and perfectly legitimate.
///
/// The determinant of a 2D linear transform is an *area* scale factor — the
/// product of both axes, not either one alone — so this threshold is a
/// statement about the sprite's on-screen area, not about its width or height
/// individually. A uniform scale of `1e-6` gives a determinant of `1e-12`: a
/// sprite a millionth of a world unit on a side, far below a pixel under any
/// zoom worth supporting (this crate has no `Camera2D` — that type lives in
/// `voltra-editor` and has no business being named here). An anisotropic
/// transform can reach the same determinant a different way, say `1e-4` on one
/// axis against `1e-8` on the other, and is rejected for the same reason:
/// whatever the shape, the quad it maps to has negligible area. Inverting a
/// matrix with that determinant still produces finite numbers, well inside
/// `f32`'s range — so this threshold is about intent, not about the arithmetic
/// breaking down.
const MIN_DETERMINANT: f32 = 1e-12;

/// The topmost sprite whose quad contains `point`, in world space.
///
/// "Topmost" is the same `(sort_order, entity index)` ordering
/// [`SpriteBatch::from_world`] draws in, so the sprite this returns is the one
/// whose pixels are actually visible at that point.
///
/// [`SpriteBatch::from_world`]: crate::batch::SpriteBatch::from_world
pub fn sprite_at(world: &World, point: Vec2, sheets: Sheets<'_>) -> Option<Entity> {
    // The same composed matrices the batch draws with, so what is picked is
    // what is on screen. A parented sprite is nowhere near its own `Transform`.
    let transforms = WorldTransforms::from_world(world);

    world
        .query2::<Transform, Sprite>()
        .filter(|(entity, _transform, sprite)| {
            contains(transforms.matrix(*entity), point, sprite, sheets)
        })
        .max_by_key(|(entity, _transform, sprite)| draw_key(*entity, sprite))
        .map(|(entity, _transform, _sprite)| entity)
}

/// Whether `point`, in world space, falls inside the quad `matrix` places.
///
/// Carries the point into the sprite's local space rather than building an
/// oriented bounding box in world space. Every sprite is the same axis-aligned
/// unit quad before its transform, so once the point is local the test is two
/// comparisons — and rotation and non-uniform scale come out exact with no
/// second code path.
fn contains(matrix: Mat3, point: Vec2, sprite: &Sprite, sheets: Sheets<'_>) -> bool {
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
    quad(sprite, sheets).contains(local)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::FRAC_PI_4;

    // `World`, `Entity`, `Transform`, `Sprite` and `Vec2` all arrive through
    // `super::*` — do not import them again here, or the glob and the explicit
    // import name the same item twice.

    /// A pick with no sheets loaded: every sprite here is a plain unit quad.
    fn pick(world: &World, point: Vec2) -> Option<Entity> {
        sprite_at(world, point, Sheets::default())
    }

    fn spawn(world: &mut World, transform: Transform, sprite: Sprite) -> Entity {
        let e = world.spawn();
        world.insert(e, transform);
        world.insert(e, sprite);
        e
    }

    #[test]
    fn an_empty_world_picks_nothing() {
        assert_eq!(pick(&World::new(), Vec2::ZERO), None);
    }

    #[test]
    fn a_point_outside_every_sprite_picks_nothing() {
        let mut world = World::new();
        spawn(&mut world, Transform::default(), Sprite::default());
        assert_eq!(pick(&world, Vec2::new(5.0, 5.0)), None);
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

        assert_eq!(pick(&world, Vec2::new(-2.1, 0.1)), Some(a));
        assert_eq!(pick(&world, Vec2::new(2.1, -0.1)), Some(b));
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
        assert!(pick(&world, Vec2::new(0.0, 0.6)).is_some());
        // Inside the bounding box, outside the diamond. An AABB test would
        // wrongly report a hit here, which is the whole point of this test.
        assert_eq!(pick(&world, Vec2::new(0.45, 0.45)), None);
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

        assert!(pick(&world, Vec2::new(1.8, 0.0)).is_some());
        // Inside on x, outside on y.
        assert_eq!(pick(&world, Vec2::new(1.8, 0.4)), None);
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

        assert_eq!(pick(&world, Vec2::ZERO), Some(over));

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
        assert_eq!(pick(&reversed, Vec2::ZERO), Some(over_first));
    }

    #[test]
    fn an_overlap_tie_goes_to_the_later_entity() {
        let mut world = World::new();
        let _first = spawn(&mut world, Transform::default(), Sprite::default());
        let second = spawn(&mut world, Transform::default(), Sprite::default());

        assert_eq!(pick(&world, Vec2::ZERO), Some(second));
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
        assert_eq!(pick(&world, Vec2::ZERO), None);
        assert_eq!(pick(&world, Vec2::new(100.0, 100.0)), None);
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

        assert_eq!(pick(&world, Vec2::ZERO), None);
    }

    #[test]
    fn a_small_but_usable_sprite_is_still_picked() {
        let mut world = World::new();
        // Uniform scale 1e-4 gives a determinant of 1e-8, which is below
        // `f32::EPSILON` (about 1.19e-7) and above `MIN_DETERMINANT`. That gap
        // is the whole reason the named constant exists: under `f32::EPSILON`
        // this sprite inverted perfectly well and was silently unpickable
        // anyway. This is the assertion that makes the *value* load-bearing
        // rather than merely present.
        let sprite = spawn(
            &mut world,
            Transform::default().with_scale(Vec2::splat(1e-4)),
            Sprite::default(),
        );

        assert_eq!(pick(&world, Vec2::ZERO), Some(sprite));
    }

    #[test]
    fn despawning_an_unrelated_entity_does_not_change_who_is_picked() {
        // Spawned in this order on purpose: `third` sits before both
        // overlapping sprites in storage, and `SparseSet::remove` is a
        // `swap_remove` — removing `third` pulls the dense array's *last*
        // element (`second`) into `third`'s freed slot, which swaps the
        // storage-order positions of `first` and `second`. A pick that read
        // iteration order instead of `entity.index()` would answer
        // differently before and after the despawn; `max_by_key` over a
        // unique key must not. Verified: swapping the implementation for
        // `.filter(...).last()` makes this test fail (left: Some(index 1),
        // right: Some(index 2)), so this pins a real regression, not just a
        // structural property.
        let mut world = World::new();
        let third = spawn(
            &mut world,
            Transform::from_translation(Vec2::new(50.0, 50.0)),
            Sprite::default(),
        );
        let _first = spawn(&mut world, Transform::default(), Sprite::default());
        let second = spawn(&mut world, Transform::default(), Sprite::default());

        assert_eq!(pick(&world, Vec2::ZERO), Some(second));

        world.despawn(third);

        assert_eq!(pick(&world, Vec2::ZERO), Some(second));
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

        let batch = crate::batch::SpriteBatch::from_world(&world, Sheets::default());
        let last_drawn_green = batch.vertices[batch.vertices.len() - 1].color[1];

        assert_eq!(pick(&world, Vec2::ZERO), Some(top));
        assert!(
            last_drawn_green > 0.5,
            "the sort_order 2 sprite must be drawn last, got green {last_drawn_green}"
        );
    }
}

#[cfg(test)]
mod frame_tests {
    use super::*;
    use crate::batch::SpriteBatch;
    use voltra_assets::{AssetPath, Atlases};
    use voltra_testkit::scratch_root;

    /// One 32×16 cell, so the frame is not the unit square and not square.
    fn atlases() -> (Atlases, AssetPath) {
        let root = scratch_root();
        std::fs::write(
            root.join("wide.atlas.ron"),
            "(version: 1, grid: Some((cell: (32, 16), columns: 1, rows: 1)))",
        )
        .expect("the fixture writes");
        (
            Atlases::new(&root),
            AssetPath::new("wide.atlas.ron").expect("valid"),
        )
    }

    #[test]
    fn a_pixels_per_unit_sprite_is_picked_over_the_size_it_draws() {
        // The property `HALF_EXTENT` used to hold as a constant, now that the
        // extents depend on a frame: batching and picking must agree, or a
        // click lands somewhere other than the pixels it appears to.
        let (mut atlases, path) = atlases();
        let mut world = World::new();
        let entity = world.spawn();
        world.insert(entity, Transform::default());
        let mut sprite = Sprite::default();
        sprite.set_atlas(Some(path), &mut atlases);
        sprite.pixels_per_unit = Some(16.0);
        world.insert(entity, sprite);
        let sheets = Sheets {
            atlases: Some(&atlases),
            textures: None,
        };

        assert_eq!(
            sprite_at(&world, Vec2::new(0.9, 0.0), sheets),
            Some(entity),
            "two units wide at 16 texels per unit"
        );
        assert_eq!(sprite_at(&world, Vec2::new(1.1, 0.0), sheets), None);
        assert_eq!(
            sprite_at(&world, Vec2::new(0.0, 0.6), sheets),
            None,
            "and one unit tall, not two"
        );

        let batch = SpriteBatch::from_world(&world, sheets);
        let left = batch
            .vertices
            .iter()
            .map(|vertex| vertex.position[0])
            .fold(f32::INFINITY, f32::min);
        assert_eq!(left, -1.0, "which is exactly where the batch drew it");
    }

    #[test]
    fn the_same_sprite_without_the_sheets_is_the_unit_quad() {
        // A caller that passes no stores gets the geometry every scene on disk
        // was authored against, rather than a sprite that silently shrinks.
        let mut world = World::new();
        let entity = world.spawn();
        world.insert(entity, Transform::default());
        world.insert(entity, Sprite::default());

        assert_eq!(
            sprite_at(&world, Vec2::new(0.4, 0.4), Sheets::default()),
            Some(entity)
        );
        assert_eq!(
            sprite_at(&world, Vec2::new(0.9, 0.0), Sheets::default()),
            None
        );
    }
}
