//! Layers that scroll slower or faster than the world as the camera moves.

use std::collections::HashMap;

use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;

use crate::hierarchy::{self, WorldTransforms};

/// Draws this entity and everything under it as if the camera had moved by
/// only `factor` of how far it really did: `1` moves with the world, `0` is
/// pinned to the camera — a sky — and `0.5` drifts half as fast as the level,
/// so it reads as far away. Above `1` reads as nearer than the level.
///
/// Godot's `Parallax2D::scroll_scale`. Per axis, so a background can follow
/// the camera vertically and parallax only horizontally.
///
/// **Draw-time only.** Picking agrees with it, because a click must land on
/// what is drawn; physics does not, because a parallax layer is decoration —
/// a collider under one stays where its transform says. The nearest
/// `Parallax` above an entity is the one that applies; they do not multiply.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Parallax {
    /// How much of the camera's movement it follows, per axis; see above.
    pub factor: Vec2,
    /// Draws the layer again every `repeat` units, as many times as it
    /// takes to cover the view — Godot's `repeat_size`. Per axis; `0` on an
    /// axis does not repeat on it. Picking sees the original only: the copies
    /// are the same decoration again.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repeat: Option<Vec2>,
}

impl Default for Parallax {
    /// Half speed on both axes: a background, the usual first use.
    fn default() -> Self {
        Self::new(Vec2::splat(0.5))
    }
}

/// The most copies of one drawable a repeating layer draws, per axis: a
/// period far smaller than the view must not turn one sprite into millions
/// of quads.
pub const MAX_REPEATS: i32 = 64;

impl Parallax {
    /// A layer moving at `factor` of the camera, not repeating.
    pub fn new(factor: Vec2) -> Self {
        Self {
            factor,
            repeat: None,
        }
    }

    /// The same layer, drawn again every `period` units.
    pub fn repeating(mut self, period: Vec2) -> Self {
        self.repeat = Some(period);
        self
    }

    /// The offsets at which a drawable at `at` is drawn so the copies cover
    /// `visible` — `[ZERO]` when this layer does not repeat or there is no
    /// view to cover. One period of margin either side, so a copy sliding in
    /// is already there.
    pub fn copies(&self, at: Vec2, visible: Option<(Vec2, Vec2)>) -> Vec<Vec2> {
        let (Some(period), Some((min, max))) = (self.repeat, visible) else {
            return vec![Vec2::ZERO];
        };
        let span = |axis: usize| -> std::ops::RangeInclusive<i32> {
            let p = period[axis];
            if !(p.is_finite() && p > 0.0) || !at[axis].is_finite() {
                return 0..=0;
            }
            let first = ((min[axis] - p - at[axis]) / p).ceil();
            let last = ((max[axis] + p - at[axis]) / p).floor();
            if !(first.is_finite() && last.is_finite()) || last < first {
                return 0..=0;
            }
            let first = first.max(-(MAX_REPEATS as f32)) as i32;
            let last = (last as i32).min(first + MAX_REPEATS - 1);
            first..=last
        };
        let mut offsets = Vec::new();
        for y in span(1) {
            for x in span(0) {
                offsets.push(Vec2::new(x as f32 * period.x, y as f32 * period.y));
            }
        }
        offsets
    }
    /// How far a layer of this factor is drawn from where its transform puts
    /// it, seen through a camera at `camera`.
    pub fn offset(&self, camera: Vec2) -> Vec2 {
        let offset = camera * (Vec2::ONE - self.factor);
        if offset.is_finite() {
            offset
        } else {
            Vec2::ZERO
        }
    }
}

/// Displaces every composed matrix under a [`Parallax`] for a camera at
/// `camera`. A world with no `Parallax` is left untouched at the cost of
/// one empty query.
pub(crate) fn apply(transforms: &mut WorldTransforms, world: &World, camera: Vec2) {
    if world.query::<Parallax>().next().is_none() || camera == Vec2::ZERO {
        return;
    }
    let mut nearest: HashMap<Entity, Option<Vec2>> = HashMap::new();
    let entities: Vec<Entity> = transforms.entities().collect();
    for entity in entities {
        let offset = nearest_offset(world, entity, camera, &mut nearest);
        if let Some(offset) = offset {
            transforms.translate(entity, offset);
        }
    }
}

