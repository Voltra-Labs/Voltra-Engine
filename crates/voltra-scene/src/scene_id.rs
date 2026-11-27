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
#[serde(transparent)]
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

    /// The same identity every time for the same two: what `record`, a
    /// record of some prefab, becomes inside the instance whose own id is
    /// `within`.
    ///
    /// For an entity a prefab sync adds to an instance — a record the file
    /// gained — so that two readings of one stored instance (a prefab read
    /// by the store, and the same file opened in prefab mode) give the
    /// added entity one id, and an instance of the outer file keeps naming
    /// it. A UUID v7 carrying `within`'s timestamp, its other bits a 128-bit
    /// mix of the two: a save orders by id, and this sorts it among the
    /// entities of its instance rather than anywhere in the file, the same
    /// place every time.
    pub fn derived(within: SceneId, record: SceneId) -> Self {
        let (a, b) = (within.0.as_u128(), record.0.as_u128());
        let words = [(a >> 64) as u64, a as u64, (b >> 64) as u64, b as u64];
        let mut high = 0x9e37_79b9_7f4a_7c15_u64;
        let mut low = 0xc2b2_ae3d_27d4_eb4f_u64;
        for word in words {
            high = mix(high ^ word);
            low = mix(low.rotate_left(29) ^ word ^ high);
        }
        let mut bytes = ((u128::from(high) << 64) | u128::from(low)).to_be_bytes();
        // The first 48 bits of a v7 are its milliseconds: `within`'s.
        bytes[..6].copy_from_slice(&within.0.as_bytes()[..6]);
        Self(
            uuid::Builder::from_bytes(bytes)
                .with_variant(uuid::Variant::RFC4122)
                .with_version(uuid::Version::SortRand)
                .into_uuid(),
        )
    }
}

/// SplitMix64's finaliser: every input bit reaches every output bit.
fn mix(mut x: u64) -> u64 {
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    x ^ (x >> 31)
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
    fn a_derived_id_is_the_same_for_the_same_two_and_differs_otherwise() {
        let (a, b) = (SceneId::new(), SceneId::new());

        assert_eq!(SceneId::derived(a, b), SceneId::derived(a, b));
        assert_ne!(SceneId::derived(a, b), SceneId::derived(b, a));
        assert_ne!(SceneId::derived(a, b), a);
        assert_ne!(SceneId::derived(a, b), SceneId::derived(a, SceneId::new()));
    }

    #[test]
    fn a_derived_id_sorts_with_the_instance_it_was_made_in() {
        let within = SceneId(
            "019fe500-0000-7000-8000-000000000001"
                .parse()
                .expect("a uuid"),
        );
        let later = SceneId::new();

        let made = SceneId::derived(within, later);

        assert_eq!(made.0.as_bytes()[..6], within.0.as_bytes()[..6]);
        assert!(made < later, "with its instance, not where it was made");
        assert_eq!(made.0.get_version_num(), 7);
    }

    #[test]
    fn unknown_components_start_empty() {
        assert!(UnknownComponents::default().0.is_empty());
    }
}
