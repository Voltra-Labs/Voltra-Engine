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
    pub factor: Vec2,
}

impl Default for Parallax {
    /// Half speed on both axes: a background, the usual first use.
    fn default() -> Self {
        Self {
            factor: Vec2::splat(0.5),
        }
    }
}

impl Parallax {
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
        world.insert(sky, Parallax { factor: Vec2::ZERO });
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
        world.insert(same, Parallax { factor: Vec2::ONE });
        let camera = Vec2::new(5.0, 5.0);
        assert_eq!(drawn_at(&world, level, camera), Vec2::new(2.0, 0.0));
        assert_eq!(drawn_at(&world, same, camera), Vec2::new(2.0, 0.0));
    }

    #[test]
    fn children_move_with_their_parallax_parent_and_the_nearest_wins() {
        let mut world = World::new();
        let far = spawn(&mut world, Vec2::ZERO);
        world.insert(far, Parallax { factor: Vec2::ZERO });
        let tree = spawn(&mut world, Vec2::new(1.0, 0.0));
        hierarchy::set_parent(&mut world, tree, far).expect("parents");
        let near = spawn(&mut world, Vec2::ZERO);
        hierarchy::set_parent(&mut world, near, far).expect("parents");
        world.insert(near, Parallax { factor: Vec2::ONE });

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
        world.insert(
            layer,
            Parallax {
                factor: Vec2::new(0.5, 1.0),
            },
        );
        assert_eq!(
            drawn_at(&world, layer, Vec2::new(8.0, 8.0)),
            Vec2::new(4.0, 0.0)
        );
    }

    #[test]
    fn a_nonsense_factor_draws_where_the_transform_says() {
        let parallax = Parallax {
            factor: Vec2::new(f32::NAN, 0.0),
        };
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
        world.insert(hill, Parallax { factor: Vec2::ZERO });
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
