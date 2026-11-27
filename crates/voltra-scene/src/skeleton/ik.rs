//! Two-bone IK: a limb — an upper bone and the bone hung from it — bent so
//! the lower bone's tip reaches a target.
//!
//! Unity's `LimbSolver2D` and Godot's `SkeletonModification2DTwoBoneIK`:
//! the law of cosines gives the elbow, each bone is turned to point where
//! it should, and a target out of reach gets the limb straight towards it.
//! The component sits on the **lower** bone (the effector's end, as
//! Unity's solver does); the upper bone is its parent. The target is named
//! by path from the skeleton's root, as a clip's tracks are, so every copy
//! of a character prefab reaches for its own.
//!
//! Applied after the clips, while the world runs: a clip poses the limb
//! and IK has the last word on the bones it drives.

use serde::{Deserialize, Serialize};
use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;

use super::{skeleton_of, Bone2D};
use crate::hierarchy::{parent_matrix, parent_of, world_matrix};
use crate::keyframes::target::resolve;

/// Bends the limb ending in this bone towards a target.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TwoBoneIk {
    /// The entity the tip reaches for, by path of names from the
    /// skeleton's root (`"target"`, `"rig/left_foot_goal"`).
    pub target: String,
    /// Bend the other way: an elbow out rather than in, a knee forward
    /// rather than back.
    #[serde(default)]
    pub flip: bool,
    /// Whether it bends anything; off, the clip's pose stands.
    #[serde(default = "enabled")]
    pub enabled: bool,
}

impl TwoBoneIk {
    /// The registered name a scene file stores it under.
    pub const NAME: &'static str = "TwoBoneIk";
}

fn enabled() -> bool {
    true
}

impl Default for TwoBoneIk {
    fn default() -> Self {
        Self {
            target: "target".to_owned(),
            flip: false,
            enabled: true,
        }
    }
}

/// Why a limb is not being solved, for an inspector to say.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IkProblem {
    /// The entity with the IK has no [`Bone2D`].
    NotABone,
    /// Its parent is not a bone: there is no upper bone to bend.
    NoUpperBone,
    /// It is under no [`Skeleton2D`](super::Skeleton2D) to name the target
    /// from.
    NoSkeleton,
    /// No entity has the target's path under the skeleton.
    NoTarget,
}

/// Where the elbow goes for a limb rooted at `root` with bones `upper` and
/// `lower` long to reach `target`: on the left of the reach, or the right
/// with `flip`. A target too far — or too near — leaves the limb as close
/// as it gets, straight towards it; a target on the root points it along
/// +x.
pub fn solve_two_bone(root: Vec2, upper: f32, lower: f32, target: Vec2, flip: bool) -> Vec2 {
    let to = target - root;
    let length = to.length();
    let direction = if length > 1e-6 { to / length } else { Vec2::X };
    let reach = length.clamp((upper - lower).abs(), upper + lower);
    let cos = if reach > 1e-6 {
        ((upper * upper + reach * reach - lower * lower) / (2.0 * upper * reach)).clamp(-1.0, 1.0)
    } else {
        1.0
    };
    let bend = if flip { -cos.acos() } else { cos.acos() };
    root + Vec2::from_angle(bend).rotate(direction) * upper
}

/// What stops `lower`'s IK from solving, if anything.
pub fn problem(world: &World, lower: Entity) -> Option<IkProblem> {
    chain(world, lower).err()
}

/// The upper bone and the target of `lower`'s IK.
fn chain(world: &World, lower: Entity) -> Result<(Entity, Entity), IkProblem> {
    let ik = world.get::<TwoBoneIk>(lower).ok_or(IkProblem::NotABone)?;
    world.get::<Bone2D>(lower).ok_or(IkProblem::NotABone)?;
    let upper = parent_of(world, lower)
        .filter(|&p| world.get::<Bone2D>(p).is_some())
        .ok_or(IkProblem::NoUpperBone)?;
    let skeleton = skeleton_of(world, lower).ok_or(IkProblem::NoSkeleton)?;
    let target = resolve(world, skeleton, &ik.target)
        .filter(|&t| t != lower && t != upper)
        .ok_or(IkProblem::NoTarget)?;
    Ok((upper, target))
}

/// Bends every enabled limb towards its target. One that cannot be solved
/// is left as it is; [`problem`] says why.
pub fn apply(world: &mut World) {
    let limbs: Vec<Entity> = world
        .query::<TwoBoneIk>()
        .filter(|(_, ik)| ik.enabled)
        .map(|(entity, _)| entity)
        .collect();
    for lower in limbs {
        solve(world, lower);
    }
}

fn solve(world: &mut World, lower: Entity) {
    let Ok((upper, target)) = chain(world, lower) else {
        return;
    };
    let (Some(ik), Some(bone)) = (
        world.get::<TwoBoneIk>(lower).cloned(),
        world.get::<Bone2D>(lower).copied(),
    ) else {
        return;
    };
    let goal = world_matrix(world, target).transform_point2(Vec2::ZERO);
    let root = world_matrix(world, upper).transform_point2(Vec2::ZERO);
    let (joint, tip) = ends(world, lower, bone.length);
    let (upper_length, lower_length) = (root.distance(joint), joint.distance(tip));
    if !(goal.is_finite() && upper_length > 1e-6 && lower_length > 1e-6) {
        return;
    }
    let elbow = solve_two_bone(root, upper_length, lower_length, goal, ik.flip);
    turn(world, upper, angle_between(joint - root, elbow - root));
    let (joint, tip) = ends(world, lower, bone.length);
    if goal.distance(joint) > 1e-6 {
        turn(world, lower, angle_between(tip - joint, goal - joint));
    }
}

/// Where `bone` starts and where its tip is, in the world.
fn ends(world: &World, bone: Entity, length: f32) -> (Vec2, Vec2) {
    let matrix = world_matrix(world, bone);
    (
        matrix.transform_point2(Vec2::ZERO),
        matrix.transform_point2(Vec2::new(length, 0.0)),
    )
}

/// The signed angle from `from` to `to`, in `-π..=π`.
fn angle_between(from: Vec2, to: Vec2) -> f32 {
    from.perp_dot(to).atan2(from.dot(to))
}

/// Turns `entity` by `angle` in the world, through its parent — whose
/// mirroring, if it has any, turns the other way.
fn turn(world: &mut World, entity: Entity, angle: f32) {
    if !angle.is_finite() {
        return;
    }
    let mirrored = parent_matrix(world, entity).determinant() < 0.0;
    if let Some(transform) = world.get_mut::<crate::Transform>(entity) {
        transform.rotation += if mirrored { -angle } else { angle };
    }
}
