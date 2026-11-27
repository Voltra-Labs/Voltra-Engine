//! The queries of [`crate::query`], asked of the world as it was.

use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;
use voltra_scene::{Collider, Transform};

use crate::query::{self, Poses, QueryFilter, RayHit, ShapeHit};

/// The world with its lag-compensated colliders put back to one moment of
/// the past — or the world as it is ([`live`](Self::live)). Made by
/// [`PoseHistory::rewound`](super::PoseHistory::rewound); every query here
/// is [`crate::query`]'s, reading the past poses in place of the world's
/// and the world for everything else.
///
/// It borrows the world and changes nothing in it.
pub struct Rewound<'w> {
    world: &'w World,
    /// `None`: the world as it is.
    poses: Option<Poses>,
    tick: Option<f64>,
}

impl std::fmt::Debug for Rewound<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Rewound")
            .field("tick", &self.tick)
            .field("poses", &self.poses)
            .finish_non_exhaustive()
    }
}

impl<'w> Rewound<'w> {
    /// The world as it is: every query is [`crate::query`]'s own.
    pub fn live(world: &'w World) -> Self {
        Self {
            world,
            poses: None,
            tick: None,
        }
    }

    pub(super) fn past(world: &'w World, poses: Poses, tick: f64) -> Self {
        Self {
            world,
            poses: Some(poses),
            tick: Some(tick),
        }
    }

    /// The moment it is, in ticks; `None` for the world as it is.
    pub fn tick(&self) -> Option<f64> {
        self.tick
    }

    /// The world it reads.
    pub fn world(&self) -> &'w World {
        self.world
    }

    /// Where a query finds `entity`: its past pose if it is
    /// lag-compensated, its transform now if not; `None` when the query
    /// cannot find it — not there at that moment, hidden, or without a
    /// transform.
    pub fn pose(&self, entity: Entity) -> Option<Transform> {
        match self.poses.as_ref().and_then(|poses| poses.get(&entity)) {
            Some(Some((_, transform))) => Some(*transform),
            Some(None) => None,
            None => self.world.get::<Transform>(entity).copied(),
        }
    }

    /// The entities given a pose of the past, or hidden: the
    /// lag-compensated ones. Empty for the world as it is.
    pub fn compensated(&self) -> impl Iterator<Item = Entity> + '_ {
        self.poses.iter().flat_map(|poses| poses.keys().copied())
    }

    /// Makes `entity` one no query here finds — one the asker had no copy
    /// of, out of its interest.
    pub fn hide(&mut self, entity: Entity) {
        self.poses
            .get_or_insert_with(Poses::default)
            .insert(entity, None);
    }

    /// [`query::ray`] at this moment.
    pub fn ray(
        &self,
        origin: Vec2,
        direction: Vec2,
        max: f32,
        filter: QueryFilter,
    ) -> Option<RayHit> {
        query::ray_among(
            self.world,
            self.poses.as_ref(),
            origin,
            direction,
            max,
            filter,
        )
    }

    /// [`query::cast`] at this moment.
    pub fn cast(
        &self,
        shape: &Collider,
        at: &Transform,
        motion: Vec2,
        filter: QueryFilter,
    ) -> Option<ShapeHit> {
        query::cast_among(self.world, self.poses.as_ref(), shape, at, motion, filter)
    }

    /// [`query::point`] at this moment.
    pub fn point(&self, at: Vec2, filter: QueryFilter) -> Option<Entity> {
        query::point_among(self.world, self.poses.as_ref(), at, filter)
    }

    /// [`query::overlap_aabb`] at this moment.
    pub fn overlap_aabb(&self, min: Vec2, max: Vec2, filter: QueryFilter) -> Vec<Entity> {
        query::overlap_aabb_among(self.world, self.poses.as_ref(), min, max, filter)
    }
}
