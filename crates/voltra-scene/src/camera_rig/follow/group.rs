//! The targets a follower frames: which of them are in the scene, and the
//! box around them.

use std::hash::{Hash, Hasher};

use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;

use super::super::world_position;
use crate::format::entity_with_id;
use crate::scene_id::SceneId;

/// The targets of a group found in the scene this frame.
pub(super) struct Present {
    /// The smallest x and y of their positions.
    pub min: Vec2,
    /// The largest x and y of their positions.
    pub max: Vec2,
    /// Which of them were found, hashed in order: when it changes, the
    /// centre jumps, and that jump is not a velocity to lead by.
    pub members: u64,
}

impl Present {
    /// The centre of the box around them: what the follower aims at.
    pub fn centre(&self) -> Vec2 {
        (self.min + self.max) * 0.5
    }
}

/// The targets of `group` that are in the scene, other than `follower`
/// itself, and the ids of those that are not. `None` when none are.
pub(super) fn gather(
    world: &World,
    follower: Entity,
    group: &[SceneId],
) -> (Option<Present>, Vec<SceneId>) {
    let mut found: Option<Present> = None;
    let mut missing = Vec::new();
    let mut hasher = std::hash::DefaultHasher::new();
    for &id in group {
        let Some(target) = entity_with_id(world, id).filter(|&t| t != follower) else {
            missing.push(id);
            continue;
        };
        let at = world_position(world, target);
        if !at.is_finite() {
            missing.push(id);
            continue;
        }
        id.hash(&mut hasher);
        found = Some(match found {
            None => Present {
                min: at,
                max: at,
                members: 0,
            },
            Some(box_) => Present {
                min: box_.min.min(at),
                max: box_.max.max(at),
                members: 0,
            },
        });
    }
    let members = hasher.finish();
    (found.map(|present| Present { members, ..present }), missing)
}
