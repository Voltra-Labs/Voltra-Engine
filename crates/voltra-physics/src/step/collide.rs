//! Finding a step's overlaps: the broad phase's pairs through the narrow
//! phase, with sleeping pairs reused and jointed ones left out.

use voltra_ecs::{Entity, World};
use voltra_scene::{Collider, Sensor, Transform};

use super::Overlaps;
use crate::broad::candidate_pairs;
use crate::narrow::{manifold, Contact};
use crate::one_way::OneWayPasses;
use crate::solver::JointEnds;

/// Every overlap in the world right now, sensors kept apart from the rest.
#[cfg(test)]
pub(crate) fn collide(world: &World) -> Overlaps {
    collide_with(world, |_, _| Reuse::Test, &mut OneWayPasses::default())
}

/// What [`collide_with`] does with a candidate pair.
pub(super) enum Reuse {
    /// Run the narrow phase.
    Test,
    /// Take this contact instead — a sleeping pair's.
    Contact(Contact),
    /// Leave it out — two bodies a joint joins.
    Skip,
}

/// A pair in one order, whichever it arrives in.
pub(super) fn ordered(a: Entity, b: Entity) -> (Entity, Entity) {
    if a.index() <= b.index() {
        (a, b)
    } else {
        (b, a)
    }
}

/// The pairs joints keep from colliding: every joint between two bodies
/// that does not ask to collide.
pub(super) fn unjoined(joints: &[JointEnds]) -> crate::hash::FastSet<(Entity, Entity)> {
    joints
        .iter()
        .filter(|joint| !joint.collide_connected)
        .filter_map(|joint| joint.b.map(|b| ordered(joint.a, b)))
        .collect()
}

/// [`collide`], asking `reuse` first about each pair — how sleeping pairs
/// skip the narrow phase, and jointed ones are left out.
pub(super) fn collide_with(
    world: &World,
    reuse: impl Fn(Entity, Entity) -> Reuse,
    passes: &mut OneWayPasses,
) -> Overlaps {
    let mut overlaps = Overlaps::default();
    for (a, b) in candidate_pairs(world) {
        match reuse(a, b) {
            Reuse::Test => {}
            Reuse::Contact(contact) => {
                overlaps.contacts.push(contact);
                continue;
            }
            Reuse::Skip => continue,
        }
        let Some(a_shape) = shape(world, a) else {
            continue;
        };
        let Some(b_shape) = shape(world, b) else {
            continue;
        };
        let Some(manifold) = manifold(a_shape, b_shape) else {
            continue;
        };

        // Either side being a sensor makes the pair one: a trigger volume must
        // not push the player out, and a player must not be stopped by it.
        if world.get::<Sensor>(a).is_some() || world.get::<Sensor>(b).is_some() {
            overlaps.sensors.push((a, b));
        } else if passes.admits(world, a, b, &manifold) {
            overlaps.contacts.push(Contact::new(a, b, manifold));
        }
    }
    passes.end_step();
    overlaps
}

/// The shape `entity` collides with, if it has one and sits somewhere.
fn shape(world: &World, entity: Entity) -> Option<(&Collider, &Transform)> {
    Some((
        world.get::<Collider>(entity)?,
        world.get::<Transform>(entity)?,
    ))
}
