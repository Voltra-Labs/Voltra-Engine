//! A typed reference to something in an [`Assets`](crate::store::Assets) store.

use std::fmt;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;

/// A typed reference to a stored asset.
///
/// The generation is what makes this safe to keep around: a slot is reused
/// after its occupant is removed, and without a generation an old handle would
/// silently address whatever asset took the slot. Same shape as
/// `voltra_ecs::Entity`, so the engine has one idea of what a handle is.
pub struct Handle<T> {
    index: u32,
    generation: u32,
    /// `fn() -> T` rather than `T`: a handle does not own a `T`, so it should
    /// be `Send`, `Sync` and covariant no matter what `T` is.
    _marker: PhantomData<fn() -> T>,
}

impl<T> Handle<T> {
    /// Crate-visible to allow other modules to construct handles.
    pub(crate) fn new(index: u32, generation: u32) -> Self {
        Self {
            index,
            generation,
            _marker: PhantomData,
        }
    }

    /// Slot this handle addresses.
    pub fn index(&self) -> u32 {
        self.index
    }

    /// How many times that slot has been reused.
    pub fn generation(&self) -> u32 {
        self.generation
    }
}

// Every trait below is implemented by hand rather than derived. A derive on a
// generic struct emits `impl<T: Clone>` and friends, which would make a
// `Handle` stop being `Copy` as soon as it named an asset type that is not —
// even though the `T` never appears in a field.

impl<T> Clone for Handle<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for Handle<T> {}

impl<T> PartialEq for Handle<T> {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index && self.generation == other.generation
    }
}

impl<T> Eq for Handle<T> {}

impl<T> Hash for Handle<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.index.hash(state);
        self.generation.hash(state);
    }
}

impl<T> fmt::Debug for Handle<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Handle({}v{})", self.index, self.generation)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A type that is deliberately neither `Clone` nor `Debug`.
    ///
    /// `#[derive(Clone)]` on a generic struct generates `impl<T: Clone>`, even
    /// when the only `T` in the struct is behind a `PhantomData` that does not
    /// own one. A derived `Handle` would therefore stop being `Copy` the moment
    /// it named an asset type that is not, which is a trap worth a test rather
    /// than a comment.
    struct NotClone;

    #[test]
    fn a_handle_is_copy_whatever_it_points_at() {
        let handle: Handle<NotClone> = Handle::new(3, 7);
        let copy = handle;
        assert_eq!(handle, copy);
        assert_eq!(handle.index(), 3);
        assert_eq!(handle.generation(), 7);
    }

    #[test]
    fn handles_to_different_slots_differ() {
        let a: Handle<NotClone> = Handle::new(0, 0);
        let b: Handle<NotClone> = Handle::new(1, 0);
        assert_ne!(a, b);
    }

    #[test]
    fn the_same_slot_at_different_generations_differs() {
        // The whole point of the generation. Two handles naming slot 0 are not
        // interchangeable if the slot was reused in between.
        let old: Handle<NotClone> = Handle::new(0, 0);
        let new: Handle<NotClone> = Handle::new(0, 1);
        assert_ne!(old, new);
    }

    #[test]
    fn a_handle_can_key_a_hash_map() {
        let mut map = std::collections::HashMap::new();
        map.insert(Handle::<NotClone>::new(2, 5), "hero");
        assert_eq!(map.get(&Handle::<NotClone>::new(2, 5)), Some(&"hero"));
        assert_eq!(map.get(&Handle::<NotClone>::new(2, 6)), None);
    }
}
