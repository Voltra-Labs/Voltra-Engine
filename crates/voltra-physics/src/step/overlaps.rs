//! What a collide found.

use voltra_ecs::Entity;

use crate::narrow::Contact;

/// What a collide found: the contacts to solve, and the sensor pairs to
/// report and nothing else.
///
/// Split here rather than filtered later so a sensor never reaches the solver,
/// never enters the impulse cache and never appears in
/// [`PhysicsWorld::contacts`] — which stays "what the solver resolved", so a
/// character cannot stand on a trigger and a ground check needs no idea that
/// sensors exist.
///
/// [`PhysicsWorld::contacts`]: crate::PhysicsWorld::contacts
#[derive(Debug, Default, Clone)]
pub struct Overlaps {
    /// Solid overlaps, in the order the broad phase found them.
    pub contacts: Vec<Contact>,
    /// Overlaps where at least one side is a [`Sensor`].
    pub sensors: Vec<(Entity, Entity)>,
}

impl Overlaps {
    /// Every overlapping pair and whether it is a sensor pair, which is what
    /// [`Touching::update`] diffs into events.
    ///
    /// [`Touching::update`]: crate::events::Touching::update
    pub fn pairs(&self) -> impl Iterator<Item = (Entity, Entity, bool)> + '_ {
        self.contacts
            .iter()
            .map(|contact| (contact.a, contact.b, false))
            .chain(self.sensors.iter().map(|&(a, b)| (a, b, true)))
    }
}
