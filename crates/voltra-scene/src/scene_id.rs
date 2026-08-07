//! What identifies an entity in a scene file, and what to do with the parts of
//! that file this build does not understand.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A stable identity that survives saving, loading and reordering a file.
///
/// `Entity` is an index and a generation, both recycled by the allocator at
/// runtime — allocator bookkeeping, not identity, and it must never reach a
/// file. Only entities carrying a `SceneId` are saved, so a transient runtime
/// spawn opts out simply by not having one, and no exclusion list has to exist.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SceneId(pub Uuid);

impl SceneId {
    /// A fresh identity.
    ///
    /// UUID **v7**, which carries a timestamp in its high bits, so ordering by
    /// id is ordering by creation. That is what lets a scene file be both
    /// deterministic and append-only in a diff; v4 forces a choice between the
    /// two.
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }
}

impl Default for SceneId {
    fn default() -> Self {
        Self::new()
    }
}

/// Components read from a file that no registered type claimed.
///
/// Held unparsed and written straight back out on save. A build that does not
/// know what `Physics` is can still open a scene, move a sprite and save without
/// deleting it — which is the failure mode this exists to prevent, and the one
/// Unity is criticised for.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct UnknownComponents(pub BTreeMap<String, Box<ron::value::RawValue>>);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique() {
        let a = SceneId::new();
        let b = SceneId::new();
        assert_ne!(a, b);
    }

    #[test]
    fn ids_sort_by_creation_order() {
        // The whole reason for v7 over v4. Sorting a scene file by id has to be
        // the same as sorting it by when each entity was made, or new entities
        // land in the middle of the diff instead of at the end.
        let mut ids: Vec<SceneId> = (0..64).map(|_| SceneId::new()).collect();
        let created = ids.clone();
        ids.sort();
        assert_eq!(ids, created, "v7 ids must already be in creation order");
    }

    #[test]
    fn unknown_components_start_empty() {
        assert!(UnknownComponents::default().0.is_empty());
    }
}