/// Where `entity`, drawn by `matrix`, is drawn again for the nearest
/// repeating layer above it — `[ZERO]` with none.
pub(crate) fn copies_of(
    world: &World,
    entity: Entity,
    matrix: voltra_render::glam::Mat3,
    visible: Option<(Vec2, Vec2)>,
) -> Vec<Vec2> {
    std::iter::once(entity)
        .chain(hierarchy::ancestors(world, entity))
        .find_map(|e| world.get::<Parallax>(e))
        .map_or_else(
            || vec![Vec2::ZERO],
            |layer| layer.copies(matrix.transform_point2(Vec2::ZERO), visible),
        )
}

/// The offset of the nearest `Parallax` at or above `entity`, memoised so a
/// deep layer of tiles walks its chain once.
fn nearest_offset(
    world: &World,
    entity: Entity,
    camera: Vec2,
    memo: &mut HashMap<Entity, Option<Vec2>>,
) -> Option<Vec2> {
    if let Some(known) = memo.get(&entity) {
        return *known;
    }
    let found = std::iter::once(entity)
        .chain(hierarchy::ancestors(world, entity))
        .find_map(|e| world.get::<Parallax>(e))
        .map(|parallax| parallax.offset(camera));
    memo.insert(entity, found);
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SceneId, Transform};

    fn spawn(world: &mut World, at: Vec2) -> Entity {
        let entity = world.spawn();
        world.insert(entity, SceneId::new());
        world.insert(entity, Transform::from_translation(at));
        entity
    }

    fn drawn_at(world: &World, entity: Entity, camera: Vec2) -> Vec2 {
        WorldTransforms::seen_from(world, camera)
            .matrix(entity)
            .transform_point2(Vec2::ZERO)
    }

    #[test]
    fn a_half_speed_layer_is_drawn_half_way_to_the_camera() {
        let mut world = World::new();
        let hills = spawn(&mut world, Vec2::new(1.0, 0.0));
        world.insert(hills, Parallax::default());
        assert_eq!(
            drawn_at(&world, hills, Vec2::new(10.0, 4.0)),
            Vec2::new(6.0, 2.0)
        );
    }

    #[test]
    fn a_factor_of_zero_is_pinned_to_the_camera() {
        let mut world = World::new();
        let sky = spawn(&mut world, Vec2::ZERO);
        world.insert(sky, Parallax::new(Vec2::ZERO));
        assert_eq!(
            drawn_at(&world, sky, Vec2::new(-7.0, 3.0)),
            Vec2::new(-7.0, 3.0)
        );
    }

    #[test]
    fn a_factor_of_one_and_no_parallax_are_the_world() {
        let mut world = World::new();
        let level = spawn(&mut world, Vec2::new(2.0, 0.0));
        let same = spawn(&mut world, Vec2::new(2.0, 0.0));
        world.insert(same, Parallax::new(Vec2::ONE));
        let camera = Vec2::new(5.0, 5.0);
        assert_eq!(drawn_at(&world, level, camera), Vec2::new(2.0, 0.0));
        assert_eq!(drawn_at(&world, same, camera), Vec2::new(2.0, 0.0));
    }

    #[test]
    fn children_move_with_their_parallax_parent_and_the_nearest_wins() {
        let mut world = World::new();
        let far = spawn(&mut world, Vec2::ZERO);
        world.insert(far, Parallax::new(Vec2::ZERO));
        let tree = spawn(&mut world, Vec2::new(1.0, 0.0));
        hierarchy::set_parent(&mut world, tree, far).expect("parents");
        let near = spawn(&mut world, Vec2::ZERO);
        hierarchy::set_parent(&mut world, near, far).expect("parents");
        world.insert(near, Parallax::new(Vec2::ONE));

        let camera = Vec2::new(10.0, 0.0);
        assert_eq!(drawn_at(&world, tree, camera), Vec2::new(11.0, 0.0));
        assert_eq!(
            drawn_at(&world, near, camera),
            Vec2::ZERO,
            "its own factor of one"
        );
    }

    #[test]
    fn per_axis_factors_are_independent() {
        let mut world = World::new();
        let layer = spawn(&mut world, Vec2::ZERO);
        world.insert(layer, Parallax::new(Vec2::new(0.5, 1.0)));
        assert_eq!(
            drawn_at(&world, layer, Vec2::new(8.0, 8.0)),
            Vec2::new(4.0, 0.0)
        );
    }

    #[test]
    fn a_repeating_layer_covers_the_view_and_no_more() {
        let hills = Parallax::new(Vec2::splat(0.5)).repeating(Vec2::new(10.0, 0.0));
        let view = Some((Vec2::new(-15.0, -5.0), Vec2::new(15.0, 5.0)));
        let copies = hills.copies(Vec2::ZERO, view);
        let xs: Vec<f32> = copies.iter().map(|o| o.x).collect();
        assert_eq!(xs, vec![-20.0, -10.0, 0.0, 10.0, 20.0]);
        assert!(
            copies.iter().all(|o| o.y == 0.0),
            "0 on y: no vertical copies"
        );
    }

    #[test]
    fn without_a_view_or_a_repeat_there_is_one_copy() {
        let plain = Parallax::default();
        assert_eq!(
            plain.copies(Vec2::ZERO, Some((Vec2::ZERO, Vec2::ONE))),
            vec![Vec2::ZERO]
        );
        let hills = plain.repeating(Vec2::splat(5.0));
        assert_eq!(hills.copies(Vec2::ZERO, None), vec![Vec2::ZERO]);
    }

    #[test]
    fn a_tiny_period_is_capped_rather_than_flooding_the_batch() {
        let dust = Parallax::default().repeating(Vec2::new(0.001, 0.0));
        let copies = dust.copies(Vec2::ZERO, Some((Vec2::splat(-100.0), Vec2::splat(100.0))));
        assert!(copies.len() <= MAX_REPEATS as usize, "{}", copies.len());
    }

    #[test]
    fn a_repeating_layer_is_drawn_once_per_copy_in_the_batch() {
        use crate::sprite::sheets::Sheets;
        use crate::{Sprite, SpriteBatch};

        let mut world = World::new();
        let hill = spawn(&mut world, Vec2::ZERO);
        world.insert(hill, Sprite::default());
        world.insert(
            hill,
            Parallax::new(Vec2::ONE).repeating(Vec2::new(10.0, 0.0)),
        );
        let sheets = Sheets::default().visible_in(Vec2::new(-15.0, -5.0), Vec2::new(15.0, 5.0));
        // Copies at -20..=20 by 10; the ones at ±20 are culled, ±10 and 0 kept.
        assert_eq!(SpriteBatch::from_world(&world, sheets).sprite_count(), 3);
    }

    #[test]
    fn a_nonsense_factor_draws_where_the_transform_says() {
        let parallax = Parallax::new(Vec2::new(f32::NAN, 0.0));
        assert_eq!(parallax.offset(Vec2::ONE), Vec2::ZERO);
    }

    #[test]
    fn the_draw_and_the_pick_agree_on_where_a_layer_is() {
        use crate::pick::entity_at;
        use crate::sprite::sheets::Sheets;
        use crate::{Sprite, SpriteBatch};

        let mut world = World::new();
        let hill = spawn(&mut world, Vec2::ZERO);
        world.insert(hill, Sprite::default());
        world.insert(hill, Parallax::new(Vec2::ZERO));
        let sheets = Sheets::default().seen_from(Vec2::new(20.0, 0.0));

        let batch = SpriteBatch::from_world(&world, sheets);
        let left = batch
            .vertices
            .iter()
            .map(|v| v.position[0])
            .fold(f32::INFINITY, f32::min);
        assert_eq!(left, 19.5, "drawn around the camera");
        assert_eq!(entity_at(&world, Vec2::new(20.0, 0.0), sheets), Some(hill));
        assert_eq!(
            entity_at(&world, Vec2::ZERO, sheets),
            None,
            "not where it was"
        );
    }
}
