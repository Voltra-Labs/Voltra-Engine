//! The targets a follower frames: which of them are in the scene, and the
//! box around them.

use std::hash::{Hash, Hasher};

use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;

use super::super::world_position;
use super::CameraFollow2D;
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

/// What [`gather`] found of a group: those present, the ids not in the
/// scene, and whether the follower named itself.
pub(super) struct Gathered {
    present: Option<Present>,
    missing: Vec<SceneId>,
    itself: bool,
}

/// The targets of `group` that are in the scene, other than `follower`
/// itself, and the ids of those that are not.
pub(super) fn gather(world: &World, follower: Entity, group: &[SceneId]) -> Gathered {
    let mut found: Option<Present> = None;
    let mut missing = Vec::new();
    let mut itself = false;
    let mut hasher = std::hash::DefaultHasher::new();
    for &id in group {
        let Some(target) = entity_with_id(world, id) else {
            missing.push(id);
            continue;
        };
        if target == follower {
            itself = true;
            continue;
        }
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
    Gathered {
        present: found.map(|present| Present { members, ..present }),
        missing,
        itself,
    }
}

/// Says once each target newly missing — a warning when none of the group
/// is left, since then it holds still; a debug line while the rest are
/// framed, since a member despawning (an enemy killed) is how a group
/// usually shrinks — and once that the follower names itself, which it
/// leaves out; remembers both, and answers those present.
pub(super) fn say_missing(
    world: &mut World,
    entity: Entity,
    gathered: Gathered,
) -> Option<Present> {
    let Gathered {
        present,
        missing,
        itself,
    } = gathered;
    let rig = world.get_mut::<CameraFollow2D>(entity)?;
    let any_left = present.is_some();
    for id in missing.iter().filter(|id| !rig.state.missing.contains(id)) {
        if any_left {
            log::debug!("a follower's target {id:?} is not in the scene; it frames the rest");
        } else {
            log::warn!("a follower's target {id:?} is not in the scene; it holds still");
        }
    }
    if itself && !rig.state.named_itself {
        log::warn!("a follower names itself among its targets; it leaves itself out");
    }
    rig.state.missing = missing;
    rig.state.named_itself = itself;
    present
}
