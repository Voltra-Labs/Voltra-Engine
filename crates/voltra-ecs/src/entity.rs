//! Entity handles and their allocator.

/// A handle to an entity.
///
/// The generation is what makes this safe to keep around: indices are recycled
/// after a despawn, and without a generation an old handle would silently
/// address whatever entity took the slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Entity {
    index: u32,
    generation: u32,
}

impl Entity {
    /// Slot this entity occupies. Component storages key on this.
    pub fn index(&self) -> u32 {
        self.index
    }

    /// How many times this slot has been reused.
    pub fn generation(&self) -> u32 {
        self.generation
    }
}

/// Allocates and recycles entity handles.
#[derive(Debug, Default, Clone)]
pub struct Entities {
    /// Current generation per slot, indexed by `Entity::index`.
    generations: Vec<u32>,
    /// Whether each slot currently holds a live entity.
    alive: Vec<bool>,
    /// Slots ready for reuse. Reusing keeps indices dense, which keeps the
    /// sparse arrays in component storage small.
    free: Vec<u32>,
    live_count: usize,
}

impl Entities {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn spawn(&mut self) -> Entity {
        self.live_count += 1;

        if let Some(index) = self.free.pop() {
            let slot = index as usize;
            self.alive[slot] = true;
            return Entity {
                index,
                generation: self.generations[slot],
            };
        }

        let index = self.generations.len() as u32;
        self.generations.push(0);
        self.alive.push(true);
        Entity {
            index,
            generation: 0,
        }
    }

    /// Returns `false` if the handle was already dead or stale.
    pub fn despawn(&mut self, entity: Entity) -> bool {
        if !self.is_alive(entity) {
            return false;
        }

        let slot = entity.index as usize;
        self.alive[slot] = false;
        // Bumped on despawn rather than on reuse so every handle to this slot
        // goes stale immediately, not just once the slot is claimed again.
        self.generations[slot] = self.generations[slot].wrapping_add(1);
        self.free.push(entity.index);
        self.live_count -= 1;
        true
    }

    pub fn is_alive(&self, entity: Entity) -> bool {
        let slot = entity.index as usize;
        self.alive.get(slot).copied().unwrap_or(false)
            && self.generations[slot] == entity.generation
    }

    /// Number of live entities.
    pub fn len(&self) -> usize {
        self.live_count
    }

    pub fn is_empty(&self) -> bool {
        self.live_count == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spawn_hands_out_distinct_entities() {
        let mut entities = Entities::default();
        let a = entities.spawn();
        let b = entities.spawn();

        assert_ne!(a, b);
        assert!(entities.is_alive(a));
        assert!(entities.is_alive(b));
        assert_eq!(entities.len(), 2);
    }

    #[test]
    fn despawned_entity_is_no_longer_alive() {
        let mut entities = Entities::default();
        let e = entities.spawn();

        assert!(entities.despawn(e));
        assert!(!entities.is_alive(e));
        assert_eq!(entities.len(), 0);
    }

    #[test]
    fn despawning_twice_reports_false() {
        let mut entities = Entities::default();
        let e = entities.spawn();

        assert!(entities.despawn(e));
        assert!(!entities.despawn(e), "the second despawn is a no-op");
    }

    #[test]
    fn recycled_index_gets_a_new_generation() {
        let mut entities = Entities::default();
        let first = entities.spawn();
        entities.despawn(first);
        let second = entities.spawn();

        assert_eq!(
            second.index(),
            first.index(),
            "the free index should be reused"
        );
        assert_ne!(
            second.generation(),
            first.generation(),
            "but the generation must move on"
        );
    }

    #[test]
    fn stale_handle_does_not_address_the_recycled_entity() {
        let mut entities = Entities::default();
        let stale = entities.spawn();
        entities.despawn(stale);
        let fresh = entities.spawn();

        // This is the entire reason generations exist. Without them the stale
        // handle would silently read and write the new entity.
        assert!(!entities.is_alive(stale));
        assert!(entities.is_alive(fresh));
        assert!(!entities.despawn(stale));
        assert!(entities.is_alive(fresh), "fresh survived the stale despawn");
    }
}
