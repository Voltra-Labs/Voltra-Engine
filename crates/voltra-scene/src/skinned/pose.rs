//! The matrices a skinned mesh is drawn with this frame: one per bone,
//! taking a vertex from the mesh's bind pose to where that bone has carried
//! it in the world.
//!
//! For bone `j`, `world(j) · rest(j)⁻¹ · rest(mesh)`, where `rest(x)` is
//! `x` relative to the skeleton's root with every bone in its rest pose
//! ([`Bone2D::rest`](crate::Bone2D)) — so at rest every bone's matrix is the
//! mesh's own and nothing moves. A bone whose path names nothing, or whose
//! rest cannot be undone, holds its vertices where the mesh is.

use voltra_ecs::{Entity, World};
use voltra_render::glam::{Mat3, Vec2};
use voltra_render::skinning::{MAX_BONES, RIGID_SLOT};

use super::SkinnedSprite;
use crate::hierarchy::parent_of;
use crate::keyframes::target::resolve;
use crate::skeleton::skeleton_of;
use crate::{Bone2D, Transform2D};

/// One frame's matrices for a skinned mesh.
#[derive(Debug, Clone, PartialEq)]
pub struct SkinPose {
    /// Per bone in the component's list (at most
    /// [`MAX_BONES`]): bind pose to world.
    pub bones: Vec<Mat3>,
    /// The mesh's own world matrix, for vertices no bone holds.
    pub rigid: Mat3,
}

/// The pose of `skinned` on `entity`, its bones placed in the world by
/// `world_of` — the draw's own composed matrices, so the mesh lands where
/// the sprites around it do.
pub fn skin_pose(
    world: &World,
    entity: Entity,
    skinned: &SkinnedSprite,
    world_of: &dyn Fn(Entity) -> Mat3,
) -> SkinPose {
    let rigid = world_of(entity);
    let skeleton = skeleton_of(world, entity);
    let mesh_rest = skeleton.and_then(|root| rest_relative(world, root, entity));
    let bones = skinned
        .bones
        .iter()
        .take(MAX_BONES)
        .map(|path| {
            let (Some(root), Some(mesh_rest)) = (skeleton, mesh_rest) else {
                return rigid;
            };
            let Some(bone) = resolve(world, root, path) else {
                return rigid;
            };
            let Some(rest) = rest_relative(world, root, bone) else {
                return rigid;
            };
            if rest.determinant().abs() <= 1e-12 || !rest.is_finite() {
                return rigid;
            }
            world_of(bone) * rest.inverse() * mesh_rest
        })
        .collect();
    SkinPose { bones, rigid }
}

/// `entity` relative to `root` with every bone between in its rest pose:
/// the product of local matrices from just under `root` down to `entity`.
/// The identity for `root` itself; `None` when `root` is not above it.
pub fn rest_relative(world: &World, root: Entity, entity: Entity) -> Option<Mat3> {
    let mut matrix = Mat3::IDENTITY;
    let mut at = entity;
    // A cycle cannot be made through `set_parent`, but a file can say
    // anything: bounded by the entity count.
    for _ in 0..=world.entity_count() {
        if at == root {
            return Some(matrix);
        }
        let local = match world.get::<Bone2D>(at) {
            Some(bone) => bone.rest.matrix(),
            None => world
                .get::<Transform2D>(at)
                .copied()
                .unwrap_or_default()
                .matrix(),
        };
        matrix = local * matrix;
        at = parent_of(world, at)?;
    }
    None
}

/// Each of `skinned`'s bones as a segment from its origin to its tip, in
/// the mesh's own space in the bind pose — what automatic weights measure
/// distance to. A bone that resolves to nothing is a point at the origin.
pub fn bone_segments(world: &World, entity: Entity, skinned: &SkinnedSprite) -> Vec<(Vec2, Vec2)> {
    let skeleton = skeleton_of(world, entity);
    let to_mesh = skeleton
        .and_then(|root| rest_relative(world, root, entity))
        .filter(|m| m.determinant().abs() > 1e-12)
        .map(|m| m.inverse());
    skinned
        .bones
        .iter()
        .take(MAX_BONES)
        .map(|path| {
            let found = skeleton.zip(to_mesh).and_then(|(root, to_mesh)| {
                let bone = resolve(world, root, path)?;
                let length = world.get::<Bone2D>(bone).map_or(0.0, |b| b.length);
                let rest = to_mesh * rest_relative(world, root, bone)?;
                Some((
                    rest.transform_point2(Vec2::ZERO),
                    rest.transform_point2(Vec2::new(length, 0.0)),
                ))
            });
            found.unwrap_or((Vec2::ZERO, Vec2::ZERO))
        })
        .collect()
}

/// Where each of `skinned`'s vertices is drawn in the world with `pose` —
/// what `skinned.wgsl` computes, on the CPU, for the cull box and the
/// editor's handles.
pub fn deformed(skinned: &SkinnedSprite, pose: &SkinPose) -> Vec<Vec2> {
    let (vertices, _) = skinned.gpu_mesh();
    vertices
        .iter()
        .map(|v| {
            let p = Vec2::from(v.position);
            v.bones
                .iter()
                .zip(v.weights)
                .filter(|(_, w)| *w > 0.0)
                .map(|(&bone, w)| {
                    let matrix = if bone >= RIGID_SLOT {
                        pose.rigid
                    } else {
                        pose.bones.get(bone as usize).copied().unwrap_or(pose.rigid)
                    };
                    matrix.transform_point2(p) * w
                })
                .sum()
        })
        .collect()
}
