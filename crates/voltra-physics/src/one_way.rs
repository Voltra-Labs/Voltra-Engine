//! Which pairs a one-way collider is letting through.
//!
//! A body meeting a [`OneWay`] collider from any side but its solid one
//! passes through it — and keeps passing, step after step, until the two
//! no longer overlap. Without the memory, a body rising through a platform
//! would be caught the step its overlap's normal first turned to face up,
//! halfway through, and popped onto the top. Unity's effector and Box2D's
//! one-way sample keep the same state.

use voltra_ecs::{Entity, World};
use voltra_scene::{OneWay, Transform};

use crate::hash::FastSet;
use crate::narrow::Manifold;

/// The pairs being let through, carried from one step to the next.
#[derive(Debug, Default, Clone)]
pub struct OneWayPasses {
    passing: FastSet<(Entity, Entity)>,
    /// This step's, swapped in at its end.
    next: FastSet<(Entity, Entity)>,
}

impl OneWayPasses {
    /// Whether the overlap of `a` and `b` — `manifold`, its normal pushing
    /// `a` away from `b` — is a contact, rather than a pass through a
    /// one-way collider on either side. Records a pass for the next step.
    pub(crate) fn admits(
        &mut self,
        world: &World,
        a: Entity,
        b: Entity,
        manifold: &Manifold,
    ) -> bool {
        let one_way_a = side(world, a);
        let one_way_b = side(world, b);
        if one_way_a.is_none() && one_way_b.is_none() {
            return true;
        }
        let pair = if a.index() <= b.index() {
            (a, b)
        } else {
            (b, a)
        };
        let met = !self.passing.contains(&pair)
            && one_way_b.is_none_or(|(platform, at)| platform.admits(at, manifold.normal))
            && one_way_a.is_none_or(|(platform, at)| platform.admits(at, -manifold.normal));
        if !met {
            self.next.insert(pair);
        }
        met
    }

    /// Ends a step: what was let through and still overlaps is remembered,
    /// and nothing else — a pair apart, or despawned, is forgotten.
    pub(crate) fn end_step(&mut self) {
        std::mem::swap(&mut self.passing, &mut self.next);
        self.next.clear();
    }

    /// Forgets everything, for a world that is no longer the same world.
    pub fn clear(&mut self) {
        self.passing.clear();
        self.next.clear();
    }

    /// Whether `a` and `b` are being let through, as of the last step.
    pub(crate) fn is_passing(&self, a: Entity, b: Entity) -> bool {
        let pair = if a.index() <= b.index() {
            (a, b)
        } else {
            (b, a)
        };
        self.passing.contains(&pair) || self.next.contains(&pair)
    }

    /// How many pairs are passing through.
    pub fn len(&self) -> usize {
        self.passing.len()
    }

    /// Whether no pair is passing through.
    pub fn is_empty(&self) -> bool {
        self.passing.is_empty()
    }
}

fn side(world: &World, entity: Entity) -> Option<(&OneWay, &Transform)> {
    Some((
        world.get::<OneWay>(entity)?,
        world.get::<Transform>(entity)?,
    ))
}
