//! A number that changes whenever a layer's cells do.
//!
//! What lets something built from the cells — the generated colliders, a
//! future per-chunk mesh — tell whether it is stale without comparing the
//! cells themselves. Godot's `TileMapLayer` keeps a dirty flag for the same
//! reason; a flag needs someone to clear it, and two consumers would fight
//! over who does, so this is a value each consumer remembers instead.

use std::sync::atomic::{AtomicU64, Ordering};

/// Every revision ever handed out comes from here, so two layers — or one
/// layer before and after an undo replaced it with a deserialized copy —
/// never share a number unless one is a clone of the other.
static NEXT: AtomicU64 = AtomicU64::new(1);

/// Which edit of its cells a layer is at.
///
/// **Equal to every other revision**, on purpose: two layers with the same
/// cells are the same layer whatever edit history brought them there, and the
/// derived `PartialEq` on `Tilemap` must keep meaning that. Read the number
/// with [`Revision::get`] to compare revisions themselves.
#[derive(Debug, Clone, Copy)]
pub struct Revision(u64);

impl Revision {
    /// A number no other revision has.
    pub fn fresh() -> Self {
        Self(NEXT.fetch_add(1, Ordering::Relaxed))
    }

    pub fn get(self) -> u64 {
        self.0
    }
}

impl Default for Revision {
    /// Fresh, not zero: a layer loaded from disk or rebuilt by an undo is new
    /// content as far as anything cached from the old one is concerned.
    fn default() -> Self {
        Self::fresh()
    }
}

impl PartialEq for Revision {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_revisions_differ() {
        assert_ne!(Revision::fresh().get(), Revision::fresh().get());
    }

    #[test]
    fn a_default_revision_is_fresh() {
        assert_ne!(Revision::default().get(), Revision::default().get());
    }

    #[test]
    fn any_two_revisions_compare_equal() {
        assert_eq!(Revision::fresh(), Revision::fresh());
    }
}
