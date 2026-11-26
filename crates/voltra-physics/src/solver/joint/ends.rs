//! Which bodies each joint joins, found in the world.

use std::collections::HashMap;

use voltra_ecs::{Entity, World};
use voltra_scene::{Joint2D, JointKind, SceneId};

/// The two bodies a joint joins, found in the world.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct JointEnds {
    /// The entity carrying the joint.
    pub owner: Entity,
    /// The first body the joint joins.
    pub a: Entity,
    /// `None` for the world.
    pub b: Option<Entity>,
    /// Whether the two still collide with each other.
    pub collide_connected: bool,
    /// Whether a motor turns it: its island never sleeps.
    pub driven: bool,
}

/// Every joint in `world` whose bodies are there. A joint naming a body the
/// world does not hold, or the same body twice, joins nothing — rather than
/// holding to the world, which nobody asked for.
pub fn joint_ends(world: &World) -> Vec<JointEnds> {
    let mut joints = world.query::<Joint2D>().peekable();
    if joints.peek().is_none() {
        return Vec::new();
    }
    let ids: HashMap<SceneId, Entity> = world.query::<SceneId>().map(|(e, id)| (*id, e)).collect();
    joints
        .filter_map(|(owner, joint)| {
            let a = joint.a.map_or(Some(owner), |id| ids.get(&id).copied())?;
            let b = match joint.b {
                None => None,
                Some(id) => Some(*ids.get(&id)?),
            };
            if b == Some(a) {
                return None;
            }
            let driven = matches!(
                joint.kind,
                JointKind::Revolute { motor: Some(m), .. } if m.speed != 0.0 && m.max_torque > 0.0
            );
            Some(JointEnds {
                owner,
                a,
                b,
                collide_connected: joint.collide_connected,
                driven,
            })
        })
        .collect()
}
