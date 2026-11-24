//! A fast hasher for the entity-keyed maps rebuilt every frame — the
//! physics step's and the draw's.
//!
//! The standard library's SipHash resists collision attacks on keys an
//! attacker chooses. Entity ids are handed out by the world, never chosen
//! from outside, and a frame hashes thousands of them — measured, SipHash was
//! most of what sleeping bookkeeping cost, and most of composing transforms. This is the
//! multiply-rotate mix `rustc` itself uses (FxHash), in a dozen lines rather
//! than a dependency.

use std::collections::{HashMap, HashSet};
use std::hash::{BuildHasherDefault, Hasher};

/// FxHash's multiplier: odd, with its bits spread so a small change in the
/// key moves the high bits a hash table indexes by.
const SEED: u64 = 0x51_7c_c1_b7_27_22_0a_95;

/// A multiply-rotate hasher; see the module docs.
#[derive(Debug, Default, Clone, Copy)]
pub struct FastHasher(u64);

impl FastHasher {
    fn add(&mut self, word: u64) {
        self.0 = (self.0.rotate_left(5) ^ word).wrapping_mul(SEED);
    }
}

impl Hasher for FastHasher {
    fn write(&mut self, bytes: &[u8]) {
        for chunk in bytes.chunks(8) {
            let mut word = [0u8; 8];
            word[..chunk.len()].copy_from_slice(chunk);
            self.add(u64::from_le_bytes(word));
        }
    }

    fn write_u16(&mut self, n: u16) {
        self.add(u64::from(n));
    }

    fn write_u32(&mut self, n: u32) {
        self.add(u64::from(n));
    }

    fn write_u64(&mut self, n: u64) {
        self.add(n);
    }

    fn write_usize(&mut self, n: usize) {
        self.add(n as u64);
    }

    fn finish(&self) -> u64 {
        self.0
    }
}

/// A `HashMap` hashed with [`FastHasher`].
pub type FastMap<K, V> = HashMap<K, V, BuildHasherDefault<FastHasher>>;
/// A `HashSet` hashed with [`FastHasher`].
pub type FastSet<K> = HashSet<K, BuildHasherDefault<FastHasher>>;

#[cfg(test)]
mod tests {
    use std::hash::Hash;

    use super::*;

    fn hash(value: impl Hash) -> u64 {
        let mut hasher = FastHasher::default();
        value.hash(&mut hasher);
        hasher.finish()
    }

    #[test]
    fn equal_keys_hash_equal_and_neighbours_do_not() {
        assert_eq!(hash((3u32, 7u32)), hash((3u32, 7u32)));
        assert_ne!(hash((3u32, 7u32)), hash((3u32, 8u32)));
        assert_ne!(hash((3u32, 7u32)), hash((7u32, 3u32)), "order matters");
    }

    #[test]
    fn a_map_works_as_a_map() {
        let mut map: FastMap<u32, &str> = FastMap::default();
        map.insert(1, "one");
        map.insert(2, "two");
        assert_eq!(map.get(&1), Some(&"one"));
        assert_eq!(map.len(), 2);
    }
}
