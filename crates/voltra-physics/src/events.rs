//! What began and ended touching, from one step to the next.

use std::collections::BTreeMap;

use voltra_ecs::Entity;

/// Whether a pair started touching or stopped.
///
/// There is no `Stayed`: that is what [`PhysicsWorld::contacts`] already is,
/// and emitting one per resting pair per step would mean a stack of crates
/// generating events forever for standing still. Unity, Godot and Box2D all
/// report the two edges and leave the middle to whoever wants it.
///
/// [`PhysicsWorld::contacts`]: crate::PhysicsWorld::contacts
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Touch {
    Began,
    Ended,
}

/// A pair that started or stopped overlapping during a step.
///
/// `a` is always the lower entity, so a game can compare against one field or
/// the other without worrying which side of the pair it is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CollisionEvent {
    pub a: Entity,
    pub b: Entity,
    pub touch: Touch,
    /// Whether either side of the pair is a sensor, and so was detected rather
    /// than solved. A game that only wants pickups reads this; one that only
    /// wants impacts reads it too, the other way round.
    pub sensor: bool,
}

impl CollisionEvent {
    /// The other half of the pair, if `entity` is in it at all.
    ///
    /// What a game asks: an event is interesting because of what the player
    /// touched, not because of which side of the pair the player landed on.
    pub fn other(&self, entity: Entity) -> Option<Entity> {
        if self.a == entity {
            Some(self.b)
        } else if self.b == entity {
            Some(self.a)
        } else {
            None
        }
    }
}

/// The pairs that were overlapping when the last step ended.
///
/// Kept so the next step can diff against it: what is new began, what is
/// missing ended. A despawned entity needs no special case — it stops being a
/// candidate, so its pairs go missing and end like any other.
///
/// A [`BTreeMap`] rather than a hash map so the events come out in a fixed
/// order. A simulation whose event order changes between two runs of the same
/// scene is one nobody can reproduce a bug in.
#[derive(Debug, Default, Clone)]
pub struct Touching {
    pairs: BTreeMap<(Entity, Entity), bool>,
}

impl Touching {
    pub fn new() -> Self {
        Self::default()
    }

    /// Replaces what is touching with `overlapping` and reports the change.
    ///
    /// Each item is a pair and whether it is a sensor pair. Order within a
    /// pair does not matter; it is normalised here, so a narrow phase that
    /// swapped the two sides cannot report the same overlap ending and
    /// beginning again on alternate steps.
    ///
    /// Every [`Touch::Began`] comes before every [`Touch::Ended`], each group
    /// in entity order.
    pub fn update<I>(&mut self, overlapping: I) -> Vec<CollisionEvent>
    where
        I: IntoIterator<Item = (Entity, Entity, bool)>,
    {
        let now: BTreeMap<(Entity, Entity), bool> = overlapping
            .into_iter()
            .map(|(a, b, sensor)| (ordered(a, b), sensor))
            .collect();

        let mut events = Vec::new();
        for (&(a, b), &sensor) in &now {
            if !self.pairs.contains_key(&(a, b)) {
                events.push(CollisionEvent {
                    a,
                    b,
                    touch: Touch::Began,
                    sensor,
                });
            }
        }
        // The stored flag, not a recomputed one: by the time a pair ends, one
        // side may have been despawned and there is nothing left to ask.
        for (&(a, b), &sensor) in &self.pairs {
            if !now.contains_key(&(a, b)) {
                events.push(CollisionEvent {
                    a,
                    b,
                    touch: Touch::Ended,
                    sensor,
                });
            }
        }

        self.pairs = now;
        events
    }

    /// Whether these two are touching as of the last step, either way round.
    pub fn contains(&self, a: Entity, b: Entity) -> bool {
        self.pairs.contains_key(&ordered(a, b))
    }

    /// How many pairs are touching.
    pub fn len(&self) -> usize {
        self.pairs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pairs.is_empty()
    }

    /// Forgets everything, silently.
    ///
    /// For a world that is no longer the same world — a scene load, or the
    /// editor's Stop. Diffing the next step against the old scene's pairs
    /// would open the session with an `Ended` for every contact of a scene
    /// that is no longer loaded.
    pub fn clear(&mut self) {
        self.pairs.clear();
    }
}

/// The pair, lower entity first.
fn ordered(a: Entity, b: Entity) -> (Entity, Entity) {
    if a <= b {
        (a, b)
    } else {
        (b, a)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use voltra_ecs::World;

    fn two() -> (Entity, Entity) {
        let mut world = World::new();
        (world.spawn(), world.spawn())
    }

    #[test]
    fn a_new_pair_began() {
        let (a, b) = two();
        let mut touching = Touching::new();

        let events = touching.update([(a, b, false)]);

        assert_eq!(
            events,
            vec![CollisionEvent {
                a,
                b,
                touch: Touch::Began,
                sensor: false
            }]
        );
        assert!(touching.contains(b, a), "either way round");
    }

    #[test]
    fn a_pair_that_keeps_touching_says_nothing_more() {
        let (a, b) = two();
        let mut touching = Touching::new();
        touching.update([(a, b, false)]);

        assert!(touching.update([(a, b, false)]).is_empty());
        assert!(touching.update([(a, b, false)]).is_empty());
        assert_eq!(touching.len(), 1);
    }

    #[test]
    fn a_pair_that_stops_overlapping_ended() {
        let (a, b) = two();
        let mut touching = Touching::new();
        touching.update([(a, b, true)]);

        let events = touching.update([]);

        assert_eq!(
            events,
            vec![CollisionEvent {
                a,
                b,
                touch: Touch::Ended,
                sensor: true
            }],
            "and it is still remembered as a sensor pair"
        );
        assert!(touching.is_empty());
    }

    #[test]
    fn a_pair_reported_the_other_way_round_is_the_same_pair() {
        let (a, b) = two();
        let mut touching = Touching::new();
        touching.update([(a, b, false)]);

        assert!(touching.update([(b, a, false)]).is_empty());
    }

    #[test]
    fn what_began_comes_before_what_ended() {
        let mut world = World::new();
        let (a, b, c) = (world.spawn(), world.spawn(), world.spawn());
        let mut touching = Touching::new();
        touching.update([(a, b, false)]);

        let events = touching.update([(b, c, false)]);

        assert_eq!(events.len(), 2);
        assert_eq!(events[0].touch, Touch::Began);
        assert_eq!(events[1].touch, Touch::Ended);
    }

    #[test]
    fn clearing_ends_nothing() {
        let (a, b) = two();
        let mut touching = Touching::new();
        touching.update([(a, b, false)]);

        touching.clear();

        assert!(
            touching.update([]).is_empty(),
            "the pairs are gone, not ended"
        );
    }

    #[test]
    fn an_event_names_the_other_side() {
        let (a, b) = two();
        let event = CollisionEvent {
            a,
            b,
            touch: Touch::Began,
            sensor: true,
        };

        assert_eq!(event.other(a), Some(b));
        assert_eq!(event.other(b), Some(a));
    }
}
