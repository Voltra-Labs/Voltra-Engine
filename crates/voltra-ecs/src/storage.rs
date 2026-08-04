//! Dense storage for one component type.

use crate::entity::Entity;

/// Dense storage for one component type, addressed by entity.
///
/// Components live packed in `dense`, so iteration is contiguous. `sparse` maps
/// an entity index to its slot in `dense`, giving O(1) lookup at the cost of an
/// array as long as the highest entity index seen.
#[derive(Debug, Clone)]
pub struct SparseSet<T> {
    /// Entity index -> dense slot. `None` means the entity has no component.
    sparse: Vec<Option<usize>>,
    /// Dense slot -> owning entity, kept parallel to `dense`. This is what
    /// makes swap-remove and iteration possible.
    entities: Vec<Entity>,
    dense: Vec<T>,
}

// Derived Default would demand `T: Default`, which components need not be.
impl<T> Default for SparseSet<T> {
    fn default() -> Self {
        Self {
            sparse: Vec::new(),
            entities: Vec::new(),
            dense: Vec::new(),
        }
    }
}

impl<T> SparseSet<T> {
    pub fn new() -> Self {
        Self::default()
    }

    /// Dense slot for a live handle, or `None` if absent or stale.
    fn slot_of(&self, entity: Entity) -> Option<usize> {
        let slot = (*self.sparse.get(entity.index() as usize)?)?;
        // Generation check: the index may have been recycled since.
        (self.entities[slot] == entity).then_some(slot)
    }

    /// Inserts a component, returning the value it replaced.
    pub fn insert(&mut self, entity: Entity, value: T) -> Option<T> {
        if let Some(slot) = self.slot_of(entity) {
            return Some(std::mem::replace(&mut self.dense[slot], value));
        }

        let index = entity.index() as usize;
        if index >= self.sparse.len() {
            self.sparse.resize(index + 1, None);
        }
        self.sparse[index] = Some(self.dense.len());
        self.entities.push(entity);
        self.dense.push(value);
        None
    }

    pub fn get(&self, entity: Entity) -> Option<&T> {
        self.slot_of(entity).map(|slot| &self.dense[slot])
    }

    pub fn get_mut(&mut self, entity: Entity) -> Option<&mut T> {
        self.slot_of(entity).map(|slot| &mut self.dense[slot])
    }

    /// Removes a component, returning it.
    pub fn remove(&mut self, entity: Entity) -> Option<T> {
        let slot = self.slot_of(entity)?;

        self.sparse[entity.index() as usize] = None;
        self.entities.swap_remove(slot);
        let value = self.dense.swap_remove(slot);

        // swap_remove moved the last element into `slot`; its sparse entry
        // still points at the old tail. Repairing it is the whole trick.
        if slot < self.entities.len() {
            let moved = self.entities[slot];
            self.sparse[moved.index() as usize] = Some(slot);
        }

        Some(value)
    }

    pub fn contains(&self, entity: Entity) -> bool {
        self.slot_of(entity).is_some()
    }

    pub fn len(&self) -> usize {
        self.dense.len()
    }

    pub fn is_empty(&self) -> bool {
        self.dense.is_empty()
    }

    /// Iterates every `(entity, component)` pair in dense order.
    pub fn iter(&self) -> impl Iterator<Item = (Entity, &T)> {
        self.entities.iter().copied().zip(self.dense.iter())
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (Entity, &mut T)> {
        self.entities.iter().copied().zip(self.dense.iter_mut())
    }

    /// The entities holding this component, in dense order.
    pub fn entities(&self) -> &[Entity] {
        &self.entities
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::Entities;

    fn three() -> (Entities, Entity, Entity, Entity) {
        let mut entities = Entities::new();
        let a = entities.spawn();
        let b = entities.spawn();
        let c = entities.spawn();
        (entities, a, b, c)
    }

    #[test]
    fn insert_then_get_returns_the_value() {
        let (_e, a, _b, _c) = three();
        let mut set = SparseSet::<i32>::default();

        assert_eq!(set.insert(a, 7), None);
        assert_eq!(set.get(a), Some(&7));
        assert_eq!(set.len(), 1);
    }

    #[test]
    fn inserting_twice_replaces_and_returns_the_old_value() {
        let (_e, a, _b, _c) = three();
        let mut set = SparseSet::default();

        set.insert(a, 1);
        assert_eq!(set.insert(a, 2), Some(1));
        assert_eq!(set.get(a), Some(&2));
        assert_eq!(set.len(), 1, "replacing must not grow the set");
    }

    #[test]
    fn get_mut_writes_through() {
        let (_e, a, _b, _c) = three();
        let mut set = SparseSet::default();
        set.insert(a, 1);

        *set.get_mut(a).unwrap() = 9;

        assert_eq!(set.get(a), Some(&9));
    }

    #[test]
    fn missing_entity_reads_as_none() {
        let (_e, a, b, _c) = three();
        let mut set = SparseSet::<i32>::default();
        set.insert(a, 1);

        assert_eq!(set.get(b), None);
        assert_eq!(set.remove(b), None);
    }

    #[test]
    fn remove_returns_the_value_and_shrinks() {
        let (_e, a, _b, _c) = three();
        let mut set = SparseSet::default();
        set.insert(a, 5);

        assert_eq!(set.remove(a), Some(5));
        assert_eq!(set.get(a), None);
        assert!(set.is_empty());
    }

    #[test]
    fn removing_from_the_middle_keeps_the_moved_entity_reachable() {
        let (_e, a, b, c) = three();
        let mut set = SparseSet::default();
        set.insert(a, 10);
        set.insert(b, 20);
        set.insert(c, 30);

        // `a` sits at dense slot 0, so swap-remove moves `c` into it. Forget to
        // repair c's sparse entry and it silently points at the wrong slot —
        // the classic sparse-set bug.
        assert_eq!(set.remove(a), Some(10));

        assert_eq!(set.get(c), Some(&30), "the moved element must follow");
        assert_eq!(set.get(b), Some(&20));
        assert_eq!(set.get(a), None);
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn a_stale_handle_cannot_read_a_recycled_slot() {
        let mut entities = Entities::new();
        let stale = entities.spawn();
        let mut set = SparseSet::default();
        set.insert(stale, 1);
        set.remove(stale);

        entities.despawn(stale);
        let fresh = entities.spawn();
        set.insert(fresh, 2);

        assert_eq!(set.get(stale), None, "same index, older generation");
        assert_eq!(set.get(fresh), Some(&2));
    }

    #[test]
    fn iteration_yields_every_pair() {
        let (_e, a, b, _c) = three();
        let mut set = SparseSet::default();
        set.insert(a, 1);
        set.insert(b, 2);

        let mut pairs: Vec<(Entity, i32)> = set.iter().map(|(e, v)| (e, *v)).collect();
        pairs.sort();

        assert_eq!(pairs, vec![(a, 1), (b, 2)]);
    }

    #[test]
    fn iter_mut_writes_through() {
        let (_e, a, b, _c) = three();
        let mut set = SparseSet::default();
        set.insert(a, 1);
        set.insert(b, 2);

        for (_, value) in set.iter_mut() {
            *value *= 10;
        }

        assert_eq!(set.get(a), Some(&10));
        assert_eq!(set.get(b), Some(&20));
    }
}
