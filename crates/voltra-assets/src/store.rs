//! A generational arena of assets of one type.

use crate::handle::Handle;

/// Stores assets of one type and hands out [`Handle`]s to them.
///
/// Knows nothing about paths, files or an asset root — that is
/// [`Textures`](crate::textures::Textures)' job, and the split is what lets
/// this be tested without a GPU.
#[derive(Debug)]
pub struct Assets<T> {
    slots: Vec<Option<T>>,
    /// How many times each slot has been reused. Indexed alongside `slots`.
    generations: Vec<u32>,
    /// Slots ready for reuse, so an arena that churns does not grow forever.
    free: Vec<u32>,
    live: usize,
}

impl<T> Default for Assets<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Assets<T> {
    pub fn new() -> Self {
        Self {
            slots: Vec::new(),
            generations: Vec::new(),
            free: Vec::new(),
            live: 0,
        }
    }

    /// Stores `asset` and returns a handle to it.
    pub fn insert(&mut self, asset: T) -> Handle<T> {
        self.live += 1;

        if let Some(index) = self.free.pop() {
            let slot = index as usize;
            self.slots[slot] = Some(asset);
            return Handle::new(index, self.generations[slot]);
        }

        let index = self.slots.len() as u32;
        self.slots.push(Some(asset));
        self.generations.push(0);
        Handle::new(index, 0)
    }

    /// `None` if the handle is stale, already removed, or from another store.
    pub fn get(&self, handle: Handle<T>) -> Option<&T> {
        self.slot(handle).and_then(|slot| self.slots[slot].as_ref())
    }

    /// `None` if the handle is stale, already removed, or from another store.
    pub fn get_mut(&mut self, handle: Handle<T>) -> Option<&mut T> {
        let slot = self.slot(handle)?;
        self.slots[slot].as_mut()
    }

    /// Takes the asset out and frees its slot for reuse.
    ///
    /// The slot is cleared *before* its generation is bumped. Reversing those
    /// two is the mistake `World::despawn` documents in ARCHITECTURE.md: bump
    /// first and the value stays in the arena with no handle able to reach it,
    /// which leaks one asset per removal forever.
    pub fn remove(&mut self, handle: Handle<T>) -> Option<T> {
        let slot = self.slot(handle)?;
        let asset = self.slots[slot].take()?;

        self.generations[slot] = self.generations[slot].wrapping_add(1);
        self.free.push(handle.index());
        self.live -= 1;
        Some(asset)
    }

    /// How many assets are stored.
    pub fn len(&self) -> usize {
        self.live
    }

    pub fn is_empty(&self) -> bool {
        self.live == 0
    }

    /// The slot `handle` addresses, if it is in range and current.
    fn slot(&self, handle: Handle<T>) -> Option<usize> {
        let slot = handle.index() as usize;
        // The bounds check is what makes a handle from another store safe to
        // pass in: its index may name a slot this arena does not have.
        if self.generations.get(slot).copied()? != handle.generation() {
            return None;
        }
        Some(slot)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_store_is_empty() {
        let store: Assets<u32> = Assets::new();
        assert_eq!(store.len(), 0);
        assert!(store.is_empty());
    }

    #[test]
    fn an_inserted_asset_reads_back() {
        let mut store = Assets::new();
        let handle = store.insert("hero");
        assert_eq!(store.get(handle), Some(&"hero"));
        assert_eq!(store.len(), 1);
        assert!(!store.is_empty());
    }

    #[test]
    fn get_mut_hands_out_the_stored_value() {
        let mut store = Assets::new();
        let handle = store.insert(1u32);
        if let Some(value) = store.get_mut(handle) {
            *value = 2;
        }
        assert_eq!(store.get(handle), Some(&2));
    }

    #[test]
    fn removing_returns_the_asset_and_empties_the_slot() {
        let mut store = Assets::new();
        let handle = store.insert("hero");
        assert_eq!(store.remove(handle), Some("hero"));
        assert_eq!(store.get(handle), None);
        assert_eq!(store.len(), 0);
    }

    #[test]
    fn removing_twice_reports_the_second_as_gone() {
        let mut store = Assets::new();
        let handle = store.insert("hero");
        assert_eq!(store.remove(handle), Some("hero"));
        assert_eq!(store.remove(handle), None);
    }

    #[test]
    fn a_stale_handle_does_not_read_the_slot_that_replaced_it() {
        // The invariant the generation exists for. Without it, `stale` would
        // address "villain" — the bug this whole shape prevents.
        let mut store = Assets::new();
        let stale = store.insert("hero");
        store.remove(stale);
        let fresh = store.insert("villain");

        assert_eq!(fresh.index(), stale.index(), "the slot must be reused");
        assert_ne!(fresh.generation(), stale.generation());
        assert_eq!(store.get(stale), None);
        assert_eq!(store.get(fresh), Some(&"villain"));
    }

    #[test]
    fn a_stale_handle_cannot_remove_the_slot_that_replaced_it() {
        let mut store = Assets::new();
        let stale = store.insert("hero");
        store.remove(stale);
        let fresh = store.insert("villain");

        assert_eq!(store.remove(stale), None);
        assert_eq!(store.get(fresh), Some(&"villain"));
    }

    #[test]
    fn a_handle_from_another_store_does_not_resolve() {
        // Indices are per-store, so a handle from one is meaningless in
        // another. `get` must answer `None` rather than index out of bounds.
        let mut a = Assets::new();
        let from_a = a.insert("hero");
        let b: Assets<&str> = Assets::new();
        assert_eq!(b.get(from_a), None);
    }

    #[test]
    fn slots_are_reused_rather_than_growing_the_arena() {
        let mut store = Assets::new();
        for _ in 0..8 {
            let handle = store.insert(0u32);
            store.remove(handle);
        }
        let handle = store.insert(1u32);
        assert_eq!(handle.index(), 0, "one slot should have served all of them");
        assert_eq!(store.len(), 1);
    }
}
