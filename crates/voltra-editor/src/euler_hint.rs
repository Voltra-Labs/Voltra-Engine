//! The Euler angles the inspector shows for a 3D rotation, as they were
//! typed.
//!
//! A `Transform3D` stores a quaternion, and one rotation has many Euler
//! spellings: a yaw of 180° reads back from the quaternion as a pitch and a
//! roll of 180° each, or as −180°. An inspector that recomputed the angles
//! from the quaternion every frame would make a field typed as 180 jump to
//! its equivalent the moment it was left. So the editor keeps, per entity,
//! the angles last shown and the rotation they were shown for — Unity's
//! `m_LocalEulerAnglesHint`, kept here rather than in the file, since the
//! file's quaternion is exact and needs none of it. When the rotation
//! changes under the hint — a gizmo, an undo, a script — the hint is stale
//! and the angles are read from the quaternion again.

use std::collections::HashMap;

use voltra_ecs::{Entity, World};
use voltra_render::glam::{Quat, Vec3};
use voltra_scene::Transform3D;

/// The angles shown for one entity, and the rotation they stand for.
#[derive(Debug, Clone, Copy)]
struct Hint {
    rotation: Quat,
    degrees: Vec3,
}

/// How many of an entity's past hints are kept, newest first: enough for an
/// undo and a redo across a few typed rotations to come back to the angles
/// each was typed as, rather than only the last.
const KEPT: usize = 8;

/// Every inspected 3D entity's shown angles.
#[derive(Debug, Default)]
pub(crate) struct EulerHints {
    by_entity: HashMap<Entity, Vec<Hint>>,
}

impl EulerHints {
    /// The angles to show for `entity`, whose rotation is now `rotation`:
    /// the ones typed or shown for that very rotation, if it is one of the
    /// last few it had, and the quaternion's own otherwise.
    pub(crate) fn degrees(&mut self, entity: Entity, rotation: Quat) -> Vec3 {
        let kept = self.by_entity.entry(entity).or_default();
        if let Some(hint) = kept.iter().find(|hint| hint.rotation == rotation) {
            return hint.degrees;
        }
        let degrees = Transform3D::default()
            .with_rotation(rotation)
            .euler_degrees();
        remember(kept, Hint { rotation, degrees });
        degrees
    }

    /// `degrees` typed for `entity`: the rotation to write, remembered with
    /// the angles that made it.
    pub(crate) fn typed(&mut self, entity: Entity, degrees: Vec3) -> Quat {
        let rotation = Transform3D::from_euler_degrees(degrees).rotation;
        remember(
            self.by_entity.entry(entity).or_default(),
            Hint { rotation, degrees },
        );
        rotation
    }

    /// Forgets the entities that are gone: an index the allocator hands out
    /// again comes with a new generation, so a hint never names the wrong
    /// entity, but it would be kept for ever.
    pub(crate) fn forget_dead(&mut self, world: &World) {
        self.by_entity.retain(|&entity, _| world.is_alive(entity));
    }
}

/// `hint` as the newest of `kept`, replacing one for the same rotation and
/// dropping the oldest past [`KEPT`].
fn remember(kept: &mut Vec<Hint>, hint: Hint) {
    kept.retain(|old| old.rotation != hint.rotation);
    kept.insert(0, hint);
    kept.truncate(KEPT);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entity() -> (World, Entity) {
        let mut world = World::new();
        let e = world.spawn();
        (world, e)
    }

    #[test]
    fn a_typed_half_turn_reads_back_as_typed() {
        // 180° of yaw is, as the quaternion reads it, a pitch and a roll of
        // 180° and a yaw of about 0: the field must not jump to that.
        let (_, e) = entity();
        let mut hints = EulerHints::default();
        let typed = Vec3::new(0.0, 180.0, 0.0);
        let rotation = hints.typed(e, typed);

        assert_eq!(hints.degrees(e, rotation), typed);
        let raw = Transform3D::default()
            .with_rotation(rotation)
            .euler_degrees();
        assert!(
            (raw - typed).length() > 1.0,
            "the quaternion alone reads {raw:?}: the hint is what keeps 180"
        );
        // And the hint is the same rotation, to a millionth.
        let again = Transform3D::from_euler_degrees(hints.degrees(e, rotation)).rotation;
        assert!(again.abs_diff_eq(rotation, 1e-6));
    }

    #[test]
    fn a_rotation_changed_elsewhere_is_read_from_the_quaternion() {
        let (_, e) = entity();
        let mut hints = EulerHints::default();
        hints.typed(e, Vec3::new(0.0, 180.0, 0.0));

        let turned = Quat::from_rotation_x(30f32.to_radians());
        let shown = hints.degrees(e, turned);

        assert!(
            (shown - Vec3::new(30.0, 0.0, 0.0)).length() < 1e-4,
            "{shown:?}"
        );
    }

    #[test]
    fn an_angle_past_a_full_turn_is_kept() {
        let (_, e) = entity();
        let mut hints = EulerHints::default();
        let typed = Vec3::new(10.0, 450.0, -720.0);
        let rotation = hints.typed(e, typed);
        assert_eq!(hints.degrees(e, rotation), typed);
    }

    #[test]
    fn undo_then_redo_comes_back_to_each_typed_angle() {
        let (_, e) = entity();
        let mut hints = EulerHints::default();
        let first = hints.typed(e, Vec3::new(0.0, 180.0, 0.0));
        let second = hints.typed(e, Vec3::new(180.0, 0.0, 0.0));

        // Undo puts the first rotation back, redo the second.
        assert_eq!(hints.degrees(e, first), Vec3::new(0.0, 180.0, 0.0));
        assert_eq!(hints.degrees(e, second), Vec3::new(180.0, 0.0, 0.0));
        assert_eq!(hints.degrees(e, first), Vec3::new(0.0, 180.0, 0.0));
    }

    #[test]
    fn only_the_last_few_are_kept() {
        let (_, e) = entity();
        let mut hints = EulerHints::default();
        for yaw in 0..20 {
            hints.typed(e, Vec3::new(0.0, yaw as f32 * 5.0 + 360.0, 0.0));
        }
        assert_eq!(hints.by_entity[&e].len(), KEPT);
    }

    #[test]
    fn the_dead_are_forgotten() {
        let (mut world, e) = entity();
        let mut hints = EulerHints::default();
        hints.typed(e, Vec3::ONE);
        world.despawn(e);
        hints.forget_dead(&world);
        assert!(hints.by_entity.is_empty());
    }
}
