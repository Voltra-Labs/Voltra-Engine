//! 2D bones: a skeleton of parented bones, sprites hung on them, poses keyed
//! by clips and limbs bent by two-bone IK.
//!
//! Godot's `Skeleton2D` + `Bone2D` and Unity's 2D Animation package agree
//! on the shape, and so does this: **a bone is an entity** in the
//! hierarchy, its pose its own [`Transform2D`] — so a clip keys a bone's
//! rotation as it keys any entity's (stage 66, bound by name from the
//! player), the gizmo turns one, and a sprite parented to one is cut-out
//! animation with nothing more. [`Bone2D`] adds what a transform does not
//! say: how long the bone is (it points along its own +x) and its rest
//! pose. [`Skeleton2D`] marks the root the bones are named from; [`TwoBoneIk`]
//! bends a limb to reach a target.

pub mod bone;
pub mod ik;

use voltra_ecs::{Entity, World};
use voltra_render::glam::{Mat3, Vec2};

use crate::hierarchy::{ancestors, descendants, WorldTransforms2D};

pub use bone::Bone2D;
pub use ik::{solve_two_bone, TwoBoneIk};

/// The root of a skeleton: what its bones' names, an IK target's path and a
/// skinned mesh's bone list are read from.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Skeleton2D {}

impl Skeleton2D {
    /// The registered name a scene file stores it under.
    pub const NAME: &'static str = "Skeleton2D";
}

/// The nearest ancestor of `entity` (or `entity` itself) that is a
/// [`Skeleton2D`].
pub fn skeleton_of(world: &World, entity: Entity) -> Option<Entity> {
    std::iter::once(entity)
        .chain(ancestors(world, entity))
        .find(|&e| world.get::<Skeleton2D>(e).is_some())
}

/// Every bone under `skeleton`, depth first in the hierarchy's order.
pub fn bones_of(world: &World, skeleton: Entity) -> Vec<Entity> {
    descendants(world, skeleton)
        .into_iter()
        .filter(|&e| world.get::<Bone2D>(e).is_some())
        .collect()
}

/// Puts every bone under `skeleton` back in its rest pose.
pub fn reset_to_rest(world: &mut World, skeleton: Entity) {
    for bone in bones_of(world, skeleton) {
        let rest = world.get::<Bone2D>(bone).map(|b| b.rest);
        if let (Some(rest), Some(transform)) = (rest, world.get_mut::<crate::Transform2D>(bone)) {
            *transform = rest;
        }
    }
}

/// A bone as drawn: where it starts and ends in the world.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlacedBone {
    /// The bone's entity.
    pub entity: Entity,
    /// Its origin in the world.
    pub start: Vec2,
    /// Its tip in the world: `length` along its +x.
    pub end: Vec2,
}

/// Every bone in `world`, placed by `transforms`.
pub fn placed_bones(world: &World, transforms: &WorldTransforms2D) -> Vec<PlacedBone> {
    world
        .query::<Bone2D>()
        .map(|(entity, bone)| place(transforms.matrix(entity), entity, bone))
        .collect()
}

fn place(matrix: Mat3, entity: Entity, bone: &Bone2D) -> PlacedBone {
    PlacedBone {
        entity,
        start: matrix.transform_point2(Vec2::ZERO),
        end: matrix.transform_point2(Vec2::new(bone.length, 0.0)),
    }
}

/// The bone whose drawn segment passes within `reach` of `point` in the
/// world, nearest first — what a click in the viewport selects before the
/// sprites hung on it.
pub fn bone_at(world: &World, point: Vec2, reach: f32) -> Option<Entity> {
    let transforms = WorldTransforms2D::from_world(world);
    placed_bones(world, &transforms)
        .into_iter()
        .map(|bone| {
            (
                bone.entity,
                distance_to_segment(point, bone.start, bone.end),
            )
        })
        .filter(|(_, d)| *d <= reach)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(entity, _)| entity)
}

fn distance_to_segment(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    let ab = b - a;
    let t = if ab.length_squared() > 0.0 {
        ((p - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0)
    } else {
        0.0
    };
    p.distance(a + ab * t)
}

#[cfg(test)]
mod tests;
