//! The impulses that survive from one step to the next.
//!
//! Warm starting — feeding a contact the impulse it needed last step — is the
//! single largest quality difference in the solver. Without it, each step
//! rediscovers the force holding a stack up from zero, the lower boxes give way
//! before the solve converges, and the stack sinks and jitters. With it, a
//! settled stack starts each step almost solved.
//!
//! That is also why `step` cannot be a pure function: something has to remember.
//!
//! How the others remember it:
//!
//! - **Box2D v3** keeps the impulses on the manifold points of the persistent
//!   contact owned by the world, matching points across steps by feature ID.
//! - **Godot** keeps a `GodotBodyPair2D` per overlapping pair; a new contact
//!   inherits the accumulated impulses of a previous one within
//!   `contact_recycle_radius` in local space, and any contact not touched this
//!   frame is erased.
//! - **Avian** keys the constraint by the `ContactId` of its contact-graph edge
//!   and stores the warm-start impulses on the manifold point.
//!
//! All three agree on the shape: the persistent unit is the *pair*, the world
//! owns it, and an entry dies when the pair stops being reported. The per-point
//! matching that separates them exists because their manifolds carry several
//! points; ours carries one, so the pair itself is the key.
//!
//! When 11b-3 gives a manifold its second point, this key gains a point
//! identifier — which is exactly the problem Box2D's feature IDs and Godot's
//! recycle radius solve, and the reason both exist.

use std::collections::HashMap;

use voltra_ecs::Entity;

/// What one contact accumulated over a step.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct CachedImpulse {
    pub normal: f32,
    pub tangent: f32,
}

/// The pair a cached impulse belongs to.
///
/// Ordered as the broad phase emits it — `(a, b)` with `a` the lower entity,
/// never mirrored — so a pair has one key rather than two. `Entity` carries a
/// generation, so a recycled index cannot inherit a dead entity's impulses.
pub type ContactKey = (Entity, Entity);

/// Contact impulses, kept for exactly as long as their pair keeps touching.
///
/// Two maps rather than one: `current` is what this step reads, `next` is what
/// it writes, and [`ImpulseCache::commit`] swaps them. Eviction then costs
/// nothing and cannot be forgotten — a pair that separated, a body that was
/// despawned and a scene that was reloaded all disappear by the same rule,
/// because none of them recorded anything this step.
#[derive(Debug, Default)]
pub struct ImpulseCache {
    current: HashMap<ContactKey, CachedImpulse>,
    next: HashMap<ContactKey, CachedImpulse>,
}

impl ImpulseCache {
    /// What this pair accumulated last step, or zeroes for a new contact.
    pub fn warm_start(&self, key: ContactKey) -> CachedImpulse {
        self.current.get(&key).copied().unwrap_or_default()
    }

    /// Remembers what this pair accumulated, for the next step to start from.
    pub fn record(&mut self, key: ContactKey, impulse: CachedImpulse) {
        self.next.insert(key, impulse);
    }

    /// Ends the step: what was recorded becomes what is read, and everything
    /// not recorded is gone.
    pub fn commit(&mut self) {
        std::mem::swap(&mut self.current, &mut self.next);
        self.next.clear();
    }

    /// Pairs currently warm. Their number is bounded by the contacts of the
    /// last step, which is what makes it safe to keep this in a long-lived
    /// world.
    pub fn len(&self) -> usize {
        self.current.len()
    }

    pub fn is_empty(&self) -> bool {
        self.current.is_empty()
    }

    /// Forgets everything, for a world that is no longer the same world —
    /// a scene load, or physics being switched off and on.
    pub fn clear(&mut self) {
        self.current.clear();
        self.next.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use voltra_ecs::World;

    fn two_entities() -> (Entity, Entity) {
        let mut world = World::new();
        (world.spawn(), world.spawn())
    }

    #[test]
    fn an_unknown_pair_starts_from_zero() {
        let (a, b) = two_entities();

        assert_eq!(
            ImpulseCache::default().warm_start((a, b)),
            CachedImpulse::default()
        );
    }

    #[test]
    fn a_recorded_pair_is_returned_once_the_step_is_committed() {
        let (a, b) = two_entities();
        let mut cache = ImpulseCache::default();
        let impulse = CachedImpulse {
            normal: 3.0,
            tangent: -1.0,
        };

        cache.record((a, b), impulse);
        assert_eq!(
            cache.warm_start((a, b)),
            CachedImpulse::default(),
            "a step must not read what it is still writing"
        );

        cache.commit();
        assert_eq!(cache.warm_start((a, b)), impulse);
    }

    #[test]
    fn a_pair_that_stops_touching_is_forgotten() {
        let (a, b) = two_entities();
        let mut cache = ImpulseCache::default();

        cache.record(
            (a, b),
            CachedImpulse {
                normal: 3.0,
                tangent: 0.0,
            },
        );
        cache.commit();
        cache.commit(); // a step in which nothing was recorded

        assert!(cache.is_empty());
        assert_eq!(cache.warm_start((a, b)), CachedImpulse::default());
    }

    #[test]
    fn a_pair_that_keeps_touching_is_kept() {
        let (a, b) = two_entities();
        let mut cache = ImpulseCache::default();

        for step in 1..=10 {
            cache.record(
                (a, b),
                CachedImpulse {
                    normal: step as f32,
                    tangent: 0.0,
                },
            );
            cache.commit();
        }

        assert_eq!(cache.len(), 1);
        assert_eq!(cache.warm_start((a, b)).normal, 10.0);
    }

    #[test]
    fn clearing_forgets_both_the_committed_and_the_pending() {
        let (a, b) = two_entities();
        let mut cache = ImpulseCache::default();
        cache.record(
            (a, b),
            CachedImpulse {
                normal: 1.0,
                tangent: 0.0,
            },
        );
        cache.commit();
        cache.record(
            (a, b),
            CachedImpulse {
                normal: 2.0,
                tangent: 0.0,
            },
        );

        cache.clear();
        cache.commit();

        assert!(cache.is_empty());
    }
}
