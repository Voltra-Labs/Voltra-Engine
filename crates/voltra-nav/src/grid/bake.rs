//! Baking a grid from the scene's colliders.

use voltra_ecs::{Entity, World};
use voltra_physics::narrow::manifold;
use voltra_render::glam::{IVec2, Vec2};
use voltra_scene::{
    BodyType, CharacterBody2D, Collider, CollisionLayers, OneWay, RigidBody, Sensor, Transform,
};

use super::NavGrid;

/// How a grid is baked.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NavSettings {
    /// The side of a cell, in world units: finer follows walls closer and
    /// costs more cells.
    pub cell_size: f32,
    /// Half the size of the agent the grid is for: a cell is blocked when a
    /// box this size anywhere in it would overlap an obstacle, so a path
    /// through free cells is one the agent fits along.
    pub clearance: Vec2,
    /// The collision layers obstacles are on.
    pub mask: u32,
    /// Whether paths may step diagonally.
    pub diagonal: bool,
}

impl Default for NavSettings {
    /// Quarter-unit cells for an agent a quarter-unit wide, looking at
    /// every layer, diagonals allowed.
    fn default() -> Self {
        Self {
            cell_size: 0.25,
            clearance: Vec2::splat(0.125),
            mask: CollisionLayers::ALL,
            diagonal: true,
        }
    }
}

/// Whether `entity`'s collider stands in an agent's way: solid, on a layer
/// looked at, and not moving — no rigid body, or a static one, and not a
/// character. What moves is no obstacle to bake in, and a one-way platform
/// can be passed.
fn obstacle(world: &World, entity: Entity, mask: u32) -> bool {
    if world.get::<Sensor>(entity).is_some()
        || world.get::<OneWay>(entity).is_some()
        || world.get::<CharacterBody2D>(entity).is_some()
    {
        return false;
    }
    let layers = world
        .get::<CollisionLayers>(entity)
        .copied()
        .unwrap_or_default();
    if layers.layers & mask == 0 {
        return false;
    }
    world
        .get::<RigidBody>(entity)
        .is_none_or(|body| body.body_type == BodyType::Static)
}

impl NavGrid {
    /// A grid over the rectangle `min`..`max`, with the cells an obstacle
    /// blocks — see [`NavSettings`].
    ///
    /// Each obstacle is tested only against the cells under its bounds,
    /// exactly, by the narrow phase the step uses: a rotated box blocks the
    /// cells it covers, not those its bounds do.
    pub fn bake(world: &World, min: Vec2, max: Vec2, settings: &NavSettings) -> Self {
        let cell = if settings.cell_size.is_finite() && settings.cell_size > 0.0 {
            settings.cell_size
        } else {
            NavSettings::default().cell_size
        };
        let (lo, hi) = (min.min(max), min.max(max));
        let size = ((hi - lo) / cell).ceil();
        let size = if size.is_finite() {
            size.as_ivec2()
        } else {
            IVec2::ZERO
        };
        let mut grid = NavGrid::new(lo, cell, size).with_diagonal(settings.diagonal);
        let clearance = settings.clearance.max(Vec2::ZERO);
        let probe = Collider::Box {
            half_extents: Vec2::splat(cell * 0.5) + clearance,
        };

        for (entity, collider) in world.query::<Collider>() {
            let Some(transform) = world.get::<Transform>(entity) else {
                continue;
            };
            if collider.is_degenerate(transform) || !obstacle(world, entity, settings.mask) {
                continue;
            }
            let (amin, amax) = collider.world_aabb(transform);
            let first = ((amin - clearance - grid.origin) / cell)
                .floor()
                .as_ivec2()
                .max(IVec2::ZERO);
            let last = ((amax + clearance - grid.origin) / cell)
                .ceil()
                .as_ivec2()
                .min(grid.size);
            for y in first.y..last.y {
                for x in first.x..last.x {
                    let at = IVec2::new(x, y);
                    if grid.is_blocked(at) {
                        continue;
                    }
                    let centre = Transform::from_translation(grid.centre(at));
                    if manifold((&probe, &centre), (collider, transform)).is_some() {
                        grid.set_blocked(at, true);
                    }
                }
            }
        }
        grid
    }

    /// [`bake`](Self::bake) over every obstacle there is, with a margin of
    /// a cell and the clearance all round. An empty grid when there is none.
    pub fn bake_all(world: &World, settings: &NavSettings) -> Self {
        let mut bounds: Option<(Vec2, Vec2)> = None;
        for (entity, collider) in world.query::<Collider>() {
            let Some(transform) = world.get::<Transform>(entity) else {
                continue;
            };
            if collider.is_degenerate(transform) || !obstacle(world, entity, settings.mask) {
                continue;
            }
            let (lo, hi) = collider.world_aabb(transform);
            bounds = Some(match bounds {
                Some((min, max)) => (min.min(lo), max.max(hi)),
                None => (lo, hi),
            });
        }
        let Some((min, max)) = bounds else {
            return NavGrid::new(Vec2::ZERO, settings.cell_size, IVec2::ZERO);
        };
        let margin = settings.clearance.max(Vec2::ZERO) + Vec2::splat(settings.cell_size.max(0.0));
        Self::bake(world, min - margin, max + margin, settings)
    }
}
